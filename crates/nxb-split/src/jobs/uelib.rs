//! Unreal Engine 5.6 IoStore shader libraries (`uepak.py` `city64`, `io_library_name`, `io_archive`, `io_tocs`,
//! `archive_names` and the IoStore part of `job`). Each library chunk becomes a version 2 shader code archive with
//! every shader decoded. The container blocks and the shader groups are Oodle-compressed, so the game's decoder
//! ([`crate::oodle`]) must be installed.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::R;
use super::uepak::{Rd, ve};
use super::uetoc::{IO_SHADER_LIBRARY, Toc, mount_order};
use crate::rd::sl;
use crate::src::Src;

const K0: u64 = 0xc3a5_c85c_97cb_3127;
const K1: u64 = 0xb492_b66f_be98_f273;
const K2: u64 = 0x9ae1_6a3b_2f90_404f;
const KMUL: u64 = 0x9ddf_ea08_eb38_2d69;
/// The largest shader group: a longer one is damage.
const GROUP_MAX: u32 = 1 << 30;
/// The largest manifest read.
const MANIFEST_MAX: usize = 1 << 26;

fn f64_(s: &[u8], i: usize) -> u64 {
    s.get(i..i.saturating_add(8))
        .and_then(|b| b.try_into().ok())
        .map_or(0, u64::from_le_bytes)
}

fn f32_(s: &[u8], i: usize) -> u64 {
    s.get(i..i.saturating_add(4))
        .and_then(|b| b.try_into().ok())
        .map_or(0, |b| u64::from(u32::from_le_bytes(b)))
}

fn h16(u: u64, v: u64, mul: u64) -> u64 {
    let mut a = (u ^ v).wrapping_mul(mul);
    a ^= a >> 47;
    let mut b = (v ^ a).wrapping_mul(mul);
    b ^= b >> 47;
    b.wrapping_mul(mul)
}

fn weak(s: &[u8], i: usize, a: u64, b: u64) -> (u64, u64) {
    let (w, x, y, z) = (f64_(s, i), f64_(s, i + 8), f64_(s, i + 16), f64_(s, i + 24));
    let a = a.wrapping_add(w);
    let b = b.wrapping_add(a).wrapping_add(z).rotate_right(21);
    let c = a;
    let a = a.wrapping_add(x).wrapping_add(y);
    let b = b.wrapping_add(a.rotate_right(44));
    (a.wrapping_add(z), b.wrapping_add(c))
}

fn len16(s: &[u8]) -> u64 {
    let n = s.len();
    if n >= 8 {
        let mul = K2.wrapping_add(n as u64 * 2);
        let a = f64_(s, 0).wrapping_add(K2);
        let b = f64_(s, n - 8);
        let c = b.rotate_right(37).wrapping_mul(mul).wrapping_add(a);
        let d = a.rotate_right(25).wrapping_add(b).wrapping_mul(mul);
        return h16(c, d, mul);
    }
    if n >= 4 {
        let mul = K2.wrapping_add(n as u64 * 2);
        return h16(
            (n as u64).wrapping_add(f32_(s, 0) << 3),
            f32_(s, n - 4),
            mul,
        );
    }
    if n > 0 {
        let y = u64::from(s[0]) + (u64::from(s[n >> 1]) << 8);
        let z = n as u64 + (u64::from(s[n - 1]) << 2);
        let v = y.wrapping_mul(K2) ^ z.wrapping_mul(K0);
        return (v ^ (v >> 47)).wrapping_mul(K2);
    }
    K2
}

