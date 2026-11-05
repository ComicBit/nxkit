//! Universal ("fat") Mach-O files: splitting into slices and reassembling them the way
//! `codesign_allocate` lays out a signed universal file.
//!
//! Measured layout after signing: the 32-bit fat header and `fat_arch` table (big-endian), then
//! each slice at an offset aligned to `2^align`, where `align` becomes at least 14 (16 KiB) for
//! every slice, also for x86_64 slices that the linker aligned to 4 KiB. Gaps are zero, the file
//! ends with the last slice.

use crate::error::{Error, Result, format_err};

const FAT_MAGIC: u32 = 0xcafe_babe;
const FAT_MAGIC_64: u32 = 0xcafe_babf;
/// The minimum slice alignment (log2) of a file written by `codesign_allocate`.
pub const MIN_ALIGN: u32 = 14;

/// One architecture of a universal file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slice {
    /// `cputype`.
    pub cputype: u32,
    /// `cpusubtype`.
    pub cpusubtype: u32,
    /// Alignment as a power of two.
    pub align: u32,
    /// The slice's bytes (a thin Mach-O image).
    pub data: Vec<u8>,
}

/// Whether `data` is a universal file.
pub fn is_fat(data: &[u8]) -> bool {
    data.len() >= 8
        && matches!(
            u32::from_be_bytes([data[0], data[1], data[2], data[3]]),
            FAT_MAGIC | FAT_MAGIC_64
        )
}

fn be32(b: &[u8], o: usize) -> Result<u32> {
    b.get(o..o + 4)
        .map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| format_err("fat header truncated"))
}

/// Splits a universal file into its slices, in table order.
pub fn split(data: &[u8]) -> Result<Vec<Slice>> {
    let magic = be32(data, 0)?;
    if magic == FAT_MAGIC_64 {
        return Err(Error::Unsupported("64-bit fat header".into()));
    }
    if magic != FAT_MAGIC {
        return Err(format_err("not a universal file"));
    }
    let n = be32(data, 4)? as usize;
    if n == 0 || n > 64 {
        return Err(format_err("bad number of architectures"));
    }
    (0..n)
        .map(|i| {
            let o = 8 + 20 * i;
            let (off, size) = (be32(data, o + 8)? as usize, be32(data, o + 12)? as usize);
            let bytes = data
                .get(off..off + size)
                .ok_or_else(|| format_err("slice past the end of the file"))?;
            Ok(Slice {
                cputype: be32(data, o)?,
                cpusubtype: be32(data, o + 4)?,
                align: be32(data, o + 16)?,
                data: bytes.to_vec(),
            })
        })
        .collect()
}

/// Writes a universal file from `slices` with `codesign_allocate`'s layout.
pub fn assemble(slices: &[Slice]) -> Result<Vec<u8>> {
    let header = 8 + 20 * slices.len();
    let mut offsets = Vec::with_capacity(slices.len());
    let mut end = header as u64;
    for s in slices {
        let align = s.align.max(MIN_ALIGN);
        let off = end.next_multiple_of(1u64 << align);
        offsets.push((off, align));
        end = off + s.data.len() as u64;
    }
    if end > u32::MAX as u64 {
        return Err(Error::Unsupported("universal file beyond 4 GiB".into()));
    }
    let mut out = vec![0u8; end as usize];
    out[0..4].copy_from_slice(&FAT_MAGIC.to_be_bytes());
    out[4..8].copy_from_slice(&(slices.len() as u32).to_be_bytes());
    for (i, (s, (off, align))) in slices.iter().zip(&offsets).enumerate() {
        let o = 8 + 20 * i;
        for (k, v) in [
            s.cputype,
            s.cpusubtype,
            *off as u32,
            s.data.len() as u32,
            *align,
        ]
        .iter()
        .enumerate()
        {
            out[o + 4 * k..o + 4 * k + 4].copy_from_slice(&v.to_be_bytes());
        }
        out[*off as usize..*off as usize + s.data.len()].copy_from_slice(&s.data);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_matches_codesign_allocate() {
        // fat0 from the oracle: x86_64 4272 bytes align 12, arm64 16832 bytes align 14.
        // codesign wrote x86_64 (4608) at 16384 and arm64 (16896) at 32768, both align 14.
        let slices = vec![
            Slice {
                cputype: 0x0100_0007,
                cpusubtype: 3,
                align: 12,
                data: vec![1; 4608],
            },
            Slice {
                cputype: 0x0100_000c,
                cpusubtype: 0,
                align: 14,
                data: vec![2; 16896],
            },
        ];
        let f = assemble(&slices).unwrap();
        assert_eq!(f.len(), 49664);
        let back = split(&f).unwrap();
        assert_eq!(back[0].data, slices[0].data);
        assert_eq!(back[1].align, 14);
        assert_eq!(be32(&f, 8 + 8).unwrap(), 16384);
        assert_eq!(be32(&f, 28 + 8).unwrap(), 32768);
        assert!(is_fat(&f));
    }
}
