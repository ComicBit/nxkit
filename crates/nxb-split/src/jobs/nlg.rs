//! Next Level Games archives (`nlg_dict`, `nlg_walk`, `nlg_patch_section`, `nlg_shaders`): an index (`.dict`) beside
//! the data files and an optional update (`.patch`).

use std::collections::HashMap;

use super::{File, R};
use crate::SplitError;
use crate::bahs::BahsVar;
use crate::glslc::stage;
use crate::rd::{red, sl, u16le, u32le, u64le};
use crate::shaders::{CTL_MAGIC, Found, check};
use crate::src::Src;

/// The index magic.
pub const DICT: [u8; 4] = [0x58, 0x24, 0xf3, 0xa9];

/// Whether the file is read through its index: a `.data`, `.patch`, `.debug` or `.nxpc` with a `.dict` beside it.
pub fn covered(rel: &str, files: &[File]) -> bool {
    [".data", ".patch", ".debug", ".nxpc"]
        .iter()
        .any(|e| rel.ends_with(e))
        && {
            let stem = &rel[..rel.rfind('.').unwrap_or(rel.len())];
            let dict = format!("{stem}.dict");
            files.iter().any(|f| f.rel == dict)
        }
}

type Rec = (usize, Vec<usize>);
/// (offset, length, stored length, file)
type Chunk = (usize, usize, usize, usize);

/// (compressed, records, chunks, file names)
type Dict = (bool, Vec<Rec>, Vec<Chunk>, Vec<String>);

fn nlg_dict(d: &[u8], label: &str) -> R<Dict> {
    let (nchunk, nrec) = if d.len() >= 0x10 {
        (usize::from(d[0xc]), usize::from(d[0xd]))
    } else {
        (0, 0)
    };
    let co = 0x10 + 0x18 * nrec;
    let bad = d.len() < co + 0x10 * nchunk
        || nrec > nchunk
        || (0..nrec).any(|i| {
            sl(d, 0x18 + 0x18 * i, 0x28 + 0x18 * i)
                .iter()
                .any(|&c| usize::from(c) >= nchunk)
        });
    if bad {
        return red(format!(
            "{label}: the index ({:#x} bytes) is not in the layout of Luigi's Mansion 3's archives, the only one nlg_dict reads",
            d.len()
        ));
    }
    let (comp, nname) = (d[6] != 0, usize::from(d[0xe]));
    let recs = (0..nrec)
        .map(|i| {
            Ok((
                u16le(d, 0x14 + 0x18 * i)?,
                sl(d, 0x18 + 0x18 * i, 0x28 + 0x18 * i)
                    .iter()
                    .map(|&c| usize::from(c))
                    .collect(),
            ))
        })
        .collect::<R<Vec<_>>>()?;
    let chunks = (0..nchunk)
        .map(|i| {
            let o = co + 0x10 * i;
            Ok((
                u32le(d, o)?,
                u32le(d, o + 4)?,
                u32le(d, o + 8)?,
                usize::from(*d.get(o + 14).unwrap_or(&0)),
            ))
        })
        .collect::<R<Vec<_>>>()?;
    let names = sl(d, co + 0x10 * nchunk, d.len())
        .split(|&b| b == 0)
        .take(nname)
        .map(|n| n.iter().map(|&b| b as char).collect())
        .collect();
    Ok((comp, recs, chunks, names))
}

fn walk(tab: &[u8], start: usize, count: usize, out: &mut Vec<usize>) -> R<()> {
    for i in start..start + count {
        let (w0, w1, w2) = (
            u32le(tab, 12 * i)?,
            u32le(tab, 12 * i + 4)?,
            u32le(tab, 12 * i + 8)?,
        );
        if w0 >> 31 != 0 {
            walk(tab, w2, w1, out)?;
        } else {
            out.push(i);
        }
    }
    Ok(())
}

/// Python's `buf[a:b] = x` on a bytearray (the length changes when `x` has another length).
fn assign(buf: &mut Vec<u8>, a: usize, b: usize, x: &[u8]) {
    let a = a.min(buf.len());
    let b = b.clamp(a, buf.len());
    buf.splice(a..b, x.iter().copied());
}

