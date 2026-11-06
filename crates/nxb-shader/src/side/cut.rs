//! `cut(lib, side)`: split a metallib into the library without the named functions and the library of those
//! (port of `dmsh_side.cut` / `_take` / `_tags` / `_sized`). Python's slicing clamps at the end of the data, the port
//! does the same where the Python code slices, and fails where Python's `struct.unpack` would.

use std::collections::BTreeMap;
use std::fmt;

/// The library cannot be cut (Python raises `ValueError`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CutError(pub String);

impl fmt::Display for CutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CutError {}

fn err(s: &str) -> CutError {
    CutError(s.to_string())
}

fn unreadable() -> CutError {
    err("an unreadable library (struct.error)")
}

fn u16_at(b: &[u8], p: usize) -> Result<u16, CutError> {
    let s = b.get(p..p + 2).ok_or_else(unreadable)?;
    Ok(u16::from_le_bytes([s[0], s[1]]))
}

fn u32_at(b: &[u8], p: usize) -> Result<u32, CutError> {
    let s = b.get(p..p + 4).ok_or_else(unreadable)?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn u64_at(b: &[u8], p: usize) -> Result<u64, CutError> {
    let s = b.get(p..p + 8).ok_or_else(unreadable)?;
    let mut a = [0u8; 8];
    a.copy_from_slice(s);
    Ok(u64::from_le_bytes(a))
}

/// Python `b[x:y]` with clamping (and an empty result when `x > y`).
fn slice(b: &[u8], x: u64, y: u64) -> &[u8] {
    let n = b.len() as u64;
    let (x, y) = (x.min(n), y.min(n));
    if x >= y {
        &[]
    } else {
        // Both are at most b.len(), which fits usize.
        &b[x as usize..y as usize]
    }
}

fn idx(v: u64) -> Result<usize, CutError> {
    usize::try_from(v).map_err(|_| unreadable())
}

type Tags = Vec<([u8; 4], Vec<u8>)>;

/// `_tags(b, p, end)`.
fn tags(b: &[u8], mut p: usize, end: usize) -> Result<Tags, CutError> {
    let mut out = Vec::new();
    while p + 4 <= end {
        let tag: [u8; 4] = b
            .get(p..p + 4)
            .and_then(|s| s.try_into().ok())
            .ok_or_else(unreadable)?;
        p += 4;
        if &tag == b"ENDT" {
            return Ok(out);
        }
        let n = usize::from(u16_at(b, p)?);
        p += 2;
        out.push((tag, slice(b, p as u64, (p + n) as u64).to_vec()));
        p += n;
    }
    Err(err("an unterminated tag list"))
}

/// `_sized(b, p, n)`: n size-prefixed entries.
fn sized(b: &[u8], mut p: u64, n: usize) -> Result<Vec<Vec<u8>>, CutError> {
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let size = u64::from(u32_at(b, idx(p)?)?);
        out.push(slice(b, p, p + 4 + size).to_vec());
        p += 4 + size;
    }
    Ok(out)
}

fn pack_u64s(head: &mut [u8], at: usize, vals: &[i128]) -> Result<(), CutError> {
    for (k, v) in vals.iter().enumerate() {
        let v = u64::try_from(*v).map_err(|_| unreadable())?;
        let p = at + 8 * k;
        head.get_mut(p..p + 8)
            .ok_or_else(unreadable)?
            .copy_from_slice(&v.to_le_bytes());
    }
    Ok(())
}

struct Layout<'a> {
    b: &'a [u8],
    fns: Vec<Tags>,
    tail: &'a [u8],
    sections: [u64; 8],
}

