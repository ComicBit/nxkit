//! Yaz0 decompression (`yaz0` of `shader_split.py`, some titles pack most of their files this way).

use crate::SplitError;

/// Decodes a Yaz0 stream (16-byte header, big-endian size at +4).
pub fn decode(s: &[u8], label: &str) -> Result<Vec<u8>, SplitError> {
    let n = s
        .get(4..8)
        .map_or(0, |b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize);
    let mut out: Vec<u8> = Vec::with_capacity(n);
    let mut si = 16usize;
    let short = |out: &Vec<u8>| {
        SplitError::Red(format!(
            "{label}: the Yaz0 stream ends at {:#x} with {:#x} of its {n:#x} bytes decoded",
            s.len(),
            out.len()
        ))
    };
    while out.len() < n {
        let Some(&h) = s.get(si) else {
            return Err(short(&out));
        };
        si += 1;
        let mut h = h;
        for _ in 0..8 {
            if h & 0x80 != 0 {
                let Some(&b) = s.get(si) else {
                    return Err(short(&out));
                };
                out.push(b);
                si += 1;
            } else {
                let (Some(&b1), Some(&b2)) = (s.get(si), s.get(si + 1)) else {
                    return Err(short(&out));
                };
                si += 2;
                let dist = (((b1 & 15) as usize) << 8 | b2 as usize) + 1;
                let mut ln = (b1 >> 4) as usize;
                if ln != 0 {
                    ln += 2;
                } else {
                    let Some(&b3) = s.get(si) else {
                        return Err(short(&out));
                    };
                    ln = b3 as usize + 0x12;
                    si += 1;
                }
                let Some(st) = out.len().checked_sub(dist) else {
                    return Err(SplitError::Red(format!(
                        "{label}: a back-reference at {si:#x} reaches {:#x} bytes before the start",
                        dist - out.len()
                    )));
                };
                for k in 0..ln {
                    let c = out[st + k];
                    out.push(c);
                }
            }
            if out.len() >= n {
                break;
            }
            h <<= 1;
        }
    }
    out.truncate(n);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_and_copy() {
        // header, then group 0b1110_0000: 'a' 'b' 'c' literal, then a copy of 6 at distance 3
        let mut s = b"Yaz0\0\0\0\x09\0\0\0\0\0\0\0\0".to_vec();
        s.extend_from_slice(&[0xe0, b'a', b'b', b'c', 0x40, 0x02]);
        assert_eq!(decode(&s, "t").ok(), Some(b"abcabcabc".to_vec()));
        assert!(decode(&s[..20], "t").is_err());
    }
}
