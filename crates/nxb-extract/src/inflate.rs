//! Raw deflate (RFC 1951) decoding for the compressed language entries of newer NACPs (`TitleName` in `nx_extract`
//! uses .NET's `DeflateStream`). Small and slow on purpose: it decodes at most 0x6000 bytes per package. It belongs in
//! `nxb-codec` (zlib) once that crate has it.

struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
}

impl Bits<'_> {
    fn bit(&mut self) -> Option<u32> {
        let b = *self.d.get(self.pos / 8)?;
        let v = (b >> (self.pos % 8)) & 1;
        self.pos += 1;
        Some(v as u32)
    }
    fn bits(&mut self, n: u32) -> Option<u32> {
        let mut v = 0;
        for i in 0..n {
            v |= self.bit()? << i;
        }
        Some(v)
    }
}

/// Canonical Huffman code: (count per length, symbols sorted by code).
struct Code {
    count: [u16; 16],
    sym: Vec<u16>,
}

impl Code {
    fn new(lengths: &[u8]) -> Code {
        let mut count = [0u16; 16];
        for &l in lengths {
            count[l as usize] += 1;
        }
        count[0] = 0;
        let mut offs = [0u16; 16];
        for i in 1..16 {
            offs[i] = offs[i - 1] + count[i - 1];
        }
        let mut sym = vec![0u16; lengths.len()];
        for (s, &l) in lengths.iter().enumerate() {
            if l != 0 {
                sym[offs[l as usize] as usize] = s as u16;
                offs[l as usize] += 1;
            }
        }
        Code { count, sym }
    }
    fn decode(&self, b: &mut Bits<'_>) -> Option<u16> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= b.bit()? as i32;
            let count = self.count[len] as i32;
            if code - count < first {
                return self.sym.get((index + code - first) as usize).copied();
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        None
    }
}

const LBASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEXT: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DBASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DEXT: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

/// Inflates a raw deflate stream.`None` if it is not valid deflate or grows past `limit`.
pub fn inflate(d: &[u8], limit: usize) -> Option<Vec<u8>> {
    let mut b = Bits { d, pos: 0 };
    let mut out = Vec::new();
    loop {
        let last = b.bit()?;
        match b.bits(2)? {
            0 => {
                b.pos = b.pos.div_ceil(8) * 8;
                let p = b.pos / 8;
                let len = u16::from_le_bytes([*d.get(p)?, *d.get(p + 1)?]);
                let nlen = u16::from_le_bytes([*d.get(p + 2)?, *d.get(p + 3)?]);
                if len != !nlen {
                    return None;
                }
                out.extend_from_slice(d.get(p + 4..p + 4 + len as usize)?);
                b.pos = (p + 4 + len as usize) * 8;
            }
            t @ (1 | 2) => {
                let (lit, dist) = if t == 1 {
                    let mut l = [8u8; 288];
                    l[144..256].iter_mut().for_each(|x| *x = 9);
                    l[256..280].iter_mut().for_each(|x| *x = 7);
                    (Code::new(&l), Code::new(&[5u8; 30]))
                } else {
                    let hlit = b.bits(5)? as usize + 257;
                    let hdist = b.bits(5)? as usize + 1;
                    let hclen = b.bits(4)? as usize + 4;
                    const ORDER: [usize; 19] = [
                        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
                    ];
                    let mut cl = [0u8; 19];
                    for &o in ORDER.iter().take(hclen) {
                        cl[o] = b.bits(3)? as u8;
                    }
                    let clc = Code::new(&cl);
                    let mut lens = Vec::with_capacity(hlit + hdist);
                    while lens.len() < hlit + hdist {
                        let s = clc.decode(&mut b)?;
                        let (v, n) = match s {
                            0..=15 => (s as u8, 1),
                            16 => (*lens.last()?, 3 + b.bits(2)? as usize),
                            17 => (0, 3 + b.bits(3)? as usize),
                            _ => (0, 11 + b.bits(7)? as usize),
                        };
                        lens.extend(std::iter::repeat_n(v, n));
                    }
                    if lens.len() != hlit + hdist {
                        return None;
                    }
                    (Code::new(&lens[..hlit]), Code::new(&lens[hlit..]))
                };
                loop {
                    let s = lit.decode(&mut b)? as usize;
                    if s < 256 {
                        out.push(s as u8);
                    } else if s == 256 {
                        break;
                    } else {
                        let i = s - 257;
                        let len = *LBASE.get(i)? as usize + b.bits(*LEXT.get(i)? as u32)? as usize;
                        let ds = dist.decode(&mut b)? as usize;
                        let dd = *DBASE.get(ds)? as usize + b.bits(*DEXT.get(ds)? as u32)? as usize;
                        if dd > out.len() {
                            return None;
                        }
                        let start = out.len() - dd;
                        for k in 0..len {
                            let c = out[start + k];
                            out.push(c);
                        }
                    }
                    if out.len() > limit {
                        return None;
                    }
                }
            }
            _ => return None,
        }
        if out.len() > limit {
            return None;
        }
        if last == 1 {
            return Some(out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_and_fixed() {
        // stored block "abc"
        assert_eq!(
            inflate(&[0x01, 3, 0, 0xfc, 0xff, b'a', b'b', b'c'], 100),
            Some(b"abc".to_vec())
        );
        // fixed Huffman "a" (zlib's raw deflate of "a": 4b 04 00)
        assert_eq!(inflate(&[0x4b, 0x04, 0x00], 100), Some(b"a".to_vec()));
        assert_eq!(inflate(&[0x07], 100), None);
    }
}
