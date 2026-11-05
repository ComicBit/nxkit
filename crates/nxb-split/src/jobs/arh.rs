//! Monolith's ARH1 index with its ARD data file (`arh_entries`, `arh_jobs`, `arh_shaders`).

use std::collections::HashSet;
use std::sync::Arc;

use super::{Ctx, File, Job, JobOut, Plan, R};
use crate::bahs::BahsVar;
use crate::rd::{red, sl, u32le, u64le};
use crate::shaders::Found;
use crate::src::Src;

/// (path, offset in the ARD, stored size)
type Entry = (String, u64, u64);

fn ascii(b: &[u8]) -> bool {
    b.is_ascii()
}

/// `arh_entries`.
pub fn entries(d: &[u8]) -> R<Vec<Entry>> {
    if d.len() < 48 || &d[..4] != b"arh1" {
        return Ok(Vec::new());
    }
    let w: Vec<usize> = (0..9).map(|i| u32le(d, 4 + 4 * i)).collect::<R<_>>()?;
    let (count, so, sn, no, nn, fo, fn_, key) = (w[1], w[2], w[3], w[4], w[5], w[6], w[7], w[8]);
    let out_of = |o: usize, n: usize| o > d.len() || n > d.len() - o;
    if nn != count * 8 || out_of(so, sn) || out_of(no, nn) || out_of(fo, fn_ * 24) {
        return red("ARH1: index table out of bounds");
    }
    let key = ((key as u32) ^ 0xf3f3_5353).to_le_bytes();
    let dec = |o: usize, n: usize| -> Vec<u8> {
        let full = n & !3;
        let mut v: Vec<u8> = d[o..o + full]
            .iter()
            .enumerate()
            .map(|(i, &x)| x ^ key[i & 3])
            .collect();
        v.extend_from_slice(&d[o + full..o + n]);
        v
    };
    let strings = dec(so, sn);
    let nodes: Vec<(i32, i32)> = dec(no, nn)
        .chunks_exact(8)
        .map(|c| {
            (
                i32::from_le_bytes([c[0], c[1], c[2], c[3]]),
                i32::from_le_bytes([c[4], c[5], c[6], c[7]]),
            )
        })
        .collect();
    let mut names = std::collections::HashMap::new();
    for (i, &(nxt, prev)) in nodes.iter().enumerate() {
        if nxt >= 0 || prev < 0 {
            continue;
        }
        let off = (-(i64::from(nxt))) as usize;
        let end = strings
            .get(off..)
            .and_then(|x| x.iter().position(|&b| b == 0))
            .map(|p| p + off);
        let bad = match end {
            Some(e) => off >= strings.len() || e + 5 > strings.len() || !ascii(&strings[off..e]),
            None => true,
        };
        if bad {
            return red("ARH1: invalid path suffix");
        }
        let end = end.unwrap_or(0);
        let fid = u32le(&strings, end + 1)?;
        let (mut prefix, mut cur, mut seen) = (Vec::new(), i, HashSet::new());
        while cur != 0 {
            if seen.contains(&cur) || cur >= nodes.len() {
                return red("ARH1: invalid path trie");
            }
            seen.insert(cur);
            let parent = nodes[cur].1;
            if parent < 0 || parent as usize >= nodes.len() {
                return red("ARH1: invalid path parent");
            }
            let ch = cur as i32 ^ nodes[parent as usize].0;
            if !(0..128).contains(&ch) {
                return red("ARH1: invalid path character");
            }
            prefix.push(ch as u8);
            cur = parent as usize;
        }
        prefix.reverse();
        prefix.extend_from_slice(&strings[off..end]);
        names.insert(fid, prefix.iter().map(|&b| b as char).collect::<String>());
    }
    let mut out = Vec::new();
    for i in 0..fn_ {
        let at = fo + i * 24;
        let (off, size, flags, fid) = (
            u64le(d, at)?,
            u32le(d, at + 8)? as u64,
            u32le(d, at + 16)?,
            u32le(d, at + 20)?,
        );
        if size != 0 {
            out.push((
                names
                    .get(&fid)
                    .cloned()
                    .unwrap_or_else(|| format!("#{fid}")),
                off,
                size + if flags & 2 != 0 { 48 } else { 0 },
            ));
        }
    }
    Ok(out)
}

fn shaders(s: &dyn Src, rel: &str, ents: &[Entry]) -> R<JobOut> {
    let mut out = Vec::<Found>::new();
    for (name, off, size) in ents {
        let d = s.read_up(*off, *size as usize)?;
        crate::monolith::monolith_shaders(&d, &format!("/{rel}{name}"), &mut out)?;
    }
    Ok(JobOut {
        found: out,
        var: BahsVar::new(),
    })
}

/// `arh_jobs`: the entries of each ARH1 index in `parts` ranges, as jobs over its ARD file.
pub fn plan(files: &[File], p: &mut Plan) -> R<()> {
    for (i, f) in files.iter().enumerate() {
        if !f.rel.ends_with(".arh") || !f.live {
            continue;
        }
        let d = f.src.as_ref().all()?;
        if sl(&d, 0, 4) != b"arh1" {
            continue;
        }
        let ard_rel = format!("{}.ard", &f.rel[..f.rel.len() - 4]);
        let Some(ai) = files.iter().position(|x| x.rel == ard_rel) else {
            return red(format!("{}: ARH1 index has no ARD data file", f.rel));
        };
        let ents = entries(&d)?;
        let length = files[ai].src.len();
        if ents.iter().any(|(_, o, n)| *o > length || *n > length - o) {
            return red(format!("{}: ARD entry out of bounds", f.rel));
        }
        let step = ents.len().div_ceil(64).max(1);
        for chunk in ents.chunks(step) {
            let (src, rel): (Arc<dyn Src>, String) = (files[ai].src.clone(), ard_rel.clone());
            let (r2, es) = (rel.clone(), chunk.to_vec());
            p.jobs.push(Job {
                rel,
                run: Box::new(move |_: &Ctx<'_>, _| shaders(src.as_ref(), &r2, &es)),
            });
        }
        p.taken.insert(i);
        p.taken.insert(ai);
    }
    Ok(())
}
