//! LZ4 block decompression (the NSO segment codec.`lz4.block.decompress(raw, uncompressed_size=n)`).

use crate::SplitError;

/// Decompresses one LZ4 block that must expand to exactly `size` bytes.
pub fn block(src: &[u8], size: usize) -> Result<Vec<u8>, SplitError> {
    let mut out = Vec::with_capacity(size.min(src.len().saturating_mul(255)));
    decode_into(src, &mut out, 0, size)?;
    if out.len() != size {
        return Err(SplitError::Format(
            "LZ4 block: output shorter than its size".into(),
        ));
    }
    Ok(out)
}

/// `lz4.block.decompress(src, uncompressed_size=max)`: a block that expands to at most `max` bytes (fewer is fine).
pub fn block_upto(src: &[u8], max: usize) -> Result<Vec<u8>, SplitError> {
    let mut out = Vec::new();
    decode_into(src, &mut out, 0, max)?;
    Ok(out)
}

/// Appends the block `src` to `out`. Matches may reach back to `out[base]`.`limit` bounds the length of `out`.
fn decode_into(src: &[u8], out: &mut Vec<u8>, base: usize, limit: usize) -> Result<(), SplitError> {
    let bad = |m: &str| SplitError::Format(format!("LZ4 block: {m}"));
    let size = limit;
    let mut i = 0usize;
    while i < src.len() {
        let token = src[i];
        i += 1;
        let mut lit = (token >> 4) as usize;
        if lit == 15 {
            loop {
                let b = *src
                    .get(i)
                    .ok_or_else(|| bad("literal length past the end"))?;
                i += 1;
                lit += b as usize;
                if b != 255 {
                    break;
                }
            }
        }
        let l = src
            .get(i..i + lit)
            .ok_or_else(|| bad("literals past the end"))?;
        out.extend_from_slice(l);
        i += lit;
        if i >= src.len() {
            break; // the last sequence has literals only
        }
        let off = u16::from_le_bytes([
            src[i],
            *src.get(i + 1).ok_or_else(|| bad("offset past the end"))?,
        ]) as usize;
        i += 2;
        if off == 0 || off > out.len() - base {
            return Err(bad("match offset outside the output"));
        }
        let mut ml = (token & 15) as usize + 4;
        if token & 15 == 15 {
            loop {
                let b = *src.get(i).ok_or_else(|| bad("match length past the end"))?;
                i += 1;
                ml += b as usize;
                if b != 255 {
                    break;
                }
            }
        }
        if out.len() + ml > size {
            return Err(bad("output longer than its size"));
        }
        let start = out.len() - off;
        for k in 0..ml {
            let b = out[start + k];
            out.push(b);
        }
    }
    Ok(())
}

/// Decompresses an LZ4 frame (`lz4.frame.decompress`): blocks, linked or independent. The checksums are not
/// verified. Output over `limit` bytes is an error.
pub fn frame(f: &[u8], limit: usize) -> Result<Vec<u8>, SplitError> {
    let bad = |m: &str| SplitError::Format(format!("LZ4 frame: {m}"));
    if f.get(..4) != Some(&[0x04, 0x22, 0x4d, 0x18]) {
        return Err(bad("no magic"));
    }
    let (flg, bd) = (
        *f.get(4).ok_or_else(|| bad("header"))?,
        *f.get(5).ok_or_else(|| bad("header"))?,
    );
    if flg >> 6 != 1 {
        return Err(bad("version"));
    }
    let max = match bd >> 4 & 7 {
        4 => 64 << 10,
        5 => 256 << 10,
        6 => 1 << 20,
        7 => 4 << 20,
        _ => return Err(bad("block size")),
    };
    let independent = flg & 0x20 != 0;
    let mut at = 6 + if flg & 8 != 0 { 8 } else { 0 } + if flg & 1 != 0 { 4 } else { 0 } + 1;
    let mut out = Vec::new();
    loop {
        let w = f
            .get(at..at + 4)
            .ok_or_else(|| bad("block header past the end"))?;
        let w = u32::from_le_bytes([w[0], w[1], w[2], w[3]]);
        at += 4;
        if w == 0 {
            return Ok(out);
        }
        let n = (w & 0x7fff_ffff) as usize;
        let data = f.get(at..at + n).ok_or_else(|| bad("block past the end"))?;
        at += n + if flg & 0x10 != 0 { 4 } else { 0 };
        if n > max {
            return Err(bad("block over the block size"));
        }
        let start = out.len();
        if w & 0x8000_0000 != 0 {
            out.extend_from_slice(data);
        } else {
            let base = if independent { start } else { 0 };
            decode_into(data, &mut out, base, start + max)?;
        }
        if out.len() > limit {
            return Err(bad("output over its limit"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_and_overlapping_match() {
        // "abc" then a match of 9 at offset 3 -> "abcabcabcabc"
        let src = [0x35, b'a', b'b', b'c', 3, 0];
        assert_eq!(block(&src, 12).ok(), Some(b"abcabcabcabc".to_vec()));
        assert!(block(&src, 11).is_err());
    }
}
