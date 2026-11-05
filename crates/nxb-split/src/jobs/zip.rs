//! Zip archives (`zip_jobs`, `zip_shaders`, Python's `zipfile` for stored and deflated members, Zip64 included).

use std::collections::HashMap;
use std::sync::Arc;

use super::{Ctx, File, Job, JobOut, Plan, R, ranges};
use crate::SplitError;
use crate::bahs::BahsVar;
use crate::glslc::glslc_shaders;
use crate::rd::red;
use crate::shaders::{Found, memfind, scan_blob};
use crate::src::Src;

/// IBM code page 437 above ASCII (`zipfile` decodes names without the UTF-8 flag with it).
const CP437: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■\u{a0}";

/// One central directory entry.
#[derive(Clone, Debug)]
pub struct Member {
    name: String,
    raw_name: Vec<u8>,
    flags: u16,
    method: u16,
    crc: u32,
    csize: u64,
    size: u64,
    offset: u64,
}

fn bad(m: &str) -> SplitError {
    SplitError::Red(format!("zipfile.BadZipFile: {m}"))
}

fn le(d: &[u8], o: usize, n: usize) -> R<u64> {
    let b = d.get(o..o + n).ok_or_else(|| bad("truncated header"))?;
    Ok(b.iter().rev().fold(0u64, |a, &x| a << 8 | u64::from(x)))
}

fn decode_name(raw: &[u8], flags: u16) -> R<String> {
    if flags & 0x800 != 0 {
        return String::from_utf8(raw.to_vec())
            .map_err(|_| SplitError::Red("UnicodeDecodeError: a zip member name".into()));
    }
    let hi: Vec<char> = CP437.chars().collect();
    Ok(raw
        .iter()
        .map(|&b| {
            if b < 0x80 {
                b as char
            } else {
                hi[usize::from(b) - 0x80]
            }
        })
        .collect())
}

/// The members of a zip (central directory order).
pub fn members(s: &dyn Src) -> R<Vec<Member>> {
    let len = s.len();
    let tail_n = len.min(0x10000 + 22 + 20) as usize;
    let tail = s.read_up(len - tail_n as u64, tail_n)?;
    let e = tail
        .windows(4)
        .rposition(|w| w == b"PK\x05\x06")
        .ok_or_else(|| bad("File is not a zip file"))?;
    let (mut count, mut cd_size, mut cd_off) = (
        le(&tail, e + 10, 2)?,
        le(&tail, e + 12, 4)?,
        le(&tail, e + 16, 4)?,
    );
    let eocd_at = len - tail_n as u64 + e as u64;
    if (count == 0xffff || cd_size == 0xffff_ffff || cd_off == 0xffff_ffff)
        && e >= 20
        && &tail[e - 20..e - 16] == b"PK\x06\x07"
    {
        let z64 = le(&tail, e - 12, 8)?;
        let r = s.read_up(z64, 56)?;
        if r.get(..4) != Some(b"PK\x06\x06") {
            return Err(bad("Zip64 end of central directory record not found"));
        }
        (count, cd_size, cd_off) = (le(&r, 32, 8)?, le(&r, 40, 8)?, le(&r, 48, 8)?);
    }
    if cd_off.checked_add(cd_size).is_none_or(|x| x > eocd_at + 20) {
        return Err(bad("bad central directory size"));
    }
    let cd = s.read_up(cd_off, cd_size as usize)?;
    let (mut p, mut out) = (0usize, Vec::new());
    for _ in 0..count {
        if cd.get(p..p + 4) != Some(b"PK\x01\x02") {
            return Err(bad("Bad magic number for central directory"));
        }
        let flags = le(&cd, p + 8, 2)? as u16;
        let (method, crc) = (le(&cd, p + 10, 2)? as u16, le(&cd, p + 16, 4)? as u32);
        let (mut csize, mut size, mut offset) = (
            le(&cd, p + 20, 4)?,
            le(&cd, p + 24, 4)?,
            le(&cd, p + 42, 4)?,
        );
        let (nl, xl, cl) = (
            le(&cd, p + 28, 2)? as usize,
            le(&cd, p + 30, 2)? as usize,
            le(&cd, p + 32, 2)? as usize,
        );
        let raw = cd
            .get(p + 46..p + 46 + nl)
            .ok_or_else(|| bad("truncated"))?
            .to_vec();
        let extra = cd
            .get(p + 46 + nl..p + 46 + nl + xl)
            .ok_or_else(|| bad("truncated"))?;
        let mut q = 0;
        while q + 4 <= extra.len() {
            let (id, n) = (le(extra, q, 2)?, le(extra, q + 2, 2)? as usize);
            if id == 1 {
                let mut at = q + 4;
                for v in [&mut size, &mut csize, &mut offset] {
                    if *v == 0xffff_ffff {
                        *v = le(extra, at, 8)?;
                        at += 8;
                    }
                }
            }
            q += 4 + n;
        }
        let name = decode_name(&raw, flags)?;
        out.push(Member {
            name,
            raw_name: raw,
            flags,
            method,
            crc,
            csize,
            size,
            offset,
        });
        p += 46 + nl + xl + cl;
    }
    Ok(out)
}