/// `_take(b, keep, fns, tail, sections)`.
fn take(l: &Layout<'_>, keep: &[usize]) -> Result<Vec<u8>, CutError> {
    let [fo, fs, po, _ps, qo, _qs, bo, _bs] = l.sections;
    let b = l.b;
    let publics = sized(b, po, l.fns.len())?;
    let privates = sized(b, qo, l.fns.len())?;
    let (mut entries, mut newpub, mut newpriv, mut code) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for &i in keep {
        let t: BTreeMap<[u8; 4], &Vec<u8>> = l.fns[i].iter().map(|(k, v)| (*k, v)).collect();
        let offt = t
            .get(b"OFFT")
            .ok_or_else(|| err("an unreadable library (KeyError OFFT)"))?;
        let mdsz = t
            .get(b"MDSZ")
            .ok_or_else(|| err("an unreadable library (KeyError MDSZ)"))?;
        if offt.len() != 24 || mdsz.len() != 8 {
            return Err(unreadable());
        }
        let code_off = u64_at(offt, 16)?;
        let size = u64_at(mdsz, 0)?;
        let mut body = Vec::new();
        for (tag, v) in &l.fns[i] {
            body.extend_from_slice(tag);
            if tag == b"OFFT" {
                body.extend_from_slice(&24u16.to_le_bytes());
                for x in [newpub.len(), newpriv.len(), code.len()] {
                    body.extend_from_slice(&(x as u64).to_le_bytes());
                }
            } else {
                let n = u16::try_from(v.len()).map_err(|_| unreadable())?;
                body.extend_from_slice(&n.to_le_bytes());
                body.extend_from_slice(v);
            }
        }
        body.extend_from_slice(b"ENDT");
        let n = u32::try_from(body.len() + 4).map_err(|_| unreadable())?;
        entries.extend_from_slice(&n.to_le_bytes());
        entries.extend_from_slice(&body);
        newpub.extend_from_slice(&publics[i]);
        newpriv.extend_from_slice(&privates[i]);
        code.extend_from_slice(slice(b, bo + code_off, bo + code_off + size));
    }
    let mut listing = u32::try_from(keep.len())
        .map_err(|_| unreadable())?
        .to_le_bytes()
        .to_vec();
    listing.extend_from_slice(&entries);
    let (fo_i, po_i) = (i128::from(fo), i128::from(po));
    let all_entries = po_i - l.tail.len() as i128 - fo_i - 4;
    let npo = fo_i + listing.len() as i128 + l.tail.len() as i128;
    let nqo = npo + newpub.len() as i128;
    let nbo = nqo + newpriv.len() as i128;
    let mut head = slice(b, 0, fo).to_vec();
    pack_u64s(&mut head, 0x10, &[nbo + code.len() as i128])?;
    pack_u64s(
        &mut head,
        0x18,
        &[
            fo_i,
            i128::from(fs) - all_entries + listing.len() as i128 - 4,
            npo,
            newpub.len() as i128,
            nqo,
            newpriv.len() as i128,
            nbo,
            code.len() as i128,
        ],
    )?;
    let mut out = head;
    out.extend_from_slice(&listing);
    out.extend_from_slice(l.tail);
    out.extend_from_slice(&newpub);
    out.extend_from_slice(&newpriv);
    out.extend_from_slice(&code);
    Ok(out)
}

/// `cut(lib, side)` → (the library without the functions named in `side`, the library of those).
pub fn cut(lib: &[u8], side: &[String]) -> Result<(Vec<u8>, Vec<u8>), CutError> {
    if !lib.starts_with(b"MTLB") || u64_at(lib, 0x10)? != lib.len() as u64 {
        return Err(err("not a whole Metal library"));
    }
    let mut sections = [0u64; 8];
    for (k, s) in sections.iter_mut().enumerate() {
        *s = u64_at(lib, 0x18 + 8 * k)?;
    }
    let (fo, po, bo, bs) = (sections[0], sections[2], sections[6], sections[7]);
    if bo.checked_add(bs) != Some(lib.len() as u64) {
        return Err(err("its bitcode is not last"));
    }
    let fo_i = idx(fo)?;
    let n = u32_at(lib, fo_i)?;
    let mut p = fo_i + 4;
    let (mut fns, mut names) = (Vec::new(), Vec::new());
    for _ in 0..n {
        let size = idx(u64::from(u32_at(lib, p)?))?;
        let t = tags(lib, p + 4, p + size)?;
        let name = t
            .iter()
            .rev()
            .find(|(k, _)| k == b"NAME")
            .map(|(_, v)| v.clone())
            .ok_or_else(|| err("an unreadable library (KeyError NAME)"))?;
        let mut name = name;
        while name.last() == Some(&0) {
            name.pop();
        }
        names.push(String::from_utf8(name).map_err(|_| err("a function name is not UTF-8"))?);
        fns.push(t);
        p += size;
    }
    let mut missing: Vec<&String> = side.iter().filter(|s| !names.contains(s)).collect();
    missing.sort();
    if let Some(m) = missing.first() {
        return Err(CutError(format!("no function {m}")));
    }
    let layout = Layout {
        b: lib,
        fns,
        tail: slice(lib, p as u64, po),
        sections,
    };
    let base: Vec<usize> = (0..names.len())
        .filter(|&i| !side.contains(&names[i]))
        .collect();
    let rest: Vec<usize> = (0..names.len())
        .filter(|&i| side.contains(&names[i]))
        .collect();
    Ok((take(&layout, &base)?, take(&layout, &rest)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_not_a_whole_library_is_refused() {
        let names = ["main0_a".to_string()];
        assert_eq!(
            cut(b"", &names),
            Err(CutError("not a whole Metal library".into()))
        );
        assert!(cut(b"MTLB", &names).is_err());
        let mut short = b"MTLB".to_vec();
        short.resize(0x40, 0);
        assert!(cut(&short, &names).is_err());
    }
}
