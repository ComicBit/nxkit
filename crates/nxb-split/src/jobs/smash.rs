//! The `data.arc` archive of one title (the Smash ARC format) (`smash_arc.py`, `smash_arc_jobs`, `smash_arc_shaders`).

use std::sync::Arc;

use super::{Ctx, File, Job, JobOut, Plan, R, ranges};
use crate::SplitError;
use crate::bahs::BahsVar;
use crate::glslc::glslc_shaders;
use crate::rd::{red, u32le, u64le};
use crate::shaders::{Found, memfind, scan_blob};
use crate::src::Src;

/// The archive's first eight bytes.
pub const MAGIC: [u8; 8] = [0x10, 0x32, 0x54, 0x76, 0x98, 0xef, 0xcd, 0xab];

/// (path Hash40, absolute offset, stored size, size, extension Hash40)
pub type Entry = (u64, u64, u32, u32, u64);

/// `smash_arc.files(path, regions=True)`: one entry per data, in offset order, `None` for another file.
pub fn files(s: &dyn Src, regions: bool) -> R<Option<Vec<Entry>>> {
    let h = s.read_up(0, 0x38)?;
    if h.get(..8) != Some(&MAGIC[..]) {
        return Ok(None);
    }
    let (file_off, fs_off) = (u64le(&h, 0x10)?, u64le(&h, 0x20)?);
    let t = s.read_up(fs_off, 16)?;
    let (size, stored) = (u32le(&t, 4)?, u32le(&t, 8)?);
    let fs = crate::zstd::decompress(&s.read_up(fs_off + 16, stored)?)?;
    if fs.len() != size {
        return red(format!(
            "file system table inflates to {:#x} bytes, its header says {size:#x}",
            fs.len()
        ));
    }
    let g = |o: usize| u32le(&fs, o);
    let nreg = usize::from(
        *fs.get(0x38)
            .ok_or_else(|| SplitError::Red("IndexError".into()))?,
    );
    let (tsize, n_paths, n_idx, n_dirs, n_off1, n_hfold, n_info, n_sub, n_data, n_off2, n_data2) = (
        g(0)?,
        g(4)?,
        g(8)?,
        g(12)?,
        g(16)?,
        g(20)?,
        g(24)?,
        g(28)?,
        g(32)?,
        g(36)?,
        g(40)?,
    );
    let (version, extra_dirs, extra_files, extra_f2d) = (g(0x3c)?, g(0x40)?, g(0x44)?, g(0x50)?);
    let mut o = 0x58 + nreg * 12;
    let (qd, sh, sfi, soe) = (g(o)?, g(o + 4)?, g(o + 8)?, g(o + 12)?);
    o += 16 + qd * 12 + sh * 20 + sfi * 4 + soe * 16;
    let (groups, buckets) = (g(o)?, g(o + 4)?);
    o += 8 + buckets * 8 + groups * 8;
    let p = o;
    o += n_paths * 32;
    let idx = o;
    o += n_idx * 8 + n_dirs * (8 + 0x34);
    let dof = o;
    let n_do = n_off1 + n_off2 + extra_dirs;
    o += n_do * 0x1c + n_hfold * 8;
    let fi = o;
    let n_fi = n_info + n_data2 + extra_files;
    o += n_fi * 16;
    let f2d = o;
    let n_f2d = n_sub + n_data2 + extra_f2d;
    o += n_f2d * 12;
    let fd = o;
    let n_fd = n_data + n_data2 + extra_files;
    o += n_fd * 16;
    if o > tsize {
        return red(format!(
            "version {version:#x}: the file system table does not have the layout this reader knows (it would end at {o:#x} of {tsize:#x})"
        ));
    }
    let (mut out, mut seen) = (Vec::new(), std::collections::HashSet::new());
    for i in 0..n_paths {
        let (c, w) = (g(p + 32 * i)?, g(p + 32 * i + 4)?);
        let k = w >> 8;
        if k >= n_idx {
            continue;
        }
        let fii = g(idx + 8 * k + 4)?;
        if fii >= n_fi {
            continue;
        }
        let (f2d0, flags) = (g(fi + 16 * fii + 8)?, g(fi + 16 * fii + 12)?);
        let count = if regions && flags & 0x8000 != 0 {
            nreg
        } else {
            1
        };
        for f2di in f2d0..f2d0 + count {
            if f2di >= n_f2d {
                break;
            }
            let (doi, fdi) = (g(f2d + 12 * f2di)?, g(f2d + 12 * f2di + 4)?);
            if doi >= n_do || fdi >= n_fd {
                continue;
            }
            let (off_in, csz, usz) = (
                g(fd + 16 * fdi)?,
                g(fd + 16 * fdi + 4)?,
                g(fd + 16 * fdi + 8)?,
            );
            let at = file_off + u64le(&fs, dof + 0x1c * doi)? + 4 * off_in as u64;
            if seen.contains(&at) || csz == 0 {
                continue;
            }
            let (ec, ew) = (g(p + 32 * i + 8)?, g(p + 32 * i + 12)?);
            seen.insert(at);
            out.push((
                u64::from(c as u32) | ((w & 0xff) as u64) << 32,
                at,
                csz as u32,
                usz as u32,
                u64::from(ec as u32) | ((ew & 0xff) as u64) << 32,
            ));
        }
    }
    out.sort_by_key(|e| e.1);
    Ok(Some(out))
}

