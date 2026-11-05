//! LZ4 block decompression (the NSO segment codec.`lz4.block.decompress(raw, uncompressed_size=n)`).

use crate::SplitError;

/// Decompresses one LZ4 block that must expand to exactly `size` bytes.
pub fn block(src: &[u8], size: usize) -> Result<Vec<u8>, SplitError> {
    let bad = |m: &str| SplitError::Format(format!("LZ4 block: {m}"));
    let mut out = Vec::with_capacity(size);
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
        if off == 0 || off > out.len() {
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
    if out.len() != size {
        return Err(bad("output shorter than its size"));
    }
    Ok(out)
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
