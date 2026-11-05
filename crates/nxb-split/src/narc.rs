//! Camelot's NARC containers (`narc_members` and `undo` of `cra.py`): each member is packed by the kind in its
//! first byte.

use crate::SplitError;

fn bad(m: impl Into<String>) -> SplitError {
    SplitError::Format(m.into())
}

fn u32at(d: &[u8], o: usize) -> Result<usize, SplitError> {
    d.get(o..o + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
        .ok_or_else(|| bad("a field past the end"))
}

/// `narc_members`: the members of a NARC (BTAF offsets, GMIF data).
pub fn narc_members<'a>(d: &'a [u8], place: &str) -> Result<Vec<&'a [u8]>, SplitError> {
    if d.get(..4) != Some(b"NARC") {
        return Err(bad(format!("{place}: not a NARC")));
    }
    let (Some(o), Some(n)) = (
        d.get(0xc..0xe)
            .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize),
        d.get(0xe..0x10)
            .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize),
    ) else {
        return Err(bad("a field past the end"));
    };
    let mut o = o;
    let mut secs: Vec<(&[u8], usize)> = Vec::new();
    for _ in 0..n {
        if o + 8 > d.len() {
            return Err(bad(format!("{place}: a section past the end")));
        }
        let s = u32at(d, o + 4)?;
        secs.push((&d[o..o + 4], o));
        o += s;
    }
    // the last section of a name wins (a dict in the Python)
    let find = |name: &[u8]| secs.iter().rev().find(|s| s.0 == name).map(|s| s.1);
    let (Some(fat), Some(gmif)) = (find(b"BTAF"), find(b"GMIF")) else {
        return Err(bad(format!("{place}: no BTAF or GMIF")));
    };
    let data = gmif + 8;
    let mut out = Vec::new();
    for i in 0..u32at(d, fat + 8)? {
        let (a, b) = (u32at(d, fat + 12 + 8 * i)?, u32at(d, fat + 16 + 8 * i)?);
        if data + a > data + b || data + b > d.len() {
            return Err(bad(format!("{place}: member {i} past the end")));
        }
        out.push(&d[data + a..data + b]);
    }
    Ok(out)
}

/// `lz_undo` (kinds 0x50 and 0x60).
fn lz_undo(d: &[u8], mut at: usize) -> Result<Vec<u8>, SplitError> {
    let at_ = |i: usize| d.get(i).copied().ok_or_else(|| bad("LZ data past the end"));
    let mut out: Vec<u8> = Vec::new();
    while at < d.len() {
        let f = d[at];
        at += 1;
        if f == 0 {
            out.extend_from_slice(d.get(at..(at + 8).min(d.len())).unwrap_or(&[]));
            at += 8;
            continue;
        }
        let f = f.wrapping_neg();
        for bit in (0..8).rev() {
            if f >> bit & 1 == 0 {
                out.push(at_(at)?);
                at += 1;
                continue;
            }
            let (lo, hi) = (at_(at)?, at_(at + 1)?);
            let dist = (lo >> 4) as usize | (hi as usize) << 4;
            if dist == 0 {
                return Ok(out);
            }
            let mut ln = (lo & 0xf) as usize;
            if ln > 1 {
                at += 2;
            } else if ln == 1 {
                ln = at_(at + 2)? as usize + 0x11 + ((at_(at + 3)? as usize) << 8) + 0xff;
                at += 4;
            } else {
                ln = at_(at + 2)? as usize + 0x10;
                at += 3;
            }
            let s = out
                .len()
                .checked_sub(dist)
                .ok_or_else(|| bad("an LZ distance before the start"))?;
            for k in 0..ln {
                out.push(out[s + k]);
            }
        }
    }
    Ok(out)
}

/// `undo`: a member unpacked by the kind in its first byte. Anything else is the member itself.
pub fn undo(d: &[u8]) -> Result<Vec<u8>, SplitError> {
    let Some(&k) = d.first() else {
        return Ok(Vec::new());
    };
    match k {
        0x4d if d.get(..4) == Some(b"MMMM") => {
            let n = u32at(d, 8)?;
            let mut at = (12 + 4 * n + 15) & !15;
            let mut out = Vec::new();
            for i in 0..n {
                let s = u32at(d, 12 + 4 * i)?;
                out.extend(undo(
                    d.get(at.min(d.len())..(at + s).min(d.len())).unwrap_or(&[]),
                )?);
                at += (s + 15) & !15;
            }
            Ok(out)
        }
        0x94 if d.len() >= 8 => crate::lz4::block(&d[8..], u32at(d, 4)?),
        0x5a if d.get(..4) == Some(b"Z\0\0\0") => crate::zstd::decompress(&d[8.min(d.len())..]),
        0x50 if d.len() >= 8 => lz_undo(d, 8),
        0x60 if d.len() >= 4 => lz_undo(d, 4),
        0x70 | 0x80 if d.len() >= 0x10 => Ok(d[0x10..].to_vec()),
        0 | 0x10 | 0x20 | 0x30 if d.len() >= 0x10 => {
            let z = if k == 0x30 {
                crate::inflate::inflate(&d[0x10..], 1 << 30)
            } else {
                crate::zlibx::zlib_auto(&d[0x10..])
            };
            Ok(z.unwrap_or_else(|| d.to_vec()))
        }
        _ => Ok(d.to_vec()),
    }
}
