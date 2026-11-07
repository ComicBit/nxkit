//! GLSL sources in game files: Media.Vision's MVGL archives (Digimon Story Cyber Sleuth) with their doboz compression, and
//! the `#version` strings in an NSO's read-only segment. The format notes follow the reference's `dm/gfx/tools/mvgl.py`.
//!
//! An MVGL archive is little-endian. At 0x00: `MDB1`, u16 tree entries, u16 name entries, u32 data entries, u32 data start,
//! u32 archive size. Then the tree entries (8 bytes each, the u16 at +2 is the data index, 0xffff for inner nodes), the name
//! entries (0x40 bytes each by tree index: a 4-byte extension padded with spaces or NULs, then a 0x3c-byte name) and the data
//! entries (12 bytes each: u32 offset from the data start, u32 length, u32 stored length). A file whose stored length is
//! below its length is doboz-compressed.

/// One file of an MVGL archive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MvglEntry {
    /// `name.ext`, the name a path with `\` separators.
    pub name: String,
    /// The offset of its bytes in the archive.
    pub offset: u64,
    /// The length once decompressed.
    pub len: u32,
    /// The length of the bytes in the archive.
    pub stored: u32,
}

impl MvglEntry {
    /// The file's content from its stored bytes.
    pub fn decode(&self, stored: &[u8]) -> Result<Vec<u8>, String> {
        if self.stored < self.len { doboz(stored) } else { Ok(stored.to_vec()) }
    }
}

fn u16_at(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(o..o + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| char::from(c)).collect()
}

/// The files of an MVGL archive, sorted by name. Of two entries with one name the later wins. `read(offset, len)` returns bytes of the archive. An error means the file is
/// not an MVGL archive (the game's sound archives share the extension).
pub fn mvgl_entries(mut read: impl FnMut(u64, usize) -> Result<Vec<u8>, String>) -> Result<Vec<MvglEntry>, String> {
    let h = read(0, 0x14)?;
    if h.get(..4) != Some(b"MDB1") {
        return Err("not an MVGL archive".into());
    }
    let bad = || "a damaged MVGL header".to_string();
    let (nt, nn) = (u16_at(&h, 4).ok_or_else(bad)? as usize, u16_at(&h, 6).ok_or_else(bad)? as usize);
    let (nd, start) = (u32_at(&h, 8).ok_or_else(bad)? as usize, u32_at(&h, 12).ok_or_else(bad)?);
    let body = read(0x14, nt * 8 + nn * 0x40 + nd * 12)?;
    let (names, data) = (nt * 8, nt * 8 + nn * 0x40);
    let mut out = std::collections::BTreeMap::new();
    for k in 0..nt {
        let idx = u16_at(&body, k * 8 + 2).ok_or_else(bad)?;
        if idx == 0xffff {
            continue;
        }
        let ne = body.get(names + k * 0x40..names + (k + 1) * 0x40).ok_or_else(bad)?;
        let ext = latin1(&ne[..4]).trim_end_matches(['\0', ' ']).to_string();
        let name = latin1(ne[4..].split(|&c| c == 0).next().unwrap_or_default());
        let d = data + idx as usize * 12;
        let (off, len, stored) =
            (u32_at(&body, d).ok_or_else(bad)?, u32_at(&body, d + 4).ok_or_else(bad)?, u32_at(&body, d + 8).ok_or_else(bad)?);
        let name = format!("{name}.{ext}");
        out.insert(name.clone(), MvglEntry { name, offset: u64::from(start) + u64::from(off), len, stored });
    }
    Ok(out.into_values().collect())
}

/// mask, offset shift, length mask, length shift, size in bytes, by the low 3 bits of a match word.
const MATCH: [(u32, u32, u32, u32, usize); 8] = [
    (0xff, 2, 0, 0, 1),
    (0xffff, 2, 0, 0, 2),
    (0xffff, 6, 15, 2, 2),
    (0xff_ffff, 8, 31, 3, 3),
    (0xff, 2, 0, 0, 1),
    (0xffff, 2, 0, 0, 2),
    (0xffff, 6, 15, 2, 2),
    (0xffff_ffff, 11, 255, 3, 4),
];

