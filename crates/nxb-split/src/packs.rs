//! Small archive readers: Bezel Engine (BEA), Game Freak (GFPAK), Criterion Bundle 2, Koei Tecmo (IDRK, chunked
//! zlib, SPKG), Heaps PAK and Virtuos (VBF) (`bea_members`, `gfpak_members`, `bnd2_shaders`, `idrk_shaders`,
//! `kt_shaders`, `heaps_files`, `vbf_entries`).

use crate::SplitError;
use crate::bahs::BahsVar;
use crate::glslc::glslc_shaders;
use crate::rd::{red, sl, u16le, u32le, u64le, zlib_all};
use crate::shaders::{Found, memfind, scan_blob, span};
use crate::src::Src;

type R<T> = Result<T, SplitError>;
const GLSLC: [u8; 4] = [0x91, 0x68, 0x86, 0x19];

fn has(d: &[u8], pat: &[u8]) -> bool {
    memfind(d, pat).is_some()
}

/// `bea_members`: (path, data) of each entry of a NintendoWare Bezel Engine archive.
pub fn bea_members(d: &[u8], label: &str) -> R<Vec<(String, Vec<u8>)>> {
    let (n, arr) = (u64le(d, 0x20)? as usize, u64le(d, 0x28)? as usize);
    let inline = sl(d, arr, arr + 4) == b"ASST";
    let mut o = arr;
    let mut out = Vec::new();
    for i in 0..n {
        if !inline {
            o = u64le(d, arr + 8 * i)? as usize;
        }
        if sl(d, o, o + 4) != b"ASST" {
            return red(format!("{label}: entry {i} @{o:#x} is not an ASST entry"));
        }
        let size = if inline { u32le(d, o + 4)? } else { 0x30 };
        let (csz, usz) = (u32le(d, o + 0x14)?, u32le(d, o + 0x18)?);
        let (off, nm) = (
            u64le(d, o + size - 0x10)? as usize,
            u64le(d, o + size - 8)? as usize,
        );
        if inline {
            o += size;
        }
        let path = String::from_utf8_lossy(sl(d, nm + 2, nm + 2 + u16le(d, nm)?)).into_owned();
        let mut data = sl(d, off, off + csz).to_vec();
        if sl(&data, 0, 4) == [0x28, 0xb5, 0x2f, 0xfd] {
            data = crate::zstd::decompress(&data)?;
        }
        if data.len() != usz {
            return red(format!(
                "{label}/{path}: {:#x} bytes, the entry says {usz:#x}",
                data.len()
            ));
        }
        out.push((path, data));
    }
    Ok(out)
}

/// `gfpak_members`: (`#index`, data) of each entry. Oodle entries are `Unsupported`.
pub fn gfpak_members(d: &[u8], label: &str) -> R<Vec<(String, Vec<u8>)>> {
    if sl(d, 8, 12) != [0, 0x10, 0, 0] {
        let h: String = sl(d, 8, 12).iter().map(|b| format!("{b:02x}")).collect();
        return red(format!("{label}: GFPAK version {h}, not 0x1000"));
    }
    let (n, files) = (u32le(d, 0x10)?, u64le(d, 0x18)? as usize);
    let mut out = Vec::new();
    for i in 0..n {
        let at = files + 0x18 * i;
        let (ct, usz, csz, off) = (
            u16le(d, at + 2)?,
            u32le(d, at + 4)?,
            u32le(d, at + 8)?,
            u64le(d, at + 16)? as usize,
        );
        let raw = sl(d, off, off + csz);
        let data = match ct {
            2 => crate::lz4::block_upto(raw, usz)?,
            1 => zlib_all(raw, label)?,
            0 => raw.to_vec(),
            3 => {
                return Err(SplitError::Unsupported(format!(
                    "{label}: entry {i} is Oodle Kraken"
                )));
            }
            _ => {
                return red(format!(
                    "{label}: entry {i} has compression {ct}, which this reader does not decode"
                ));
            }
        };
        if data.len() != usz {
            return red(format!(
                "{label}#{i}: {:#x} bytes, the entry says {usz:#x}",
                data.len()
            ));
        }
        out.push((format!("#{i}"), data));
    }
    Ok(out)
}

