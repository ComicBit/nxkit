//! CRI CPK archives (`cpk.py`: `utf`, `files`, `crilayla`, `read`.`cpk_jobs`, `cpk_shaders`).

use std::collections::HashMap;
use std::sync::Arc;

use super::{Ctx, File, Job, JobOut, Plan, R, ranges};
use crate::SplitError;
use crate::bahs::BahsVar;
use crate::rd::{sl, u32le};
use crate::shaders::{Found, scan_blob};
use crate::src::Src;

fn val_err(m: &str) -> SplitError {
    SplitError::Red(format!("ValueError: {m}"))
}

/// An @UTF cell.
#[derive(Clone, Debug)]
enum Val {
    Int(i128),
    Str(String),
    Other,
}

fn be(d: &[u8], o: usize, n: usize) -> R<u64> {
    let b = d.get(o..o.wrapping_add(n)).ok_or_else(|| {
        SplitError::Red("struct.error: unpack_from requires a buffer of more bytes".into())
    })?;
    Ok(b.iter().fold(0u64, |a, &x| a << 8 | u64::from(x)))
}

/// `unmask`: an @UTF table stored masked.
fn unmask(b: &[u8]) -> Vec<u8> {
    let mut m = 0x655fu32;
    b.iter()
        .map(|&x| {
            let o = x ^ (m & 0xff) as u8;
            m = m.wrapping_mul(0x4115) & 0xffff;
            o
        })
        .collect()
}

fn utf_at(d: &[u8], o: usize, n: usize) -> Vec<u8> {
    let t = sl(d, o, o + n);
    if t.get(..4) == Some(b"@UTF") {
        t.to_vec()
    } else {
        unmask(t)
    }
}

/// `utf`: the rows of an @UTF table at `d[o..]`.
fn utf(d: &[u8], o: usize) -> R<Vec<HashMap<String, Val>>> {
    if sl(d, o, o + 4) != b"@UTF" {
        return Err(val_err(&format!("no @UTF table at {o:#x}")));
    }
    let (ro, so, dof, nc, rw, nr) = (
        be(d, o + 10, 2)? as usize,
        be(d, o + 12, 4)? as usize,
        be(d, o + 16, 4)? as usize,
        be(d, o + 24, 2)? as usize,
        be(d, o + 26, 2)? as usize,
        be(d, o + 28, 4)? as usize,
    );
    let b = o + 8;
    let sz = |off: usize| -> R<String> {
        let a = b + so + off;
        let z = d
            .get(a..)
            .and_then(|x| x.iter().position(|&c| c == 0))
            .ok_or_else(|| val_err("substring not found"))?;
        Ok(String::from_utf8_lossy(&d[a..a + z]).into_owned())
    };
    let val = |t: u8, p: usize| -> R<(Val, usize)> {
        let w = |n: usize, signed: bool| -> R<(Val, usize)> {
            let v = be(d, p, n)?;
            let s = if signed {
                i128::from(v)
                    - if v >> (8 * n - 1) & 1 == 1 {
                        1i128 << (8 * n)
                    } else {
                        0
                    }
            } else {
                i128::from(v)
            };
            Ok((Val::Int(s), n))
        };
        match t {
            0 => w(1, false),
            1 => w(1, true),
            2 => w(2, false),
            3 => w(2, true),
            4 => w(4, false),
            5 => w(4, true),
            6 => w(8, false),
            7 => w(8, true),
            8 => Ok((Val::Other, 4)),
            0xa => Ok((Val::Str(sz(be(d, p, 4)? as usize)?), 4)),
            0xb => Ok((Val::Other, 8)),
            _ => Err(val_err(&format!("@UTF column type {t:#x}"))),
        }
    };
    let (mut cols, mut p) = (Vec::new(), o + 0x20);
    for _ in 0..nc {
        let fl = *d
            .get(p)
            .ok_or_else(|| SplitError::Red("IndexError".into()))?;
        let name = sz(be(d, p + 1, 4)? as usize)?;
        p += 5;
        match fl & 0xf0 {
            0x30 => {
                let (v, n) = val(fl & 0xf, p)?;
                p += n;
                cols.push((name, 'c', v, 0));
            }
            0x50 => cols.push((name, 'r', Val::Other, fl & 0xf)),
            _ => cols.push((name, 'z', Val::Other, 0)),
        }
    }
    let _ = dof;
    let mut rows = Vec::new();
    for i in 0..nr {
        let mut p = b + ro + i * rw;
        let mut r = HashMap::new();
        for (name, st, c, t) in &cols {
            match st {
                'r' => {
                    let (v, n) = val(*t, p)?;
                    p += n;
                    r.insert(name.clone(), v);
                }
                'c' => {
                    r.insert(name.clone(), c.clone());
                }
                _ => {}
            }
        }
        rows.push(r);
    }
    Ok(rows)
}

