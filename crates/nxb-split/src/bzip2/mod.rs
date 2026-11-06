//! A bzip2 decoder (zip members of method 12, `bz2.BZ2Decompressor` of Python's `zipfile`): one stream of blocks, the
//! block and stream CRCs checked. The obsolete randomised blocks are `Unsupported`.

use crate::SplitError;

#[cfg(test)]
mod tests;

fn bad(m: &str) -> SplitError {
    SplitError::Red(format!("OSError: {m}"))
}

const BLOCK_MAGIC: u64 = 0x3141_5926_5359;
const END_MAGIC: u64 = 0x1772_4538_5090;
const MAX_LEN: usize = 20;

/// The MSB-first CRC-32 of bzip2.
const CRC: [u32; 256] = {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = (i as u32) << 24;
        let mut k = 0;
        while k < 8 {
            c = if c & 0x8000_0000 != 0 {
                (c << 1) ^ 0x04c1_1db7
            } else {
                c << 1
            };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
};

struct Bits<'a> {
    d: &'a [u8],
    p: usize,
    acc: u64,
    n: u32,
}

impl Bits<'_> {
    fn get(&mut self, k: u32) -> Result<u32, SplitError> {
        while self.n < k {
            let b = *self.d.get(self.p).ok_or_else(|| {
                bad("Compressed file ended before the end-of-stream marker was reached")
            })?;
            self.p += 1;
            self.acc = (self.acc << 8 | u64::from(b)) & 0xffff_ffff_ffff;
            self.n += 8;
        }
        self.n -= k;
        Ok(((self.acc >> self.n) & ((1u64 << k) - 1)) as u32)
    }
    fn bit(&mut self) -> Result<u32, SplitError> {
        self.get(1)
    }
}

/// The canonical decoding table of one Huffman code (`hbCreateDecodeTables`).
struct Table {
    limit: [i32; MAX_LEN + 2],
    base: [i32; MAX_LEN + 2],
    perm: Vec<u16>,
    min: usize,
}

impl Table {
    fn new(len: &[u8]) -> Table {
        let min = len.iter().copied().min().map_or(1, usize::from);
        let max = len.iter().copied().max().map_or(1, usize::from);
        let mut perm = Vec::with_capacity(len.len());
        for l in min..=max {
            for (s, &x) in len.iter().enumerate() {
                if usize::from(x) == l {
                    perm.push(s as u16);
                }
            }
        }
        let mut base = [0i32; MAX_LEN + 2];
        for &l in len {
            base[usize::from(l) + 1] += 1;
        }
        for i in 1..base.len() {
            base[i] += base[i - 1];
        }
        let mut limit = [-1i32; MAX_LEN + 2];
        let mut v = 0i32;
        for i in min..=max {
            v += base[i + 1] - base[i];
            limit[i] = v - 1;
            v <<= 1;
        }
        for i in min + 1..=max {
            base[i] = ((limit[i - 1] + 1) << 1) - base[i];
        }
        Table {
            limit,
            base,
            perm,
            min,
        }
    }
    fn symbol(&self, r: &mut Bits<'_>) -> Result<usize, SplitError> {
        let mut n = self.min;
        let mut v = r.get(n as u32)? as i32;
        while v > self.limit[n] {
            n += 1;
            if n > MAX_LEN {
                return Err(bad("Invalid data stream"));
            }
            v = v << 1 | r.bit()? as i32;
        }
        self.perm
            .get((v - self.base[n]) as usize)
            .map(|&s| usize::from(s))
            .ok_or_else(|| bad("Invalid data stream"))
    }
}