/// Decompresses doboz (Attila T. Afra's format, version 0). One attributes byte (bits 0 to 2 the version, 3 to 5 the size
/// width minus 1, bit 7 stored), the length and the stored length in that width, then a 32-bit control word per 32 items
/// (bit 0 first, 0 a literal byte, 1 a match) that is refilled when only its sentinel bit is left. The last 8 bytes are always
/// literals.
pub fn doboz(b: &[u8]) -> Result<Vec<u8>, String> {
    let a = *b.first().ok_or("an empty doboz stream")?;
    if a & 7 != 0 {
        return Err(format!("doboz version {}", a & 7));
    }
    let w = usize::from((a >> 3) & 7) + 1;
    let int = |s: &[u8]| s.iter().rev().fold(0u64, |v, &c| (v << 8) | u64::from(c));
    let head = b.get(1..1 + 2 * w).ok_or("a cut doboz header")?;
    let (size, csize) = (usize::try_from(int(&head[..w])).map_err(|e| e.to_string())?, int(&head[w..]));
    let start = 1 + 2 * w;
    if a & 0x80 != 0 {
        return b.get(start..start + size).map(<[u8]>::to_vec).ok_or_else(|| "a cut stored doboz stream".into());
    }
    let mut src = b.to_vec();
    src.extend_from_slice(&[0; 8]);
    let at = |i: usize, n: usize| src.get(i..i + n).ok_or_else(|| "a doboz stream overrun".to_string());
    let word = |i: usize| at(i, 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]));
    let mut out: Vec<u8> = Vec::with_capacity(size);
    let (mut i, mut ctl) = (start, 1u32);
    while out.len() < size {
        if ctl == 1 {
            ctl = word(i)?;
            i += 4;
        }
        if ctl & 1 == 0 {
            if out.len() + 8 < size {
                out.push(at(i, 1)?[0]);
                i += 1;
            } else {
                while out.len() < size {
                    if ctl == 1 {
                        ctl = word(i)?;
                        i += 4;
                    }
                    out.push(at(i, 1)?[0]);
                    i += 1;
                    ctl >>= 1;
                }
                break;
            }
        } else {
            let x = word(i)?;
            let (mask, shift, lmask, lshift, n_bytes) = MATCH[(x & 7) as usize];
            let off = ((x & mask) >> shift) as usize;
            let n = ((x >> lshift) & lmask) as usize + 3;
            i += n_bytes;
            let s = out.len().checked_sub(off).ok_or("a doboz match before the start")?;
            for k in 0..n {
                let c = *out.get(s + k).ok_or("a doboz match past the end")?;
                out.push(c);
            }
        }
        ctl >>= 1;
    }
    if (i - start) as u64 > csize + 4 {
        return Err("a doboz stream overrun".into());
    }
    Ok(out)
}

/// The GLSL sources in an NSO's read-only segment: each NUL-terminated string that starts with `#version` and a digit, at most
/// 0x10000 bytes. A string `#version:` (glslang's own messages, linked into some games) does not count.
pub fn nso_glsl(ro: &[u8]) -> Vec<Vec<u8>> {
    let pat = b"#version ";
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(p) = ro.get(i..).and_then(|r| r.windows(pat.len()).position(|w| w == pat)).map(|p| p + i) {
        i = p + 1;
        let starts = p == 0 || ro[p - 1] == 0;
        if !starts || !ro.get(p + pat.len()).is_some_and(u8::is_ascii_digit) {
            continue;
        }
        let end = (p + 0x10000).min(ro.len());
        let s = &ro[p..end];
        out.push(s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())].to_vec());
    }
    out
}

#[cfg(test)]
#[path = "mvgl_tests.rs"]
mod tests;
