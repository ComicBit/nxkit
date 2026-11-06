//! NSO and NRO parsing (`dm/tools/nso.py` `load`): the memory image of text, ro and data, checked as Horizon checks it.

use nxb_model::sha256;

use crate::error::{RewriteError, fail};

const PAGE: usize = 0x1000;
const LZ4_MAX_RATIO: usize = 255;
const SIZE_MAX: usize = 0x7fff_ffff;
const NSO_ZSTD: u32 = 1 << 7;
const NRO_COMPRESSED: u32 = 1 << 1;
const NAMES: [&str; 3] = ["text", "ro", "data"];

/// A loaded module.
pub struct Module {
    /// Text, ro and data at their memory offsets (bss not included).
    pub image: Vec<u8>,
    /// `(memory offset, size)` of text, ro and data.
    pub segments: [(usize, usize); 3],
    /// The bss size.
    pub bss: usize,
    /// The 0x20 bytes of the build ID.
    pub build_id: [u8; 0x20],
}

impl Module {
    /// The end of the text segment.
    pub fn text_end(&self) -> usize {
        self.segments[0].1
    }
}

fn u32le(d: &[u8], o: usize) -> usize {
    d.get(o..o + 4)
        .map_or(0, |s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as usize)
}

/// Decompresses one LZ4 block to exactly `size` bytes.
fn lz4_block(src: &[u8], size: usize) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(size);
    let mut i = 0usize;
    while i < src.len() {
        let token = src[i];
        i += 1;
        let mut lit = usize::from(token >> 4);
        if lit == 15 {
            loop {
                let b = *src.get(i)?;
                i += 1;
                lit += usize::from(b);
                if b != 255 {
                    break;
                }
            }
        }
        out.extend_from_slice(src.get(i..i + lit)?);
        i += lit;
        if i >= src.len() {
            break;
        }
        let off = usize::from(u16::from_le_bytes([src[i], *src.get(i + 1)?]));
        i += 2;
        if off == 0 || off > out.len() {
            return None;
        }
        let mut ml = usize::from(token & 15) + 4;
        if token & 15 == 15 {
            loop {
                let b = *src.get(i)?;
                i += 1;
                ml += usize::from(b);
                if b != 255 {
                    break;
                }
            }
        }
        if out.len() + ml > size {
            return None;
        }
        let start = out.len() - off;
        for k in 0..ml {
            out.push(out[start + k]);
        }
    }
    (out.len() == size).then_some(out)
}