/// (resource ID, type, block, data)
type Bnd2Block = (u64, usize, usize, Vec<u8>);

fn bnd2_blocks(d: &[u8], label: &str) -> R<Vec<Bnd2Block>> {
    let w: Vec<usize> = (0..10).map(|i| u32le(d, 4 + 4 * i)).collect::<R<_>>()?;
    let (ver, cnt, eoff, flags) = (w[0], w[3], w[4], w[9]);
    let datoff = &w[5..9];
    let table_end = eoff + cnt * 0x50;
    if !(ver == 2 || ver == 3) || eoff < 0x30 || table_end > d.len() {
        return red(format!(
            "{label}: bnd2 version {ver} with {cnt} entries at {eoff:#x} is not a layout this reader knows"
        ));
    }
    let mut out = Vec::new();
    for i in 0..cnt {
        let e = eoff + i * 0x50;
        let (rid, typ) = (u64le(d, e)?, u32le(d, e + 0x44)?);
        for (k, &base) in datoff.iter().enumerate() {
            let n = u32le(d, e + 0x20 + 4 * k)? & 0x0fff_ffff;
            let expected = u32le(d, e + 0x10 + 4 * k)? & 0x0fff_ffff;
            let off = u32le(d, e + 0x30 + 4 * k)?;
            let where_ = format!("{label}: bnd2 resource {rid:016x} block {k}");
            if n == 0 {
                if expected != 0 {
                    return red(format!(
                        "{where_}: declares {expected} bytes and has none stored"
                    ));
                }
                continue;
            }
            let start = base + off;
            if start < table_end {
                return red(format!(
                    "{where_}: its data at {start:#x} starts inside the resource table"
                ));
            }
            let mut raw = span(d, start, n, &where_)?.to_vec();
            if flags & 1 != 0 {
                raw = crate::zlibx::zlib_stream(&raw, expected + 1).ok_or_else(|| {
                    SplitError::Red(format!("{where_}: damaged or incomplete zlib stream"))
                })?;
                if raw.len() != expected {
                    return red(format!(
                        "{where_}: incomplete zlib stream or decompressed size differs from declared size {expected}"
                    ));
                }
            } else if raw.len() != expected {
                return red(format!(
                    "{where_}: stored size differs from declared size {expected}"
                ));
            }
            out.push((rid, typ, k, raw));
        }
    }
    Ok(out)
}

/// `bnd2_shaders`: GLSLC outputs in the resources of a Criterion Bundle 2.
pub fn bnd2_shaders(d: &[u8], label: &str) -> R<Vec<Found>> {
    let mut out = Vec::new();
    for (rid, typ, k, r) in bnd2_blocks(d, label)? {
        if has(&r, &GLSLC) {
            out.extend(glslc_shaders(
                &r,
                &format!("{label}/{rid:016x}.{typ:x}.{k}"),
                false,
                &[],
            )?);
        }
    }
    Ok(out)
}

/// Window of a source: `d[a:b]` as Python's mmap slices it.
fn win(s: &dyn Src, a: u64, n: u64) -> R<Vec<u8>> {
    s.read_up(a, usize::try_from(n).unwrap_or(usize::MAX))
}

/// `idrk_data`: an entry's data (`head`: only its first bytes).
fn idrk_data(s: &dyn Src, a: u64, b: u64, flags: usize, head: usize) -> R<Vec<u8>> {
    let (new, lz4b) = (flags & 0x40_0000 != 0, flags & 0x50_0000 == 0x50_0000);
    if flags & 0x50_0000 == 0 {
        return win(
            s,
            a,
            if head != 0 {
                b.min(a + head as u64).saturating_sub(a)
            } else {
                b.saturating_sub(a)
            },
        );
    }
    let (mut a, mut out) = (a, Vec::new());
    while a + 4 <= b {
        let cs;
        if new {
            cs = u16le(&win(s, a, 2)?, 0)? as u64;
            a += 10;
        } else {
            cs = u32le(&win(s, a, 4)?, 0)? as u64;
            a += 4;
        }
        if cs == 0 {
            break;
        }
        let r;
        if lz4b {
            r = crate::lz4::block_upto(&win(s, a, cs)?, 0x4000)?;
            if head != 0 {
                return Ok(r[..head.min(r.len())].to_vec());
            }
        } else if head != 0 {
            let z = win(s, a, cs.min(1024))?;
            return crate::inflate::inflate_head(&z, head, true)
                .ok_or_else(|| SplitError::Red("zlib.error: bad header".into()));
        } else {
            r = zlib_all(&win(s, a, cs)?, "IDRK")?;
        }
        out.extend(r);
        a += cs;
    }
    Ok(out)
}