#[allow(clippy::too_many_arguments)]
fn patch_section(
    tab: &mut [u8],
    order: &[usize],
    s: usize,
    old: &[u8],
    body: &[u8],
    cut: usize,
    end: usize,
    size: usize,
) -> R<Vec<u8>> {
    let mut buf = vec![0u8; old.len().max(size)];
    buf[..old.len()].copy_from_slice(old);
    let keep = if end != 0 {
        sl(old, cut, end).to_vec()
    } else {
        Vec::new()
    };
    let mut cur = 0usize;
    for &i in order {
        let (w0, n, src) = (
            u32le(tab, 12 * i)?,
            u32le(tab, 12 * i + 4)?,
            u32le(tab, 12 * i + 8)?,
        );
        if (w0 >> 27) & 0xf != s {
            continue;
        }
        let al = if w0 & 0x7c0_0000 != 0 {
            1usize << ((w0 >> 22) & 0x1f)
        } else {
            1
        };
        cur = (cur + al - 1) & !(al - 1);
        if src >= old.len() {
            assign(
                &mut buf,
                cur,
                cur + n,
                sl(body, src - old.len(), src - old.len() + n),
            );
        } else if end != 0 && cut <= src && src < end {
            assign(&mut buf, cur, cur + n, sl(&keep, src - cut, src - cut + n));
        } else if src != cur {
            let x = sl(&buf, src, src + n).to_vec();
            assign(&mut buf, cur, cur + n, &x);
        }
        tab[12 * i + 8..12 * i + 12].copy_from_slice(&(cur as u32).to_le_bytes());
        cur += n;
    }
    buf.truncate(size);
    Ok(buf)
}

