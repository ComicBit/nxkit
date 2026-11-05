//! Capcom's RE Engine PAK (`kpka_entries`, `kpka_data`, `kpka_jobs`, `kpka_shaders`).

use std::collections::HashMap;
use std::sync::Arc;

use super::{Ctx, File, Job, JobOut, Plan, R};
use crate::SplitError;
use crate::bahs::BahsVar;
use crate::rd::{red, u16le, u32le, u64le};
use crate::shaders::Found;
use crate::src::Src;

/// (path hash, offset, stored length, length, attributes)
pub type Entry = (u64, u64, u64, u64, u64);

/// `kpka_entries`: `None` when the file is no KPKA PAK.
fn entries(s: &dyn Src, rel: &str) -> R<Option<Vec<Entry>>> {
    let h = s.read_up(0, 16)?;
    if h.get(..4) != Some(b"KPKA") {
        return Ok(None);
    }
    let (ver, feat, n) = (h[4], u16le(&h, 6)?, u32le(&h, 8)?);
    if ver != 4 || feat != 0 {
        return red(format!(
            "/{rel}: PAK version {ver}, features {feat:#x}: only version 4 with a plain entry table is known"
        ));
    }
    let t = s.read_up(16, 0x30 * n)?;
    let mut out = Vec::new();
    for c in t.chunks_exact(0x30) {
        let (lo, hi) = (u32le(c, 0)? as u64, u32le(c, 4)? as u64);
        out.push((
            lo | hi << 32,
            u64le(c, 8)?,
            u64le(c, 16)?,
            u64le(c, 24)?,
            u64le(c, 32)?,
        ));
    }
    Ok(Some(out))
}

/// `kpka_data`: an entry's bytes (`head`: its first bytes only).
fn data(s: &dyn Src, e: &Entry, head: usize) -> R<Vec<u8>> {
    let raw = s.read_up(e.1, usize::try_from(e.2).unwrap_or(usize::MAX))?;
    match e.4 & 0xf {
        0 => Ok(if head != 0 {
            raw[..head.min(raw.len())].to_vec()
        } else {
            raw
        }),
        1 => {
            if head != 0 {
                crate::inflate::inflate_head(&raw, head, false)
                    .ok_or_else(|| SplitError::Red("zlib.error".into()))
            } else {
                crate::inflate::inflate(&raw, usize::MAX >> 1)
                    .ok_or_else(|| SplitError::Red("zlib.error: damaged deflate data".into()))
            }
        }
        2 => {
            let d = crate::zstd::decompress(&raw)?;
            Ok(if head != 0 {
                d[..head.min(d.len())].to_vec()
            } else {
                d
            })
        }
        c => red(format!(
            "PAK entry {:016x}: compression {c} is not known",
            e.0
        )),
    }
}

fn shaders(s: &dyn Src, rel: &str, ents: &[Entry]) -> R<JobOut> {
    let mut out = Vec::<Found>::new();
    for e in ents {
        if e.3 >= 0x10 && data(s, e, 4)? == b"SDF\0" {
            out.extend(crate::capcom::sdf_shaders(
                &data(s, e, 0)?,
                &format!("/{rel}#{:016x}", e.0),
            )?);
        }
    }
    Ok(JobOut {
        found: out,
        var: BahsVar::new(),
    })
}

/// `kpka_jobs`: the newest copy of each path, in ranges of entries of about equal stored size.
pub fn plan(files: &[File], p: &mut Plan) -> R<()> {
    let mut paks = Vec::new();
    for (i, f) in files.iter().enumerate() {
        if f.live
            && f.rel.ends_with(".pak")
            && let Some(es) = entries(f.src.as_ref(), &f.rel)?
        {
            paks.push((i, es));
        }
    }
    if paks.is_empty() {
        return Ok(());
    }
    paks.sort_by(|a, b| files[a.0].rel.cmp(&files[b.0].rel));
    let mut view: HashMap<u64, (usize, Entry)> = HashMap::new();
    for (i, es) in &paks {
        for e in es {
            view.insert(e.0, (*i, *e));
        }
    }
    let step = view.values().map(|v| v.1.2).sum::<u64>() as f64 / 64.0;
    let mut items: Vec<(usize, Entry)> = view.into_values().collect();
    items.sort_by(|a, b| (&files[a.0].rel, a.1.1).cmp(&(&files[b.0].rel, b.1.1)));
    let (mut cur, mut order, mut size): (HashMap<usize, Vec<Entry>>, Vec<usize>, u64) =
        (HashMap::new(), Vec::new(), 0);
    let flush = |cur: &mut HashMap<usize, Vec<Entry>>, order: &mut Vec<usize>, p: &mut Plan| {
        for i in order.drain(..) {
            let es = cur.remove(&i).unwrap_or_default();
            let (src, rel): (Arc<dyn Src>, String) = (files[i].src.clone(), files[i].rel.clone());
            let r2 = rel.clone();
            p.jobs.push(Job {
                rel,
                run: Box::new(move |_: &Ctx<'_>, _| shaders(src.as_ref(), &r2, &es)),
            });
        }
    };
    for (i, e) in items {
        if !cur.contains_key(&i) {
            order.push(i);
        }
        cur.entry(i).or_default().push(e);
        size += e.2;
        if size as f64 >= step {
            flush(&mut cur, &mut order, p);
            size = 0;
        }
    }
    flush(&mut cur, &mut order, p);
    for (i, _) in paks {
        p.taken.insert(i);
    }
    Ok(())
}