/// `smash_arc.read`: a file's bytes, inflated when stored as a zstd frame.
fn read(s: &dyn Src, e: &Entry) -> R<Vec<u8>> {
    let d = s.read_up(e.1, e.2 as usize)?;
    if d.get(..4) == Some(&[0x28, 0xb5, 0x2f, 0xfd][..]) {
        crate::zstd::decompress(&d)
    } else {
        Ok(d[..d.len().min(e.3 as usize)].to_vec())
    }
}

/// `smash_arc_shaders`: each file of a range scanned as a file of its own.
fn shaders(s: &dyn Src, rel: &str, ents: &[Entry]) -> R<JobOut> {
    let (mut out, mut var) = (Vec::<Found>::new(), BahsVar::new());
    for e in ents {
        if e.3 == 0 {
            continue;
        }
        let b = read(s, e)?;
        let kind: Vec<u8> = if b.get(..4) == Some(b"HBSS") {
            (0x10..=0x13)
                .rev()
                .filter_map(|i| b.get(i).copied())
                .collect()
        } else {
            b.get(..4).unwrap_or(&b).to_vec()
        };
        let named = !kind.is_empty() && kind.iter().all(u8::is_ascii_alphanumeric);
        let k: String = if named {
            kind.iter().map(|&c| c as char).collect()
        } else {
            "data".into()
        };
        let label = format!("/{rel}/{k}#{:010x}", e.0);
        let n = out.len();
        scan_blob(&b, &label, &mut out, &mut var, 0)?;
        if out.len() == n && memfind(&b, &[0x91, 0x68, 0x86, 0x19]).is_some() {
            out.extend(glslc_shaders(&b, &label, false, &[])?);
        }
    }
    Ok(JobOut { found: out, var })
}

/// `smash_arc_jobs`.
pub fn plan(files_: &[File], p: &mut Plan) -> R<()> {
    for (i, f) in files_.iter().enumerate() {
        if !f.rel.ends_with(".arc") || !f.live || f.head(8)? != MAGIC {
            continue;
        }
        let Some(ents) = files(f.src.as_ref(), true)? else {
            continue;
        };
        p.taken.insert(i);
        let step = ents.iter().map(|e| u64::from(e.2)).sum::<u64>() as f64 / 128.0;
        for chunk in ranges(&ents, |e| u64::from(e.2), step) {
            let (rel, src): (String, Arc<dyn Src>) = (f.rel.clone(), f.src.clone());
            let r2 = rel.clone();
            p.jobs.push(Job {
                rel,
                run: Box::new(move |_: &Ctx<'_>, _| shaders(src.as_ref(), &r2, &chunk)),
            });
        }
    }
    Ok(())
}