/// `nlg_shaders`: the shaders of the archive whose index is `files[dict]`.
pub fn shaders(files: &[File], dict: usize, label: &str) -> R<(Vec<Found>, BahsVar)> {
    let d = files[dict].src.as_ref().all()?;
    let rel = &files[dict].rel;
    let stem = &rel[..rel.len() - 5];
    let (comp, recs, chunks, names) = nlg_dict(&d, label)?;
    let mut have: HashMap<usize, &dyn Src> = HashMap::new();
    for (i, nm) in names.iter().enumerate() {
        if let Some(f) = files.iter().find(|f| f.rel == format!("{stem}{nm}")) {
            have.insert(i, f.src.as_ref());
        }
    }
    let patch = files
        .iter()
        .find(|f| f.rel == format!("{stem}.patch"))
        .map(|f| f.src.as_ref().all())
        .transpose()?;
    let mut cache: HashMap<usize, Vec<u8>> = HashMap::new();
    let mut chunk = |ci: usize| -> R<Vec<u8>> {
        if let Some(c) = cache.get(&ci) {
            return Ok(c.clone());
        }
        let (off, n, stored, f) = chunks[ci];
        let src = have
            .get(&f)
            .ok_or_else(|| SplitError::Red("KeyError".into()))?;
        let raw = src.read_up(off as u64, if comp { stored } else { n })?;
        let v = if comp {
            crate::zlibx::zlib_stream(&raw, 1 << 31)
                .ok_or_else(|| SplitError::Red(format!("{label}: chunk {ci} does not inflate")))?
        } else {
            raw[..n.min(raw.len())].to_vec()
        };
        if v.len() != n {
            return red(format!(
                "{label}: chunk {ci} inflates to {:#x} bytes, the index says {n:#x}",
                v.len()
            ));
        }
        cache.insert(ci, v.clone());
        Ok(v)
    };
    let (mut mask, mut body, mut bsec) = (0u64, Vec::new(), [0usize; 16]);
    if let Some(p) = &patch {
        mask = u64le(p, 0)?;
        let (boff, bstored, bn) = (u32le(p, 8)?, u32le(p, 12)?, u32le(p, 16)?);
        body = crate::rd::zlib_all(sl(p, boff, boff + bstored), label)?;
        if body.len() != bn {
            return red(format!(
                "{label}: the patch body inflates to {:#x} bytes, its header says {bn:#x}",
                body.len()
            ));
        }
        for (k, s) in bsec.iter_mut().enumerate() {
            *s = u32le(&body, 4 * k)?;
        }
    }
    let (mut out, var) = (Vec::new(), BahsVar::new());
    for (v, (_top, secs)) in recs.iter().enumerate() {
        if std::iter::once(&v)
            .chain(secs.iter().filter(|c| **c != 0))
            .any(|&ci| !have.contains_key(&chunks[ci].3))
        {
            continue;
        }
        let mut sections: HashMap<usize, Vec<u8>> = HashMap::new();
        let live: Vec<usize>;
        let mut tab_r: Option<(Vec<u8>, Vec<usize>, Vec<usize>)> = None;
        if (mask >> v) & 1 == 1 {
            let p = patch
                .as_ref()
                .ok_or_else(|| SplitError::Red("TypeError".into()))?;
            let base = 0x18 + 0xd0 * (mask & ((1u64 << v) - 1)).count_ones() as usize;
            let r: Vec<usize> = (0..52).map(|k| u32le(p, base + 4 * k)).collect::<R<_>>()?;
            let tab = crate::rd::zlib_all(sl(p, r[0], r[0] + r[1]), label)?;
            if tab.len() != 12 * r[2] {
                return red(format!(
                    "{label}: variant {v} patch table has {} bytes for {} entries",
                    tab.len(),
                    r[2]
                ));
            }
            let mut order = Vec::new();
            walk(&tab, 0, r[3] & 0xffff, &mut order)?;
            live = (0..16).filter(|&s| r[4 + s] != 0).collect();
            tab_r = Some((tab, order, r));
        } else {
            live = (0..16).filter(|&s| secs[s] != 0).collect();
        }
        let mut section = |s: usize| -> R<Vec<u8>> {
            if let Some(x) = sections.get(&s) {
                return Ok(x.clone());
            }
            let x = match tab_r.as_mut() {
                None => chunk(secs[s])?,
                Some((tab, order, r)) => {
                    let old = if secs[s] != 0 {
                        chunk(secs[s])?
                    } else {
                        Vec::new()
                    };
                    let bend = if s < 15 { bsec[s + 1] } else { body.len() };
                    patch_section(
                        tab,
                        order,
                        s,
                        &old,
                        sl(&body, bsec[s], bend),
                        r[36 + s],
                        r[20 + s],
                        r[4 + s],
                    )?
                }
            };
            sections.insert(s, x.clone());
            Ok(x)
        };
        for &c in &live {
            let ctls = section(c)?;
            if sl(&ctls, 0, 4) != CTL_MAGIC {
                continue;
            }
            let n = u32le(&ctls, 0x20)?;
            if n == 0 {
                return red(format!(
                    "{label}: variant {v} section {c} has a control section of length 0"
                ));
            }
            let mut done = false;
            for &k in &live {
                if k == c {
                    continue;
                }
                let code = section(k)?;
                let (mut found, mut p, mut i, mut ok) = (Vec::new(), 0usize, 0usize, true);
                while sl(&ctls, i * n, i * n + 4) == CTL_MAGIC {
                    let ctl = sl(&ctls, i * n, (i + 1) * n);
                    let ln = u32le(ctl, 0x704)?;
                    let cd = sl(&code, p, p + ln);
                    let st = stage(ctl, cd);
                    match st {
                        Some(st) if check(st, ctl, cd).is_none() => {
                            found.push(Found {
                                place: format!("{label}#NLG{v}:{c}@{:#x}", i * n),
                                stage: st,
                                ctl: ctl.to_vec(),
                                code: cd.to_vec(),
                            });
                            p += ln;
                            i += 1;
                        }
                        _ => {
                            ok = false;
                            break;
                        }
                    }
                }
                if ok && !found.is_empty() {
                    out.extend(found);
                    done = true;
                    break;
                }
            }
            if !done {
                return red(format!(
                    "{label}: variant {v} section {c} holds control sections, but no section of the variant holds their code areas in order"
                ));
            }
        }
    }
    Ok((out, var))
}
