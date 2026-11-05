//! NSO module image (`dm/tools/nso.py` `load`): the three segments at their memory offsets, LZ4 segments decompressed,
//! hash-checked segments checked.

use nxb_model::sha256;

use crate::SplitError;

fn u32le(d: &[u8], o: usize) -> u32 {
    d.get(o..o + 4)
        .map_or(0, |s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// The memory image of an NSO.
pub fn image(d: &[u8]) -> Result<Vec<u8>, SplitError> {
    let err = |m: String| SplitError::Format(m);
    if d.get(..4) != Some(b"NSO0") || d.len() < 0x100 {
        return Err(err("not an NSO (or an NRO, not read yet)".into()));
    }
    let flags = u32le(d, 0xc);
    if flags & (1 << 6) != 0 {
        return Err(SplitError::Unsupported(
            "zstd-compressed NSO segments".into(),
        ));
    }
    let stored: Vec<(usize, usize, usize)> = (0..3)
        .map(|i| {
            (
                u32le(d, 0x10 + 16 * i) as usize,
                u32le(d, 0x14 + 16 * i) as usize,
                u32le(d, 0x18 + 16 * i) as usize,
            )
        })
        .collect();
    let end = stored[2].1 + stored[2].2;
    if end > d.len() * 256 || stored[0].1 != 0 {
        return Err(err("NSO segment layout".into()));
    }
    let mut img = vec![0u8; end];
    for (i, &(fo, mo, sz)) in stored.iter().enumerate() {
        let c = if flags >> i & 1 != 0 {
            u32le(d, 0x60 + 4 * i) as usize
        } else {
            sz
        };
        let raw = d
            .get(fo..fo + c)
            .ok_or_else(|| err(format!("NSO segment {i} past the end of the file")))?;
        let seg = if flags >> i & 1 != 0 {
            crate::lz4::block(raw, sz)?
        } else {
            raw.to_vec()
        };
        if flags >> (3 + i) & 1 != 0
            && d.get(0xa0 + 0x20 * i..0xc0 + 0x20 * i) != Some(&sha256(&seg).0[..])
        {
            return Err(err(format!("NSO segment {i} does not match its SHA-256")));
        }
        img.get_mut(mo..mo + sz)
            .ok_or_else(|| err("NSO segments overlap the image end".into()))?
            .copy_from_slice(&seg);
    }
    Ok(img)
}
