//! Camelot's archive `CRA.` (Mario Tennis Fever, `cra.py`): the whole game in one RomFS file. The file table is
//! not in the archive, it is built into the game's NRO (see `index`). Each file is scrambled in 16-byte blocks at
//! its own offset, with a key from the first characters of its last path element. A checksum per file proves the
//! key and the layout.

mod index;

pub use index::{Entry, Index, index_of};

use crate::SplitError;

/// The error type of `cra.py`, named as Python prints it (`Error: ...`).
pub(crate) fn err(m: impl AsRef<str>) -> SplitError {
    SplitError::Red(format!("Error: {}", m.as_ref()))
}

type Table = [[u32; 256]; 8];

const fn crc_table(poly: u32) -> Table {
    let mut t = [[0u32; 256]; 8];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = (c >> 1) ^ if c & 1 != 0 { poly } else { 0 };
            k += 1;
        }
        t[0][i] = c;
        i += 1;
    }
    let mut k = 1;
    while k < 8 {
        let mut i = 0;
        while i < 256 {
            t[k][i] = (t[k - 1][i] >> 8) ^ t[0][(t[k - 1][i] & 0xff) as usize];
            i += 1;
        }
        k += 1;
    }
    t
}

static CRC32: Table = crc_table(0xedb8_8320);
static CRC32C: Table = crc_table(0x82f6_3b78);

/// The `crc32x` and `crc32cx` instructions: no inversion before or after.
fn crc64(t: &Table, c: u64, v: u64) -> u32 {
    let x = c ^ v;
    let b = |n: u32| (x >> n & 0xff) as usize;
    t[7][b(0)]
        ^ t[6][b(8)]
        ^ t[5][b(16)]
        ^ t[4][b(24)]
        ^ t[3][b(32)]
        ^ t[2][b(40)]
        ^ t[1][b(48)]
        ^ t[0][b(56)]
}

/// The key's input: up to 4 characters of the last path element, up to its first `.` or `_`, as a big-endian
/// number.
fn stem(path: &[u8]) -> u64 {
    let e = path.rsplit(|&b| b == b'/').next().unwrap_or(path);
    let n = b"._"
        .iter()
        .filter_map(|c| e.iter().position(|b| b == c))
        .chain([4])
        .min()
        .unwrap_or(4);
    e[..n.min(e.len())]
        .iter()
        .fold(0u64, |a, &b| a << 8 | u64::from(b))
}

/// The scramble key of a path.
pub fn key(path: &[u8]) -> u32 {
    let le = |s: &[u8; 8]| u64::from_le_bytes(*s);
    let k = u64::from(crc64(&CRC32, stem(path), le(b"UseUrSk1")));
    let a = crc64(
        &CRC32,
        u64::from(crc64(&CRC32, 0xffff_ffff, le(b"Nintendo"))),
        le(b"/CAMELOT"),
    );
    a ^ crc64(&CRC32, k, le(b"lls4G00d")) ^ 0x0205_2636
}

const VARIANT_MASK: [u32; 8] = [
    0x1111_1111,
    0x2222_2223,
    0x3333_3333,
    0x4444_4445,
    0x5555_5555,
    0x6666_6667,
    0x7777_7777,
    0x8888_8889,
];

/// The blocks of a file, unscrambled, and the sum that gives its checksum. The key's low 3 bits choose a variant:
/// it takes a block's low (bit 1 clear) or high word, by CRC-32 (bit 2 clear) or CRC-32C, and writes its two
/// results low word first (bit 0 clear) or high word first.
pub fn unscramble(path: &[u8], raw: &[u8]) -> (Vec<u8>, u64) {
    let w = key(path);
    let v = (w & 7) as usize;
    let tab = if v & 4 != 0 { &CRC32C } else { &CRC32 };
    let mul = u64::from((w & 0xffff_fffe) ^ VARIANT_MASK[v]) << 32 | 1;
    let (src, oth) = if v & 2 != 0 { (1, 0) } else { (0, 1) };
    let (a, b) = if v & 1 != 0 { (1, 0) } else { (0, 1) };
    let mut out = Vec::with_capacity(raw.len() / 16 * 16);
    let mut total = 0u64;
    for blk in raw.as_chunks::<16>().0 {
        let word = |i: usize| {
            let mut x = [0u8; 8];
            x.copy_from_slice(&blk[i * 8..i * 8 + 8]);
            u64::from_le_bytes(x)
        };
        let s = word(src);
        let m = u64::from(crc64(tab, s, u64::from(w))).wrapping_mul(mul);
        let e = word(oth) ^ (m >> 3);
        let mut o = [0u64; 2];
        o[a] = !(m ^ (s << 32));
        o[b] = e;
        out.extend_from_slice(&o[0].to_le_bytes());
        out.extend_from_slice(&o[1].to_le_bytes());
        total = total.wrapping_add(e);
    }
    (out, total)
}

/// A file's bytes from the archive bytes at its offset, unscrambled and checked (`read`). The checksum covers
/// whole blocks, the tail of the last block past the size is padding.
pub fn read(raw: &[u8], e: &Entry) -> Result<Vec<u8>, SplitError> {
    let (mut plain, total) = unscramble(&e.path, raw);
    if ((total >> 32) ^ total) as u32 != e.sum {
        return Err(err(format!("{}: the checksum differs", e.label())));
    }
    plain.truncate(e.size as usize);
    Ok(plain)
}

/// How many bytes of the archive `read` takes for an entry.
pub fn span(e: &Entry) -> usize {
    (e.size as usize).saturating_add(15) & !15
}

/// The first block of a file, unscrambled (`head`): its magic.
pub fn head(raw: &[u8], e: &Entry) -> Vec<u8> {
    unscramble(&e.path, &raw[..raw.len().min(16)]).0
}

#[cfg(test)]
mod tests;