/// Loads the module in `d` (an NSO0 or NRO0 file).`name` goes into the messages.
pub fn load(d: &[u8], name: &str) -> Result<Module, RewriteError> {
    let mut flags = 0u32;
    let mut stored = [(0usize, 0usize, 0usize); 3];
    let mut csz = [0usize; 3];
    let bss;
    let build_id: [u8; 0x20];
    if d.get(..4) == Some(b"NSO0") {
        if d.len() < 0x100 {
            return fail(format!(
                "[RED] {name}: the NSO header is cut short ({} bytes)",
                d.len()
            ));
        }
        flags = u32le(d, 0xc) as u32;
        for (i, s) in stored.iter_mut().enumerate() {
            *s = (
                u32le(d, 0x10 + i * 0x10),
                u32le(d, 0x14 + i * 0x10),
                u32le(d, 0x18 + i * 0x10),
            );
            csz[i] = if flags >> i & 1 != 0 {
                u32le(d, 0x60 + i * 4)
            } else {
                s.2
            };
        }
        bss = u32le(d, 0x3c);
        build_id = d[0x40..0x60].try_into().unwrap_or([0; 0x20]);
        for (i, ((fo, mo, sz), c)) in stored.iter().zip(csz).enumerate() {
            let what = format!("{name}: {} segment", NAMES[i]);
            let comp = flags >> i & 1 != 0;
            if mo % PAGE != 0 {
                return fail(format!("[RED] {what} is at {mo:#x}, not on a page"));
            }
            if *sz > SIZE_MAX {
                return fail(format!(
                    "[RED] {what} claims {sz:#x} bytes, more than Horizon loads"
                ));
            }
            if fo + c > d.len() {
                return fail(format!(
                    "[RED] {what} is stored at {fo:#x}..{:#x}, past the end of the file ({:#x} bytes): the file is cut short or damaged",
                    fo + c,
                    d.len()
                ));
            }
            if comp && c > *sz {
                return fail(format!(
                    "[RED] {what} is stored compressed in {c:#x} bytes, more than its {sz:#x}"
                ));
            }
            if comp && flags & NSO_ZSTD == 0 && *sz > c * LZ4_MAX_RATIO {
                return fail(format!(
                    "[RED] {what} claims {sz:#x} bytes from {c:#x} stored, more than LZ4 can produce"
                ));
            }
        }
        if stored[0].1 != 0 {
            return fail(format!(
                "[RED] {name}: text segment is at {:#x}, not at 0",
                stored[0].1
            ));
        }
        for i in 1..3 {
            if stored[i].1 < stored[i - 1].1 + stored[i - 1].2 {
                return fail(format!(
                    "[RED] {name}: {} segment at {:#x} overlaps the {} segment before it",
                    NAMES[i],
                    stored[i].1,
                    NAMES[i - 1]
                ));
            }
        }
    } else if d.get(0x10..0x14) == Some(b"NRO0") {
        if d.len() < 0x80 {
            return fail(format!(
                "[RED] {name}: the NRO header is cut short ({} bytes)",
                d.len()
            ));
        }
        let size = u32le(d, 0x18);
        if u32le(d, 0x1c) as u32 & NRO_COMPRESSED != 0 {
            return fail(format!(
                "[RED] {name}: a compressed NRO (system 23.0.0+), which this loader does not decode yet"
            ));
        }
        for (i, s) in stored.iter_mut().enumerate() {
            let (o, sz) = (u32le(d, 0x20 + 8 * i), u32le(d, 0x24 + 8 * i));
            *s = (o, o, sz);
            csz[i] = sz;
        }
        bss = u32le(d, 0x38);
        build_id = d[0x40..0x60].try_into().unwrap_or([0; 0x20]);
        let mut at = 0;
        for (i, (o, _, sz)) in stored.iter().enumerate() {
            if sz % PAGE != 0 || *o != at {
                return fail(format!(
                    "[RED] {name}: {} segment {o:#x}..{:#x} is not whole pages right after the one before it (at {at:#x})",
                    NAMES[i],
                    o + sz
                ));
            }
            at += sz;
        }
        if at != size {
            return fail(format!(
                "[RED] {name}: the segments end at {at:#x}, not at the module's size {size:#x}"
            ));
        }
        if size > d.len() {
            return fail(format!(
                "[RED] {name}: the module is {size:#x} bytes, the file {:#x}: the file is cut short or damaged",
                d.len()
            ));
        }
    } else {
        return fail(format!(
            "[RED] {name}: not an NSO or NRO (magic {:?})",
            d.get(..4)
        ));
    }
    let end = stored[2].1 + stored[2].2;
    if end > d.len() * LZ4_MAX_RATIO {
        return fail(format!(
            "[RED] {name}: the segments span {end:#x} bytes, more than a file of {:#x} bytes can fill",
            d.len()
        ));
    }
    let mut img = vec![0u8; end];
    for (i, ((fo, mo, sz), c)) in stored.iter().zip(csz).enumerate() {
        let raw = &d[*fo..*fo + c];
        let seg: Vec<u8> = if flags >> i & 1 != 0 {
            if flags & NSO_ZSTD != 0 {
                match nxb_codec::zbic::decompress(raw, *sz) {
                    Ok(v) => v,
                    Err(e) => return fail(format!("{name}: {} segment: {e}", NAMES[i])),
                }
            } else {
                match lz4_block(raw, *sz) {
                    Some(v) => v,
                    None => {
                        return fail(format!(
                            "[RED] {name}: {} segment does not decompress",
                            NAMES[i]
                        ));
                    }
                }
            }
        } else {
            raw.to_vec()
        };
        if seg.len() != *sz {
            return fail(format!(
                "[RED] {name}: {} segment decompresses to {:#x} bytes, not the {sz:#x} of the header: the file is damaged",
                NAMES[i],
                seg.len()
            ));
        }
        if flags >> (3 + i) & 1 != 0
            && d.get(0xa0 + 0x20 * i..0xc0 + 0x20 * i) != Some(&sha256(&seg).0[..])
        {
            return fail(format!(
                "[RED] {name}: {} segment does not match its SHA-256 in the header: the file is damaged",
                NAMES[i]
            ));
        }
        img[*mo..*mo + *sz].copy_from_slice(&seg);
    }
    Ok(Module {
        image: img,
        segments: [
            (stored[0].1, stored[0].2),
            (stored[1].1, stored[1].2),
            (stored[2].1, stored[2].2),
        ],
        bss,
        build_id,
    })
}