fn int(r: &HashMap<String, Val>, k: &str) -> Option<u64> {
    match r.get(k) {
        Some(Val::Int(v)) => Some(*v as u64),
        _ => None,
    }
}

/// (path in the archive, offset, stored length, length)
type Entry = (String, u64, u64, u64);

/// `cpk.files`.
fn files(s: &dyn Src, rel: &str) -> R<Vec<Entry>> {
    let mut h = s.read_up(0, 0x800)?;
    if h.get(..4) != Some(b"CPK ") {
        return Err(val_err(&format!("{rel}: not a CPK")));
    }
    let n = u32le(&h, 8)?;
    if 0x10 + n > h.len() {
        h = s.read_up(0, 0x10 + n)?;
    }
    let rows = utf(&utf_at(&h, 0x10, n), 0)?;
    let [hd] = rows.as_slice() else {
        return Err(val_err("not enough values to unpack"));
    };
    let (toc, ctt) = (int(hd, "TocOffset"), int(hd, "ContentOffset"));
    let Some(toc) = toc.filter(|&t| t != 0) else {
        return Err(val_err(&format!("{rel}: no TOC (ITOC only is not read)")));
    };
    let t = s.read_up(toc, 0x10)?;
    if t.get(..4) != Some(b"TOC ") {
        return Err(val_err(&format!("{rel}: no TOC at {toc:#x}")));
    }
    let n = u32le(&t, 8)?;
    let t = utf_at(&s.read_up(toc + 0x10, n)?, 0, n);
    let base = match ctt {
        Some(c) if c != 0 => toc.min(c),
        _ => toc,
    };
    let mut out = Vec::new();
    for r in utf(&t, 0)? {
        let dir = match r.get("DirName") {
            Some(Val::Str(d)) if !d.is_empty() => format!("{d}/"),
            _ => String::new(),
        };
        let Some(Val::Str(file)) = r.get("FileName") else {
            return Err(SplitError::Red("KeyError: 'FileName'".into()));
        };
        let fo = int(&r, "FileOffset")
            .ok_or_else(|| SplitError::Red("KeyError: 'FileOffset'".into()))?;
        let fs =
            int(&r, "FileSize").ok_or_else(|| SplitError::Red("KeyError: 'FileSize'".into()))?;
        out.push((
            format!("{dir}{file}"),
            base + fo,
            fs,
            int(&r, "ExtractSize").unwrap_or(fs),
        ));
    }
    Ok(out)
}