/// `idrk_shaders`: GLSLC outputs in the G2S and DOK containers of a Koei Tecmo RDB data file.
pub fn idrk_shaders(s: &dyn Src, label: &str) -> R<Vec<Found>> {
    let mut out = Vec::new();
    let (len, mut o) = (s.len(), 0u64);
    while o + 0x30 <= len {
        if win(s, o, 8)? != b"IDRK0000" {
            match s.find(b"IDRK0000", o)? {
                Some(p) => o = p,
                None => break,
            }
        }
        let h = win(s, o + 8, 0x28)?;
        let (esz, csz, flags) = (u64le(&h, 0)?, u64le(&h, 8)?, u32le(&h, 0x24)?);
        if esz == 0 {
            return red(format!("{label}: an IDRK entry of length 0 at {o:#x}"));
        }
        let a = (o + esz).wrapping_sub(csz);
        let hd = idrk_data(s, a, o + esz, flags, 4)?;
        if hd == b"_S2G" || hd == b"_DOK" {
            let data = idrk_data(s, a, o + esz, flags, 0)?;
            out.extend(glslc_shaders(
                &data,
                &format!("{label}#IDRK@{a:#x}"),
                false,
                &[],
            )?);
        }
        o += esz;
    }
    Ok(out)
}

/// `kt_gz`: Koei Tecmo's chunked zlib, or `None` when `d` is not in that form.
pub fn kt_gz(d: &[u8]) -> R<Option<Vec<u8>>> {
    if d.len() < 16 {
        return Ok(None);
    }
    let (cs, n, tot) = (u32le(d, 0)?, u32le(d, 4)?, u32le(d, 8)?);
    if cs < 0x10000 || cs & (cs - 1) != 0 || n == 0 || n != tot.div_ceil(cs) || 12 + 4 * n > d.len()
    {
        return Ok(None);
    }
    let mut o = (12 + 4 * n + 0x7f) & !0x7f;
    let mut out = Vec::new();
    for i in 0..n {
        let s = u32le(d, 12 + 4 * i)?;
        if o + s > d.len() {
            return Ok(None);
        }
        if s == cs.min(tot - i * cs) {
            out.extend_from_slice(&d[o..o + s]);
        } else {
            let ln = u32le(d, o)?;
            match crate::zlibx::zlib_stream(sl(d, o + 4, o + 4 + ln), 1 << 31) {
                Some(z) => out.extend(z),
                None => return Ok(None),
            }
        }
        o = (o + s + 0x7f) & !0x7f;
    }
    Ok(Some(out))
}

/// `gust_gz`: Gust's chunked zlib, or `None`.
pub fn gust_gz(d: &[u8]) -> R<Option<Vec<u8>>> {
    let (mut o, mut out) = (0usize, Vec::new());
    while o + 4 <= d.len() {
        let n = u32le(d, o)?;
        o += 4;
        if n == 0 {
            return Ok(if !out.is_empty() && o == d.len() {
                Some(out)
            } else {
                None
            });
        }
        if o + n > d.len() || d[o] != 0x78 {
            return Ok(None);
        }
        match crate::zlibx::zlib_stream_used(&d[o..o + n], 1 << 31) {
            Some((z, used)) if used == n => out.extend(z),
            _ => return Ok(None),
        }
        o += n;
    }
    Ok(None)
}

