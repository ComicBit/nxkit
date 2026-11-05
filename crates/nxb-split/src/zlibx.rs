//! zlib and gzip wrappers around [`crate::inflate`], with their checksums (Python's `zlib` and `gzip` modules).

/// zlib or gzip with its checksum (`zlib.decompress(d, 47)`), `None` when it fails.
pub fn zlib_auto(d: &[u8]) -> Option<Vec<u8>> {
    const LIMIT: usize = 1 << 30;
    if d.get(..2) == Some(&[0x1f, 0x8b]) {
        gzip_member(d, LIMIT).map(|x| x.0)
    } else {
        zlib_stream(d, LIMIT)
    }
}

/// One gzip member: its bytes and the length it took.
pub fn gzip_member(d: &[u8], limit: usize) -> Option<(Vec<u8>, usize)> {
    {
        // gzip: header with optional fields, deflate, CRC-32 and size
        let flg = *d.get(3)?;
        let mut p = 10;
        if flg & 4 != 0 {
            p += 2 + u16::from_le_bytes([*d.get(p)?, *d.get(p + 1)?]) as usize;
        }
        for bit in [8u8, 16] {
            if flg & bit != 0 {
                p += d.get(p..)?.iter().position(|&b| b == 0)? + 1;
            }
        }
        if flg & 2 != 0 {
            p += 2;
        }
        let (out, used) = crate::inflate::inflate_len(d.get(p..)?, limit)?;
        let t = d.get(p + used..p + used + 8)?;
        let crc = u32::from_le_bytes([t[0], t[1], t[2], t[3]]);
        let isize = u32::from_le_bytes([t[4], t[5], t[6], t[7]]);
        (crc == crc32(&out) && isize == out.len() as u32).then_some((out, p + used + 8))
    }
}

/// A zlib stream with its Adler-32.
pub fn zlib_stream(d: &[u8], limit: usize) -> Option<Vec<u8>> {
    zlib_stream_used(d, limit).map(|x| x.0)
}

/// A zlib stream and the bytes it took (header and checksum included).
pub fn zlib_stream_used(d: &[u8], limit: usize) -> Option<(Vec<u8>, usize)> {
    {
        let (cmf, flg) = (*d.first()?, *d.get(1)?);
        if cmf & 0xf != 8 || (u32::from(cmf) << 8 | u32::from(flg)) % 31 != 0 || flg & 0x20 != 0 {
            return None;
        }
        let (out, used) = crate::inflate::inflate_len(d.get(2..)?, limit)?;
        let t = d.get(2 + used..2 + used + 4)?;
        (u32::from_be_bytes([t[0], t[1], t[2], t[3]]) == adler32(&out))
            .then_some((out, 2 + used + 4))
    }
}

fn adler32(d: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in d {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    b << 16 | a
}

pub(crate) fn crc32(d: &[u8]) -> u32 {
    let mut c = !0u32;
    for &x in d {
        c ^= u32::from(x);
        for _ in 0..8 {
            c = (c >> 1) ^ (0xEDB8_8320 & (c & 1).wrapping_neg());
        }
    }
    !c
}

/// `ungzip`: every member of a gzip file (concatenated members, zero padding after them), at most `limit` bytes.
pub fn gunzip_all(d: &[u8], limit: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < d.len() {
        if d[at..].iter().all(|&b| b == 0) {
            break;
        }
        if d[at..].get(..2) != Some(&[0x1f, 0x8b]) {
            return Err("Not a gzipped file".into());
        }
        let (m, used) = gzip_member(&d[at..], limit).ok_or("damaged gzip member")?;
        out.extend(m);
        if out.len() > limit {
            return Err("too long".into());
        }
        at += used;
    }
    Ok(out)
}