/// `crilayla`.
fn crilayla(c: &[u8]) -> R<Vec<u8>> {
    if c.get(..8) != Some(b"CRILAYLA") {
        return Err(val_err("not CRILAYLA"));
    }
    let (usz, csz) = (u32le(c, 8)?, u32le(c, 12)?);
    let mut out = vec![0u8; usz + 0x100];
    out[..0x100].copy_from_slice(
        sl(c, 0x10 + csz, 0x10 + csz + 0x100)
            .get(..0x100)
            .ok_or_else(|| val_err("short"))?,
    );
    let rev: Vec<u8> = sl(c, 0x10, 0x10 + csz).iter().rev().copied().collect();
    let bits = |p: usize, n: usize| -> R<usize> {
        if p + n > rev.len() * 8 {
            return Err(val_err("invalid literal for int() with base 2: ''"));
        }
        Ok((p..p + n).fold(0usize, |a, i| {
            a << 1 | usize::from(rev[i / 8] >> (7 - i % 8) & 1)
        }))
    };
    let (mut p, mut w) = (0usize, out.len() as i64 - 1);
    while w >= 0x100 {
        if bits(p, 1)? == 1 {
            let back = bits(p + 1, 13)? + 3;
            let mut v = bits(p + 14, 2)?;
            p += 16;
            let mut ln = 3 + v;
            if v == 3 {
                v = bits(p, 3)?;
                p += 3;
                ln += v;
                if v == 7 {
                    v = bits(p, 5)?;
                    p += 5;
                    ln += v;
                    if v == 31 {
                        loop {
                            v = bits(p, 8)?;
                            p += 8;
                            ln += v;
                            if v != 255 {
                                break;
                            }
                        }
                    }
                }
            }
            let wu = w as usize;
            if wu + 1 < ln || wu + back >= out.len() {
                return Err(val_err("a match outside the output"));
            }
            if back >= ln {
                out.copy_within(wu + back + 1 - ln..wu + back + 1, wu + 1 - ln);
                w -= ln as i64;
            } else {
                for _ in 0..ln {
                    out[w as usize] = out[w as usize + back];
                    w -= 1;
                }
            }
        } else {
            out[w as usize] = bits(p + 1, 8)? as u8;
            p += 9;
            w -= 1;
        }
    }
    Ok(out)
}

fn shaders(s: &dyn Src, rel: &str, ents: &[Entry], notes: &mut Vec<String>) -> R<JobOut> {
    let (mut out, mut var) = (Vec::<Found>::new(), BahsVar::new());
    for (nm, off, csz, usz) in ents {
        let c = s.read_up(*off, *csz as usize)?;
        let d = if csz != usz || c.get(..8) == Some(b"CRILAYLA") {
            crilayla(&c)
        } else {
            Ok(c)
        };
        let d = d.and_then(|d| {
            if d.len() as u64 != *usz {
                Err(val_err(&format!(
                    "{rel}#{nm}: {} bytes, the TOC says {usz}",
                    d.len()
                )))
            } else {
                Ok(d)
            }
        });
        match d {
            Ok(d) => scan_blob(&d, &format!("/{rel}/{nm}"), &mut out, &mut var, 0)?,
            Err(SplitError::Red(m)) => notes.push(format!("[RED] /{rel}/{nm}: {m}; not scanned")),
            Err(e) => return Err(e),
        }
    }
    Ok(JobOut { found: out, var })
}

/// `cpk_jobs`.
pub fn plan(fs_: &[File], p: &mut Plan) -> R<()> {
    let mut with = Vec::new();
    for (i, f) in fs_.iter().enumerate() {
        if f.rel.to_lowercase().ends_with(".cpk") && f.live && f.head(4)? == *b"CPK " {
            match files(f.src.as_ref(), &f.rel) {
                Ok(es) => with.push((i, es)),
                Err(SplitError::Red(e)) => p.notes.push(format!(
                    "[RED] /{}: the CPK cannot be read ({e}); scanned as one file",
                    f.rel
                )),
                Err(e) => return Err(e),
            }
        }
    }
    let step = with
        .iter()
        .flat_map(|x| x.1.iter().map(|e| e.3))
        .sum::<u64>() as f64
        / 64.0;
    for (i, es) in with {
        p.taken.insert(i);
        for chunk in ranges(&es, |e| e.3, step) {
            let (src, rel): (Arc<dyn Src>, String) = (fs_[i].src.clone(), fs_[i].rel.clone());
            let r2 = rel.clone();
            p.jobs.push(Job {
                rel,
                run: Box::new(move |_: &Ctx<'_>, n| shaders(src.as_ref(), &r2, &chunk, n)),
            });
        }
    }
    Ok(())
}