/// `kt_shaders`: GLSLC outputs in a (chunked zlib) file or entry, and BNSH in its SARC archives.
pub fn kt_shaders(d: &[u8], label: &str, out: &mut Vec<Found>, var: &mut BahsVar) -> R<()> {
    let z = kt_gz(d)?;
    let d = z.as_deref().unwrap_or(d);
    if has(d, &GLSLC) {
        out.extend(glslc_shaders(d, label, false, &[])?);
    }
    if sl(d, 0, 4) == b"SARC" || has(d, b"BNSH\0\0\0\0") {
        scan_blob(d, label, out, var, 0)?;
    }
    Ok(())
}

/// `heaps_pak_files`: (path, offset, size) of the files of a Heaps PAK, `None` when it is not one.
pub fn heaps_files(s: &dyn Src) -> R<Option<Vec<(String, u64, usize)>>> {
    let head = s.read_up(0, 12)?;
    if head.len() < 12 || &head[..3] != b"PAK" || head[3] > 1 {
        return Ok(None);
    }
    let (size, dsize) = (u32le(&head, 4)? as i32, u32le(&head, 8)? as i32);
    if !(12 < size && size <= 1 << 28) || dsize < 0 {
        return Ok(None);
    }
    let size = size as usize;
    let h = s.read_up(12, size - 12)?;
    let mut out = Vec::new();
    fn entry(
        h: &[u8],
        mut p: usize,
        path: &str,
        size: usize,
        out: &mut Vec<(String, u64, usize)>,
        depth: usize,
    ) -> Option<usize> {
        if depth > 900 {
            return None;
        }
        let n = usize::from(*h.get(p)?);
        let name = std::str::from_utf8(h.get(p + 1..p + 1 + n)?).ok()?;
        let fl = *h.get(p + 1 + n)?;
        p += n + 2;
        let path = if name.is_empty() {
            path.to_string()
        } else {
            format!("{path}/{name}")
        };
        if fl & 1 != 0 {
            let cnt = i32::from_le_bytes(h.get(p..p + 4)?.try_into().ok()?);
            p += 4;
            for _ in 0..cnt.max(0) {
                p = entry(h, p, &path, size, out, depth + 1)?;
            }
            return Some(p);
        }
        let pos = if fl & 2 != 0 {
            let v = f64::from_le_bytes(h.get(p..p + 8)?.try_into().ok()?);
            p += 8;
            v as i64
        } else {
            let v = i32::from_le_bytes(h.get(p..p + 4)?.try_into().ok()?);
            p += 4;
            i64::from(v)
        };
        let n = i32::from_le_bytes(h.get(p..p + 4)?.try_into().ok()?);
        h.get(p + 4..p + 8)?;
        out.push((path, (size as i64 + pos) as u64, n.max(0) as usize));
        Some(p + 8)
    }
    let Some(end) = entry(&h, if head[3] != 0 { 64 } else { 0 }, "", size, &mut out, 0) else {
        return Ok(None);
    };
    Ok((sl(&h, end, end + 4) == b"DATA").then_some(out))
}

fn b64(b: &[u8]) -> R<Vec<u8>> {
    let val = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        _ => 63,
    };
    let data: Vec<u8> = b.iter().copied().take_while(|&c| c != b'=').collect();
    let pad = b.len() - data.len();
    if !(data.len() + pad).is_multiple_of(4) {
        return red("binascii.Error: Incorrect padding");
    }
    let mut out = Vec::new();
    for c in data.chunks(4) {
        let v: Vec<u32> = c.iter().map(|&x| u32::from(val(x))).collect();
        let w = v.iter().fold(0u32, |a, &x| a << 6 | x) << (6 * (4 - v.len()));
        for k in 0..(v.len() * 6 / 8) {
            out.push((w >> (16 - 8 * k)) as u8);
        }
    }
    Ok(out)
}