/// `city64`: Google's CityHash64 1.1, the one Unreal's Core links.
pub(super) fn city64(s: &[u8]) -> u64 {
    let n = s.len();
    if n <= 16 {
        return len16(s);
    }
    let mul = K2.wrapping_add(n as u64 * 2);
    if n <= 32 {
        let a = f64_(s, 0).wrapping_mul(K1);
        let b = f64_(s, 8);
        let c = f64_(s, n - 8).wrapping_mul(mul);
        let d = f64_(s, n - 16).wrapping_mul(K2);
        let u = a
            .wrapping_add(b)
            .rotate_right(43)
            .wrapping_add(c.rotate_right(30))
            .wrapping_add(d);
        let v = a
            .wrapping_add(b.wrapping_add(K2).rotate_right(18))
            .wrapping_add(c);
        return h16(u, v, mul);
    }
    if n <= 64 {
        let a = f64_(s, 0).wrapping_mul(K2);
        let b = f64_(s, 8);
        let c = f64_(s, n - 24);
        let d = f64_(s, n - 32);
        let e = f64_(s, 16).wrapping_mul(K2);
        let f = f64_(s, 24).wrapping_mul(9);
        let g = f64_(s, n - 8);
        let h = f64_(s, n - 16).wrapping_mul(mul);
        let u = a
            .wrapping_add(g)
            .rotate_right(43)
            .wrapping_add(b.rotate_right(30).wrapping_add(c).wrapping_mul(9));
        let v = (a.wrapping_add(g) ^ d).wrapping_add(f).wrapping_add(1);
        let w = u
            .wrapping_add(v)
            .wrapping_mul(mul)
            .swap_bytes()
            .wrapping_add(h);
        let x = e.wrapping_add(f).rotate_right(42).wrapping_add(c);
        let y = v
            .wrapping_add(w)
            .wrapping_mul(mul)
            .swap_bytes()
            .wrapping_add(g)
            .wrapping_mul(mul);
        let z = e.wrapping_add(f).wrapping_add(c);
        let a = x
            .wrapping_add(z)
            .wrapping_mul(mul)
            .wrapping_add(y)
            .swap_bytes()
            .wrapping_add(b);
        let b = z
            .wrapping_add(a)
            .wrapping_mul(mul)
            .wrapping_add(d)
            .wrapping_add(h);
        let b = (b ^ (b >> 47)).wrapping_mul(mul);
        return b.wrapping_add(x);
    }
    let mut x = f64_(s, n - 40);
    let mut y = f64_(s, n - 16).wrapping_add(f64_(s, n - 56));
    let mut z = h16(
        f64_(s, n - 48).wrapping_add(n as u64),
        f64_(s, n - 24),
        KMUL,
    );
    let mut v = weak(s, n - 64, n as u64, z);
    let mut w = weak(s, n - 32, y.wrapping_add(K1), x);
    x = x.wrapping_mul(K1).wrapping_add(f64_(s, 0));
    let (mut left, mut i) = ((n - 1) & !63, 0usize);
    while left > 0 {
        x = x
            .wrapping_add(y)
            .wrapping_add(v.0)
            .wrapping_add(f64_(s, i + 8))
            .rotate_right(37)
            .wrapping_mul(K1);
        y = y
            .wrapping_add(v.1)
            .wrapping_add(f64_(s, i + 48))
            .rotate_right(42)
            .wrapping_mul(K1);
        x ^= w.1;
        y = y.wrapping_add(v.0).wrapping_add(f64_(s, i + 40));
        z = z.wrapping_add(w.0).rotate_right(33).wrapping_mul(K1);
        v = weak(s, i, v.1.wrapping_mul(K1), x.wrapping_add(w.0));
        w = weak(
            s,
            i + 32,
            z.wrapping_add(w.1),
            y.wrapping_add(f64_(s, i + 16)),
        );
        std::mem::swap(&mut z, &mut x);
        i += 64;
        left -= 64;
    }
    h16(
        h16(v.0, w.0, KMUL)
            .wrapping_add((y ^ (y >> 47)).wrapping_mul(K1))
            .wrapping_add(z),
        h16(v.1, w.1, KMUL).wrapping_add(x),
        KMUL,
    )
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// `archive_names`: the shader code archive names (`ShaderArchive-<library>-<format>`) that the manifests list
/// (`Manifest_*.txt` at the top of the RomFS), sorted, each once.
pub(super) fn archive_names(texts: &[Vec<u8>]) -> Vec<String> {
    const HEAD: &str = "ShaderArchive-";
    const TAIL: &str = ".ushaderbytecode";
    let mut out = std::collections::BTreeSet::new();
    for t in texts {
        let s = String::from_utf8_lossy(t);
        let mut p = 0;
        while let Some(q) = s.get(p..).and_then(|x| x.find(HEAD)).map(|q| p + q) {
            let a = q + HEAD.len();
            let run = s[a..]
                .char_indices()
                .find(|&(_, c)| !(c.is_alphanumeric() || c == '_' || c == '-'))
                .map_or(s.len() - a, |x| x.0);
            if run > 0 && s[a + run..].starts_with(TAIL) {
                out.insert(s[q..a + run].to_string());
                p = a + run + TAIL.len();
            } else {
                p = q + 1;
            }
        }
    }
    out.into_iter().collect()
}

/// `io_library_name`: a shader library chunk's archive name. Unreal makes the chunk ID from CityHash64 of
/// `<library>-<format>` in lower case and UTF-16. A chunk that no name in `names` matches is named by its ID.
fn io_library_name(id: &[u8; 12], names: &[String]) -> String {
    for n in names {
        let u: Vec<u8> = n
            .get(14..)
            .unwrap_or("")
            .to_lowercase()
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        if city64(&u).to_le_bytes()[..] == id[..8] {
            return n.clone();
        }
    }
    format!("ShaderArchive-IoStore-{}", hex(&id[..8]))
}

/// An IoStore container with its `.ucas` partitions.
struct Opened {
    toc: Toc,
    ucas: Vec<Option<Arc<dyn Src>>>,
}

fn words(b: &[u8], n: usize) -> Vec<u64> {
    b.chunks_exact(n)
        .map(|c| c.iter().rev().fold(0u64, |a, &x| a << 8 | u64::from(x)))
        .collect()
}

/// `io_archive`: library chunk `i` of `t` as a version 2 shader code archive with every shader decoded, and its
/// shader count. `chunk` gives the container (in `all`) and the index of each chunk ID, the newest container first.
fn io_archive(
    t: &Opened,
    i: usize,
    chunk: &HashMap<[u8; 12], (usize, usize)>,
    all: &[Opened],
) -> R<(Vec<u8>, usize)> {
    let rel = t.toc.rel();
    let what = format!("{rel}: shader library chunk {i}");
    let b = t.toc.read(i, &t.ucas)?;
    let mut r = Rd::new(&b, 0, what.clone());
    if r.le(4)? != 1 {
        return ve(format!("{what} is not version 1"));
    }
    let nm = r.le(4)? as usize;
    let maph = r.raw(nm.saturating_mul(20))?;
    let nsh = r.le(4)? as usize;
    let shh = r.raw(nsh.saturating_mul(20))?;
    let ng = r.le(4)? as usize;
    let gids = r.raw(ng.saturating_mul(12))?;
    let nmaps = r.le(4)? as usize;
    let maps = r.raw(nmaps.saturating_mul(8))?;
    let n = r.le(4)? as usize;
    let ents = words(r.raw(n.saturating_mul(8))?, 8);
    let ngr = r.le(4)? as usize;
    let groups = words(r.raw(ngr.saturating_mul(4).saturating_mul(4))?, 4);
    let nidx = r.le(4)? as usize;
    let idx_raw = r.raw(nidx.saturating_mul(4))?;
    let idx = words(idx_raw, 4);
    if r.pos() != b.len() || n != nsh || ng != ngr {
        return ve(format!("{what} does not parse"));
    }
    let mut code: Vec<Option<Vec<u8>>> = vec![None; n];
    for (g, (gid, e)) in gids
        .chunks_exact(12)
        .zip(groups.chunks_exact(4))
        .enumerate()
    {
        let (first, cnt, raw, stored) = (e[0] as usize, e[1] as usize, e[2] as u32, e[3] as u32);
        let id: [u8; 12] = gid.try_into().unwrap_or([0; 12]);
        let owner = chunk.get(&id).and_then(|&(o, k)| Some((all.get(o)?, k)));
        let Some((o, k)) = owner else {
            return ve(format!(
                "{rel}: shader group {g} ({}) is in no container under the RomFS",
                hex(gid)
            ));
        };
        if raw > GROUP_MAX || stored > GROUP_MAX {
            return ve(format!("{rel}: shader group {g} of {raw} bytes"));
        }
        let mut d = o.toc.read(k, &o.ucas)?;
        if d.len() != stored as usize {
            return ve(format!(
                "{rel}: shader group {g} is {} bytes, its entry says {stored}",
                d.len()
            ));
        }
        if stored < raw {
            d = crate::oodle::decode(&d, raw as usize, &format!("{rel}: shader group {g}"))?;
        }
        let a = first.min(idx.len());
        let mut mine: Vec<(u64, usize)> = idx[a..first.saturating_add(cnt).clamp(a, idx.len())]
            .iter()
            .map(|&s| {
                let s = s as usize;
                ents.get(s).map(|&x| (x >> 34, s)).ok_or_else(|| {
                    crate::SplitError::Red(format!("IndexError: {what}: shader {s}"))
                })
            })
            .collect::<R<_>>()?;
        mine.sort_unstable();
        for (j, &(at, s)) in mine.iter().enumerate() {
            let nxt = mine.get(j + 1).map_or(u64::from(raw), |x| x.0);
            if (ents[s] >> 4) & 0x3FFF_FFFF != g as u64 {
                return ve(format!(
                    "{rel}: shader {s} is listed in group {g} but names another"
                ));
            }
            let at = usize::try_from(at).unwrap_or(usize::MAX);
            let nxt = usize::try_from(nxt).unwrap_or(usize::MAX);
            code[s] = Some(sl(&d, at, nxt).to_vec());
        }
    }
    let missing = code.iter().filter(|c| c.is_none()).count();
    if missing > 0 {
        return ve(format!("{rel}: {missing} shaders in no group"));
    }
    let mut out = Vec::new();
    out.extend(2u32.to_le_bytes());
    out.extend((nm as u32).to_le_bytes());
    out.extend_from_slice(maph);
    out.extend((n as u32).to_le_bytes());
    out.extend_from_slice(shh);
    out.extend((nmaps as u32).to_le_bytes());
    for m in maps.chunks_exact(8) {
        out.extend_from_slice(m);
        out.extend([0u8; 8]);
    }
    out.extend((n as u32).to_le_bytes());
    let mut off = 0u64;
    for (e, c) in ents.iter().zip(code.iter().flatten()) {
        let len = c.len() as u32;
        out.extend(off.to_le_bytes());
        out.extend(len.to_le_bytes());
        out.extend(len.to_le_bytes());
        out.push((e & 0xF) as u8);
        off += u64::from(len);
    }
    out.extend(0u32.to_le_bytes());
    out.extend((nidx as u32).to_le_bytes());
    out.extend_from_slice(idx_raw);
    for c in code.iter().flatten() {
        out.extend_from_slice(c);
    }
    Ok((out, n))
}

/// A shader library of the RomFS as a decoded archive.
pub(super) struct IoLibrary {
    /// Its archive name (`<io_library_name>.ushaderbytecode`).
    pub(super) name: String,
    /// Its container's TOC, relative to the RomFS root.
    pub(super) rel: String,
    /// `chunk <chunk ID in hex>`.
    pub(super) path: String,
    /// Its shader count.
    pub(super) shaders: usize,
    /// The archive.
    pub(super) bytes: Vec<u8>,
}

/// The IoStore part of `job`: the shader libraries of every IoStore container in `files` (path relative to the
/// RomFS root, the file), each once and from the newest container. `key` gives the AES key of the game's paks when a
/// container is encrypted.
pub(super) fn io_libraries(
    files: &[(String, Arc<dyn Src>)],
    key: &mut dyn FnMut() -> R<Option<[u8; 32]>>,
) -> R<Vec<IoLibrary>> {
    let mut tocs: Vec<Toc> = files
        .iter()
        .filter(|f| f.0.ends_with(".utoc"))
        .filter_map(|(rel, s)| Toc::load(s.as_ref(), rel).ok())
        .collect();
    tocs.sort_by_cached_key(|t| mount_order(t.rel()));
    let Some(lib) = tocs.iter().find(|t| t.of_type(IO_SHADER_LIBRARY)) else {
        return Ok(Vec::new());
    };
    if !crate::oodle::installed() {
        return ve(format!(
            "{}: IoStore shader libraries need the game's decoder (decode)",
            lib.rel()
        ));
    }
    if tocs.iter().any(|t| t.encrypted) {
        let k = key()?;
        for t in tocs.iter_mut().filter(|t| t.encrypted) {
            t.set_key(k);
        }
    }
    let by_rel: HashMap<&str, &Arc<dyn Src>> = files.iter().map(|f| (f.0.as_str(), &f.1)).collect();
    let all: Vec<Opened> = tocs
        .into_iter()
        .map(|toc| {
            let ucas = toc
                .ucas_paths()
                .iter()
                .map(|p| by_rel.get(p.as_str()).map(|s| Arc::clone(s)))
                .collect();
            Opened { toc, ucas }
        })
        .collect();
    let manifests: Vec<Vec<u8>> = files
        .iter()
        .filter(|f| !f.0.contains('/') && f.0.starts_with("Manifest_") && f.0.ends_with(".txt"))
        .map(|f| f.1.as_ref().read_up(0, MANIFEST_MAX))
        .collect::<R<_>>()?;
    let names = archive_names(&manifests);
    let mut chunk: HashMap<[u8; 12], (usize, usize)> = HashMap::new();
    for (o, t) in all.iter().enumerate().rev() {
        for (k, c) in t.toc.ids.iter().enumerate() {
            chunk.entry(*c).or_insert((o, k));
        }
    }
    let (mut seen, mut out) = (HashSet::new(), Vec::new());
    for t in all.iter().rev() {
        for i in t.toc.chunks_of(IO_SHADER_LIBRARY) {
            let id = t.toc.ids[i];
            if !seen.insert(id) {
                continue;
            }
            let (bytes, shaders) = io_archive(t, i, &chunk, &all)?;
            out.push(IoLibrary {
                name: format!("{}.ushaderbytecode", io_library_name(&id, &names)),
                rel: t.toc.rel().to_string(),
                path: format!("chunk {}", hex(&id)),
                shaders,
                bytes,
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
