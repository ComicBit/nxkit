//! Size-prefixed frames (`unsized`, `frame_size` of `shader_split.py`): HAL's `.cmp` containers, a 4-byte
//! little-endian size and then a zstd frame (with a dictionary as for a zstd file) or an LZ4 frame.

use crate::SplitError;
use crate::lz4;
use crate::zstd::{Dicts, decompress_dict, dict_id};

const LZ4F: [u8; 4] = [0x04, 0x22, 0x4d, 0x18];

/// `frame_size`: the size a zstd or LZ4 frame header records, `None` when it records none.
fn frame_size(f: &[u8]) -> Option<u64> {
    let le = |a: usize, n: usize| {
        let mut v = 0u64;
        for i in (0..n).rev() {
            v = v << 8 | u64::from(f.get(a + i).copied().unwrap_or(0));
        }
        v
    };
    if f.get(..4) == Some(&LZ4F[..]) {
        return (f.len() > 14 && f[4] & 8 != 0).then(|| le(6, 8));
    }
    let fhd = if f.len() > 5 { f[4] } else { 0 };
    let single = usize::from(fhd >> 5 & 1);
    let n = [single, 2, 4, 8][(fhd >> 6) as usize];
    if n == 0 {
        return None;
    }
    let o = 6 - single + [0, 1, 2, 4][(fhd & 3) as usize];
    // a slice shorter than n reads as fewer bytes, as Python's slice does
    let avail = f.len().saturating_sub(o).min(n);
    let mut v = 0u64;
    for i in (0..avail).rev() {
        v = v << 8 | u64::from(f[o + i]);
    }
    Some(v + if n == 2 { 256 } else { 0 })
}

/// The result of [`unsized_frame`].
pub enum Sized {
    /// The frame's bytes.
    Opened(Vec<u8>),
    /// Not such a container, or one that did not open: the file is read as it is, the text says why for the latter.
    AsIs(Option<String>),
}

/// `unsized`: `d` is the file, a size then a frame.
pub fn unsized_frame(d: &[u8], dicts: &Dicts) -> Result<Sized, SplitError> {
    let n = d.get(..4).map_or(0, |b| {
        u64::from(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    });
    let f = d.get(4..).unwrap_or(&[]);
    if let Some(s) = frame_size(f)
        && s != n
    {
        return Ok(Sized::AsIs(None));
    }
    let did = dict_id(f);
    let dec = if f.get(..4) == Some(&LZ4F[..]) {
        lz4::frame(f, 1 << 30)
    } else if did != 0 && !dicts.contains(did) {
        return Ok(Sized::AsIs(Some(format!(
            "the frame needs zstd dictionary {did}, which Pack/ZsDic.pack.zs does not have"
        ))));
    } else {
        decompress_dict(f, dicts)
    };
    match dec {
        Ok(v) if v.len() as u64 == n => Ok(Sized::Opened(v)),
        Ok(v) => Ok(Sized::AsIs(Some(format!(
            "the size prefix says {n} bytes, the frame holds {}",
            v.len()
        )))),
        Err(e @ SplitError::Unsupported(_)) => Err(e),
        Err(e) => Ok(Sized::AsIs(Some(e.to_string()))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_of_frame_headers() {
        // single segment, 1-byte size 5
        assert_eq!(frame_size(&[0x28, 0xb5, 0x2f, 0xfd, 0x20, 5]), Some(5));
        // no size recorded (window descriptor, no size flag)
        assert_eq!(frame_size(&[0x28, 0xb5, 0x2f, 0xfd, 0x00, 0x50]), None);
        assert_eq!(frame_size(&[4, 0x22, 0x4d, 0x18, 0x60, 0x40, 0]), None);
    }
}
