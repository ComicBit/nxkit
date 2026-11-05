//! Huffman literals (RFC 8878 4.2).

use super::{Back, Fse, R, err, hb};

#[derive(Clone, Default)]
pub(super) struct Huf {
    max_bits: u32,
    sym: Vec<u8>,
    bits: Vec<u8>,
}

impl Huf {
    fn from_weights(mut w: Vec<u8>) -> R<Huf> {
        if w.len() > 255 {
            return Err(err("Huffman weights"));
        }
        let sum: u32 = w.iter().filter(|&&x| x > 0).map(|&x| 1u32 << (x - 1)).sum();
        if sum == 0 {
            return Err(err("Huffman weights all zero"));
        }
        let max_bits = hb(sum) + 1;
        let left = (1u32 << max_bits) - sum;
        if !left.is_power_of_two() || max_bits > 11 {
            return Err(err("Huffman weights do not complete a tree"));
        }
        w.push((hb(left) + 1) as u8);
        let nb: Vec<u32> = w
            .iter()
            .map(|&x| if x > 0 { max_bits + 1 - x as u32 } else { 0 })
            .collect();
        let size = 1usize << max_bits;
        let mut rank_count = vec![0usize; max_bits as usize + 1];
        for &b in &nb {
            if b > 0 {
                rank_count[b as usize] += 1;
            }
        }
        let mut rank_idx = vec![0usize; max_bits as usize + 1];
        let mut sym = vec![0u8; size];
        let mut bits = vec![0u8; size];
        rank_idx[max_bits as usize] = 0;
        for i in (1..=max_bits as usize).rev() {
            rank_idx[i - 1] = rank_idx[i] + rank_count[i] * (1 << (max_bits as usize - i));
            for x in &mut bits[rank_idx[i]..rank_idx[i - 1].min(size)] {
                *x = i as u8;
            }
        }
        for (s, &b) in nb.iter().enumerate() {
            if b > 0 {
                let code = rank_idx[b as usize];
                let len = 1usize << (max_bits - b);
                for x in sym
                    .get_mut(code..code + len)
                    .ok_or_else(|| err("Huffman table"))?
                {
                    *x = s as u8;
                }
                rank_idx[b as usize] += len;
            }
        }
        Ok(Huf {
            max_bits,
            sym,
            bits,
        })
    }

    /// Reads a tree description, returns the table and the bytes used.
    pub(super) fn read(d: &[u8]) -> R<(Huf, usize)> {
        let h = *d.first().ok_or_else(|| err("Huffman header"))? as usize;
        if h < 128 {
            let src = d
                .get(1..1 + h)
                .ok_or_else(|| err("Huffman weights past the end"))?;
            let (t, used) = Fse::read(src, 6, 255)?;
            let mut b = Back::new(&src[used..])?;
            let mut w = Vec::new();
            let (mut s1, mut s2) = (t.init(&mut b), t.init(&mut b));
            loop {
                w.push(t.sym[s1]);
                t.update(&mut s1, &mut b);
                if b.pos < 0 {
                    w.push(t.sym[s2]);
                    break;
                }
                w.push(t.sym[s2]);
                t.update(&mut s2, &mut b);
                if b.pos < 0 {
                    w.push(t.sym[s1]);
                    break;
                }
                if w.len() > 255 {
                    return Err(err("Huffman weights overflow"));
                }
            }
            Ok((Huf::from_weights(w)?, 1 + h))
        } else {
            let n = h - 127;
            let src = d
                .get(1..1 + n.div_ceil(2))
                .ok_or_else(|| err("Huffman weights past the end"))?;
            let w = (0..n)
                .map(|i| {
                    if i % 2 == 0 {
                        src[i / 2] >> 4
                    } else {
                        src[i / 2] & 15
                    }
                })
                .collect();
            Ok((Huf::from_weights(w)?, 1 + n.div_ceil(2)))
        }
    }

    pub(super) fn decode_stream(&self, src: &[u8], n: usize, out: &mut Vec<u8>) -> R<()> {
        let mut b = Back::new(src)?;
        let mask = (1usize << self.max_bits) - 1;
        let mut state = b.read(self.max_bits) as usize;
        for _ in 0..n {
            out.push(self.sym[state]);
            let nb = self.bits[state] as u32;
            state = ((state << nb) | b.read(nb) as usize) & mask;
        }
        if b.pos != -(self.max_bits as i64) {
            return Err(err("Huffman stream length"));
        }
        Ok(())
    }
}