/// `heaps_shaders`: GLSLC outputs in the `//BIN=<base64>#` comments of Heaps' shader cache.
pub fn heaps_shaders(d: &[u8], label: &str) -> R<Vec<Found>> {
    let mut found = Vec::new();
    let mut i = 0;
    while let Some(p) = memfind(sl(d, i, d.len()), b"//BIN=").map(|p| p + i) {
        let mut e = p + 6;
        while d
            .get(e)
            .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'+' || *c == b'/')
        {
            e += 1;
        }
        let ok = e > p + 6;
        let mut q = e;
        while q < e + 2 && d.get(q) == Some(&b'=') {
            q += 1;
        }
        if ok && d.get(q) == Some(&b'#') {
            let bin = b64(&d[p + 6..q])?;
            found.extend(glslc_shaders(&bin, &format!("{label}@{p:#x}"), false, &[])?);
            i = q + 1;
        } else {
            i = p + 1;
        }
    }
    if found.is_empty() {
        return red(format!("{label}: no GLSLC output in //BIN= comments"));
    }
    Ok(found)
}

/// (name, size, offset, block lengths)
pub type VbfEntry = (String, u64, u64, Vec<usize>);

/// `vbf.entries`: (name, size, offset, block lengths) of a Virtuos SRYK archive.
pub fn vbf_entries(s: &dyn Src) -> R<Vec<VbfEntry>> {
    let bad = |m: &str| SplitError::Red(format!("ValueError: {m}"));
    let head = s.read_up(0, 16)?;
    if head.len() != 16 || &head[..4] != b"SRYK" {
        return Err(bad("not a VBF archive"));
    }
    let (hlen, count) = (u32le(&head, 4)?, u64le(&head, 8)? as usize);
    if !(count.checked_mul(64).is_some_and(|c| 16 + c + 4 <= hlen) && hlen <= 256 << 20) {
        return Err(bad("invalid VBF header length"));
    }
    let h = s.read_up(0, hlen)?;
    if h.len() != hlen {
        return Err(bad("truncated VBF header"));
    }
    let strings = 16 + count * 64;
    let slen = u32le(&h, strings)?;
    let blocks = strings + slen;
    if slen < 4 || blocks > hlen {
        return Err(bad("invalid VBF string table"));
    }
    let mut out = Vec::new();
    for i in 0..count {
        let r = 16 + count * 32 + i * 32;
        let (bi, size, offset, name) = (
            u32le(&h, r)?,
            u64le(&h, r + 8)?,
            u64le(&h, r + 16)?,
            u64le(&h, r + 24)? as usize,
        );
        if offset == u64::MAX {
            continue;
        }
        let start = strings + 4 + name;
        if !(strings + 4 <= start && start < blocks) {
            return Err(bad("invalid VBF name offset"));
        }
        let end =
            memfind(&h[start..blocks], b"\0").ok_or_else(|| bad("unterminated VBF name"))? + start;
        let n = size.div_ceil(65536) as usize;
        if blocks + (bi + n) * 2 > hlen || offset < hlen as u64 {
            return Err(bad("invalid VBF block range"));
        }
        let lengths = (0..n)
            .map(|k| u16le(&h, blocks + bi * 2 + 2 * k))
            .collect::<R<Vec<_>>>()?;
        let name =
            String::from_utf8(h[start..end].to_vec()).map_err(|_| bad("name is not UTF-8"))?;
        out.push((name, size, offset, lengths));
    }
    Ok(out)
}

/// `vbf.read`: one entry's data.
pub fn vbf_read(s: &dyn Src, e: &VbfEntry) -> R<Vec<u8>> {
    let (name, size0, offset, lengths) = e;
    let mut size = *size0;
    let (mut at, mut chunks) = (*offset, Vec::new());
    for &l in lengths {
        let n = size.min(65536) as usize;
        let l = if l == 0 { 65536 } else { l };
        let mut data = s.read_up(at, l)?;
        if data.len() != l {
            return red(format!("ValueError: {name}: truncated VBF block"));
        }
        at += l as u64;
        if l != n {
            data = crate::zlibx::zlib_stream_used(&data, n + 1)
                .map(|x| x.0)
                .ok_or_else(|| {
                    SplitError::Red(format!("ValueError: {name}: invalid VBF compressed block"))
                })?;
        }
        if data.len() != n {
            return red(format!("ValueError: {name}: VBF block size mismatch"));
        }
        chunks.extend(data);
        size -= n as u64;
    }
    Ok(chunks)
}