/// The block's symbols after the move-to-front and run-length coding of the zero runs.
fn block_symbols(r: &mut Bits<'_>, max: usize) -> Result<Vec<u8>, SplitError> {
    let mut used = Vec::new();
    let groups = r.get(16)?;
    for g in 0..16 {
        if groups & (0x8000 >> g) != 0 {
            let m = r.get(16)?;
            for i in 0..16 {
                if m & (0x8000 >> i) != 0 {
                    used.push((g * 16 + i) as u8);
                }
            }
        }
    }
    if used.is_empty() {
        return Err(bad("Invalid data stream"));
    }
    let alpha = used.len() + 2;
    let ngroups = r.get(3)? as usize;
    let nsel = r.get(15)? as usize;
    if !(2..=6).contains(&ngroups) || nsel == 0 {
        return Err(bad("Invalid data stream"));
    }
    let mut mtf: Vec<u8> = (0..ngroups as u8).collect();
    let mut sel = Vec::with_capacity(nsel);
    for _ in 0..nsel {
        let mut j = 0;
        while r.bit()? == 1 {
            j += 1;
            if j >= ngroups {
                return Err(bad("Invalid data stream"));
            }
        }
        let v = mtf.remove(j);
        mtf.insert(0, v);
        sel.push(v);
    }
    let mut tables = Vec::with_capacity(ngroups);
    for _ in 0..ngroups {
        let mut cur = r.get(5)? as i32;
        let mut len = Vec::with_capacity(alpha);
        for _ in 0..alpha {
            loop {
                if !(1..=MAX_LEN as i32).contains(&cur) {
                    return Err(bad("Invalid data stream"));
                }
                if r.bit()? == 0 {
                    break;
                }
                cur += if r.bit()? == 0 { 1 } else { -1 };
            }
            len.push(cur as u8);
        }
        tables.push(Table::new(&len));
    }
    let eob = alpha - 1;
    let mut order = used;
    let mut out: Vec<u8> = Vec::new();
    let (mut run, mut weight) = (0usize, 1usize);
    let mut count = 0usize;
    loop {
        let t = sel
            .get(count / 50)
            .ok_or_else(|| bad("Invalid data stream"))?;
        count += 1;
        let s = tables[usize::from(*t)].symbol(r)?;
        if s < 2 {
            run += weight << s;
            weight <<= 1;
            if run > max {
                return Err(bad("Invalid data stream"));
            }
            continue;
        }
        if run > 0 {
            if out.len() + run > max {
                return Err(bad("Invalid data stream"));
            }
            out.resize(out.len() + run, order[0]);
            (run, weight) = (0, 1);
        }
        if s == eob {
            return Ok(out);
        }
        if out.len() >= max {
            return Err(bad("Invalid data stream"));
        }
        let v = order.remove(s - 1);
        order.insert(0, v);
        out.push(v);
    }
}

/// Undoes the Burrows-Wheeler transform and the first run-length coding, appends to `out` and returns the block CRC.
fn unblock(last: &[u8], orig: usize, out: &mut Vec<u8>, limit: usize) -> Result<u32, SplitError> {
    let n = last.len();
    if orig >= n {
        return Err(bad("Invalid data stream"));
    }
    let mut cf = [0usize; 257];
    for &b in last {
        cf[usize::from(b) + 1] += 1;
    }
    for i in 1..257 {
        cf[i] += cf[i - 1];
    }
    let mut next = vec![0u32; n];
    for (i, &b) in last.iter().enumerate() {
        next[cf[usize::from(b)]] = i as u32;
        cf[usize::from(b)] += 1;
    }
    let start = out.len();
    let mut pos = next[orig] as usize;
    let (mut prev, mut same) = (None::<u8>, 0u32);
    for _ in 0..n {
        let b = last[pos];
        pos = next[pos] as usize;
        if same == 4 {
            if out.len() + usize::from(b) > limit {
                return Err(bad("the output is longer than its size"));
            }
            out.resize(out.len() + usize::from(b), prev.unwrap_or(0));
            (prev, same) = (None, 0);
            continue;
        }
        same = if prev == Some(b) { same + 1 } else { 1 };
        prev = Some(b);
        if out.len() >= limit {
            return Err(bad("the output is longer than its size"));
        }
        out.push(b);
    }
    let mut crc = 0xffff_ffffu32;
    for &b in &out[start..] {
        crc = (crc << 8) ^ CRC[usize::from((crc >> 24) as u8 ^ b)];
    }
    Ok(!crc)
}

/// Decodes one bzip2 stream of `raw` to at most `size` bytes (more output is an error).
pub fn decompress(raw: &[u8], size: usize) -> Result<Vec<u8>, SplitError> {
    if raw.get(..3) != Some(b"BZh") {
        return Err(bad("Invalid data stream"));
    }
    let level = match raw.get(3) {
        Some(c @ b'1'..=b'9') => usize::from(c - b'0'),
        _ => return Err(bad("Invalid data stream")),
    };
    let mut r = Bits {
        d: &raw[4..],
        p: 0,
        acc: 0,
        n: 0,
    };
    let mut out: Vec<u8> = Vec::with_capacity(size.min(1 << 28));
    let mut total = 0u32;
    loop {
        let magic = u64::from(r.get(24)?) << 24 | u64::from(r.get(24)?);
        let crc = r.get(16)? << 16 | r.get(16)?;
        if magic == END_MAGIC {
            if crc != total {
                return Err(bad("Invalid data stream"));
            }
            return Ok(out);
        }
        if magic != BLOCK_MAGIC {
            return Err(bad("Invalid data stream"));
        }
        if r.bit()? == 1 {
            return Err(SplitError::Unsupported("a randomised bzip2 block".into()));
        }
        let orig = r.get(24)? as usize;
        let last = block_symbols(&mut r, level * 100_000)?;
        let got = unblock(&last, orig, &mut out, size)?;
        if got != crc {
            return Err(bad("Invalid data stream"));
        }
        total = total.rotate_left(1) ^ got;
    }
}
