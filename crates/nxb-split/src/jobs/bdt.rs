//! FromSoftware's dvdbnd archives (`bhd5_entries`, `bdt_jobs`, `undcx`, `bnd3_members`, `bdt_shaders`).

use std::sync::Arc;

use super::{Ctx, File, Job, JobOut, Plan, R, ranges};
use crate::SplitError;
use crate::bahs::BahsVar;
use crate::glslc::glslc_shaders;
use crate::rd::{red, sl, u32be, u32le, u64le, zlib_all};
use crate::shaders::{Found, memfind};
use crate::src::Src;

const GLSLC: [u8; 4] = [0x91, 0x68, 0x86, 0x19];

/// (path hash, offset, size)
type Entry = (u32, u64, u32);

/// `bhd5_entries` of the `.bhd5` beside `bdt` (by `rel` with the extension swapped).
fn entries(index: &dyn Src, rel: &str) -> R<Vec<Entry>> {
    let d = index.all()?;
    if d.get(..5) != Some(&[b'B', b'H', b'D', b'5', 0xff][..]) {
        return red(format!("{rel}: not a little-endian BHD5 index"));
    }
    let (nb, tab) = (u32le(&d, 0x10)?, u64le(&d, 0x18)? as usize);
    let mut out = Vec::new();
    for i in 0..nb {
        let (n, at) = (
            u32le(&d, tab + 16 * i)?,
            u64le(&d, tab + 16 * i + 8)? as usize,
        );
        let t = sl(&d, at, at + 16 * n);
        for c in t.chunks_exact(16) {
            out.push((u32le(c, 0)? as u32, u64le(c, 8)?, u32le(c, 4)? as u32));
        }
    }
    Ok(out)
}

/// `undcx`: a DCX container (DFLT only).
fn undcx(d: &[u8], label: &str) -> R<Vec<u8>> {
    let (n, csz) = (u32be(d, 0x1c)?, u32be(d, 0x20)?);
    if sl(d, 0x28, 0x2c) != b"DFLT" || sl(d, 0x44, 0x48) != b"DCA\0" {
        return red(format!(
            "{label}: DCX method {:?}: only DFLT is known",
            sl(d, 0x28, 0x2c)
        ));
    }
    let out = zlib_all(sl(d, 0x4c, 0x4c + csz), label)?;
    if out.len() != n {
        return red(format!(
            "{label}: DCX holds {:#x} bytes, its header says {n:#x}",
            out.len()
        ));
    }
    Ok(out)
}

/// A Shift JIS name for ASCII and half-width katakana, two-byte characters need the table this port does not carry.
fn sjis(b: &[u8]) -> R<String> {
    let mut s = String::new();
    for &c in b {
        match c {
            0..=0x7f => s.push(c as char),
            0xa1..=0xdf => s.push(char::from_u32(0xff61 + u32::from(c) - 0xa1).unwrap_or('?')),
            _ => {
                return Err(SplitError::Unsupported(
                    "a Shift JIS name with double-byte characters (BND3)".into(),
                ));
            }
        }
    }
    Ok(s)
}

/// `bnd3_members`: (name, data) of each entry of a BND3 archive.
fn bnd3_members(d: &[u8], label: &str) -> R<Vec<(String, Vec<u8>)>> {
    let (fmt, be) = (*d.get(12).unwrap_or(&0), *d.get(13).unwrap_or(&1));
    if !(fmt == 0x5c || fmt == 0x7c) || be != 0 {
        return red(format!(
            "{label}: BND3 format {fmt:#x}, big-endian {be}: only formats 0x5c and 0x7c, little-endian, are known"
        ));
    }
    let count = u32le(d, 0x10)? as u32 as i32;
    let mut out = Vec::new();
    for i in 0..count.max(0) as usize {
        let o = 0x20 + 28 * i;
        let (n, off, nm, usz) = (
            u32le(d, o + 4)? as u32 as i32,
            u64le(d, o + 8)? as usize,
            u32le(d, o + 20)?,
            u32le(d, o + 24)? as u32 as i32,
        );
        if n != usz {
            return red(format!(
                "{label}: BND3 entry {i} is compressed ({n:#x} of {usz:#x} bytes)"
            ));
        }
        let z = d
            .get(nm..)
            .and_then(|x| x.iter().position(|&b| b == 0))
            .ok_or_else(|| SplitError::Red("ValueError: substring not found".into()))?;
        let name = sjis(&d[nm..nm + z])?.replace('\\', "/");
        let name = name
            .split_once("/Source/")
            .map_or(name.as_str(), |x| x.1)
            .to_string();
        out.push((name, sl(d, off, off + n.max(0) as usize).to_vec()));
    }
    Ok(out)
}

fn shaders(s: &dyn Src, rel: &str, ents: &[Entry]) -> R<JobOut> {
    let mut out = Vec::<Found>::new();
    for (h, off, n) in ents {
        let label = format!("/{rel}#{h:08x}");
        let mut d = s.read_up(*off, *n as usize)?;
        if d.get(..4) == Some(b"DCX\0") {
            d = undcx(&d, &label)?;
        }
        if d.get(..4) != Some(b"BND3") || memfind(&d, &GLSLC).is_none() {
            continue;
        }
        for (nm, b) in bnd3_members(&d, &label)? {
            if b.get(..4) == Some(&GLSLC[..]) {
                out.extend(glslc_shaders(&b, &format!("{label}/{nm}"), true, &[])?);
            }
        }
    }
    Ok(JobOut {
        found: out,
        var: BahsVar::new(),
    })
}

/// `bdt_jobs`.
pub fn plan(files: &[File], p: &mut Plan) -> R<()> {
    for (i, f) in files.iter().enumerate() {
        if !f.rel.ends_with(".bdt") {
            continue;
        }
        let idx = format!("{}.bhd5", &f.rel[..f.rel.len() - 4]);
        let Some(xi) = files.iter().position(|x| x.rel == idx) else {
            continue;
        };
        p.taken.insert(i);
        let mut ents = entries(files[xi].src.as_ref(), &idx)?;
        ents.sort_by_key(|e| e.1);
        let step = ents.iter().map(|e| u64::from(e.2)).sum::<u64>() as f64 / 64.0;
        for chunk in ranges(&ents, |e| u64::from(e.2), step) {
            let (src, rel): (Arc<dyn Src>, String) = (f.src.clone(), f.rel.clone());
            let r2 = rel.clone();
            p.jobs.push(Job {
                rel,
                run: Box::new(move |_: &Ctx<'_>, _| shaders(src.as_ref(), &r2, &chunk)),
            });
        }
    }
    Ok(())
}
