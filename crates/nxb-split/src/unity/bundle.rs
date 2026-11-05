//! UnityFS bundles and serialized file detection (`bundle_files`, `is_serialized`, `common_strings`).

use std::collections::HashMap;

use crate::SplitError;
use crate::rd::{red, sl};

type R<T> = Result<T, SplitError>;

fn be(d: &[u8], o: usize, n: usize) -> R<u64> {
    let b = d.get(o..o.wrapping_add(n)).ok_or_else(|| {
        SplitError::Red("struct.error: unpack_from requires a buffer of more bytes".into())
    })?;
    Ok(b.iter().fold(0u64, |a, &x| a << 8 | u64::from(x)))
}

fn cstr(d: &[u8], p: usize) -> R<(String, usize)> {
    let z = d
        .get(p..)
        .and_then(|x| x.iter().position(|&b| b == 0))
        .ok_or_else(|| SplitError::Red("ValueError: substring not found".into()))?;
    Ok((
        String::from_utf8_lossy(&d[p..p + z]).into_owned(),
        p + z + 1,
    ))
}

/// `_decomp`: one block (0 stored, 1 LZMA, 2 and 3 LZ4).
fn decomp(raw: &[u8], size: usize, kind: u64) -> R<Vec<u8>> {
    match kind {
        0 => Ok(raw.to_vec()),
        1 => super::lzma::decompress(raw, size),
        2 | 3 => crate::lz4::block_upto(raw, size),
        k => red(format!("UnityFS: unknown compression {k}")),
    }
}

/// `bundle_files`: (path, bytes) of each file of a UnityFS bundle.
pub fn bundle_files(d: &[u8]) -> R<Vec<(String, Vec<u8>)>> {
    if sl(d, 0, 8) != b"UnityFS\0" {
        return red("ValueError: not UnityFS");
    }
    let fmt = be(d, 8, 4)?;
    let (_, p) = cstr(d, 12)?;
    let (_, mut p) = cstr(d, p)?;
    let (_size, csz, usz, flags) = (
        be(d, p, 8)?,
        be(d, p + 8, 4)? as usize,
        be(d, p + 12, 4)? as usize,
        be(d, p + 16, 4)?,
    );
    p += 20;
    if fmt >= 7 {
        p = (p + 15) & !15;
    }
    let info_raw = if flags & 0x80 != 0 {
        sl(d, d.len().saturating_sub(csz), d.len()).to_vec()
    } else {
        let v = sl(d, p, p + csz).to_vec();
        p += csz;
        v
    };
    let info = decomp(&info_raw, usz, flags & 0x3f)?;
    let mut q = 16;
    let nb = be(&info, q, 4)? as u32 as i32;
    q += 4;
    let mut blocks = Vec::new();
    for _ in 0..nb.max(0) {
        blocks.push((
            be(&info, q, 4)? as usize,
            be(&info, q + 4, 4)? as usize,
            be(&info, q + 8, 2)?,
        ));
        q += 10;
    }
    let nn = be(&info, q, 4)? as u32 as i32;
    q += 4;
    let mut nodes = Vec::new();
    for _ in 0..nn.max(0) {
        let (off, sz) = (be(&info, q, 8)? as usize, be(&info, q + 8, 8)? as usize);
        q += 20;
        let (name, nq) = cstr(&info, q)?;
        q = nq;
        nodes.push((off, sz, name));
    }
    if flags & 0x200 != 0 {
        p = (p + 15) & !15;
    }
    let mut out = Vec::new();
    for (u, c, f) in blocks {
        out.extend(decomp(sl(d, p, p + c), u, f & 0x3f)?);
        p += c;
    }
    Ok(nodes
        .into_iter()
        .map(|(off, sz, name)| (name, sl(&out, off, off + sz).to_vec()))
        .collect())
}

/// `is_serialized`: does it look like a serialized file (format 17 to 22).
pub fn is_serialized(d: &[u8]) -> bool {
    if d.len() < 48 {
        return false;
    }
    let ver = be(d, 8, 4).unwrap_or(0);
    if !(17..=22).contains(&ver) || d[16] > 1 {
        return false;
    }
    let (msz, fsz, doff) = if ver >= 22 {
        (
            be(d, 20, 4).unwrap_or(0),
            be(d, 24, 8).unwrap_or(0),
            be(d, 32, 8).unwrap_or(0),
        )
    } else {
        (
            be(d, 0, 4).unwrap_or(0),
            be(d, 4, 4).unwrap_or(0),
            be(d, 12, 4).unwrap_or(0),
        )
    };
    fsz == d.len() as u64 && 0 < msz && msz < fsz && doff <= fsz
}

/// `common_strings`: Unity's common string table in the game's main.
pub fn common_strings(image: &[u8]) -> R<HashMap<u32, String>> {
    let Some(a) = crate::shaders::memfind(image, b"AABB\0AnimationClip\0AnimationCurve\0") else {
        return red("ValueError: no Unity common string table in main (AABB\\0AnimationClip...)");
    };
    let (mut tab, mut p) = (HashMap::new(), a);
    loop {
        let Some(z) = image.get(p..).and_then(|x| x.iter().position(|&b| b == 0)) else {
            return red("ValueError: substring not found");
        };
        if z == 0 {
            break;
        }
        let s = &image[p..p + z];
        if !s.iter().all(|&c| (0x20..0x7f).contains(&c)) {
            break;
        }
        tab.insert((p - a) as u32, s.iter().map(|&c| c as char).collect());
        p += z + 1;
    }
    Ok(tab)
}