/// `ZipFile.read(name)`: the member's bytes, the CRC checked.
fn read(s: &dyn Src, m: &Member) -> R<Vec<u8>> {
    let h = s.read_up(m.offset, 30)?;
    if h.get(..4) != Some(b"PK\x03\x04") {
        return Err(bad("Bad magic number for file header"));
    }
    let (nl, xl) = (le(&h, 26, 2)? as usize, le(&h, 28, 2)? as usize);
    let nm = s.read_up(m.offset + 30, nl)?;
    if nm != m.raw_name {
        return Err(bad(&format!(
            "File name in directory {:?} and header differ.",
            m.name
        )));
    }
    if m.flags & 1 != 0 {
        return red(format!(
            "NotImplementedError: {} is encrypted, password required",
            m.name
        ));
    }
    let raw = s.read_up(m.offset + 30 + (nl + xl) as u64, m.csize as usize)?;
    let data = match m.method {
        0 => raw,
        8 => crate::inflate::inflate(&raw, m.size as usize + 1)
            .ok_or_else(|| bad("Error -3 while decompressing data"))?,
        12 | 14 => {
            return Err(SplitError::Unsupported(format!(
                "{}: zip compression {} (bzip2 or lzma)",
                m.name, m.method
            )));
        }
        n => {
            return red(format!(
                "NotImplementedError: That compression method is not supported ({n})"
            ));
        }
    };
    if data.len() as u64 != m.size {
        return Err(bad("the member's data does not have its declared size"));
    }
    if crate::zlibx::crc32(&data) != m.crc {
        return Err(bad("Bad CRC-32"));
    }
    Ok(data)
}

fn shaders(s: &dyn Src, all: &[Member], rel: &str, names: &[String]) -> R<JobOut> {
    let by: HashMap<&str, &Member> = all.iter().map(|m| (m.name.as_str(), m)).collect();
    let (mut out, mut var) = (Vec::<Found>::new(), BahsVar::new());
    for nm in names {
        let m = by.get(nm.as_str()).ok_or_else(|| {
            SplitError::Red(format!("KeyError: no item named {nm:?} in the archive"))
        })?;
        let d = read(s, m)?;
        let label = format!("/{rel}/{nm}");
        let n = out.len();
        scan_blob(&d, &label, &mut out, &mut var, 0)?;
        if out.len() == n
            && d.get(..4) != Some(b"MSHD")
            && memfind(&d, &[0x91, 0x68, 0x86, 0x19]).is_some()
        {
            out.extend(glslc_shaders(&d, &label, false, &[])?);
        }
    }
    Ok(JobOut { found: out, var })
}

/// `zip_jobs`.
pub fn plan(files: &[File], p: &mut Plan) -> R<()> {
    let mut zs = Vec::new();
    for (i, f) in files.iter().enumerate() {
        if !f.live || f.head(4)? != *b"PK\x03\x04" {
            continue;
        }
        match members(f.src.as_ref()) {
            Ok(ms) => zs.push((i, ms)),
            Err(SplitError::Red(e)) => p.notes.push(format!(
                "[RED] /{}: the zip cannot be read ({e}); scanned as one file",
                f.rel
            )),
            Err(e) => return Err(e),
        }
    }
    let total: u64 = zs
        .iter()
        .flat_map(|(_, ms)| ms.iter().filter(|m| !m.name.ends_with('/')).map(|m| m.size))
        .sum();
    for (i, ms) in zs {
        p.taken.insert(i);
        let f = &files[i];
        let items: Vec<(String, u64)> = ms
            .iter()
            .filter(|m| !m.name.ends_with('/'))
            .map(|m| (m.name.clone(), m.size))
            .collect();
        let all = Arc::new(ms);
        for chunk in ranges(&items, |x| x.1, total as f64 / 64.0) {
            let names: Vec<String> = chunk.into_iter().map(|x| x.0).collect();
            let (src, all, rel): (Arc<dyn Src>, _, String) =
                (f.src.clone(), all.clone(), f.rel.clone());
            let r2 = rel.clone();
            p.jobs.push(Job {
                rel,
                run: Box::new(move |_: &Ctx<'_>, _| shaders(src.as_ref(), &all, &r2, &names)),
            });
        }
    }
    Ok(())
}
