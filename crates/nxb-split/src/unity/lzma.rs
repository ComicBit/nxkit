//! A raw LZMA1 decoder (Unity's bundle blocks: five property bytes, then the stream), `lzma.LZMADecompressor(FORMAT_RAW)`
//! with the properties of the block and an output limit.

use crate::SplitError;

fn bad(m: &str) -> SplitError {
    SplitError::Red(format!("lzma.LZMAError: {m}"))
}

struct Rc<'a> {
    d: &'a [u8],
    p: usize,
    range: u32,
    code: u32,
}

const BITS: u32 = 11;
const TOP: u32 = 1 << 24;

impl<'a> Rc<'a> {
    fn new(d: &'a [u8]) -> Result<Rc<'a>, SplitError> {
        if d.len() < 5 || d[0] != 0 {
            return Err(bad("corrupt input data"));
        }
        let code = u32::from_be_bytes([d[1], d[2], d[3], d[4]]);
        Ok(Rc {
            d,
            p: 5,
            range: 0xffff_ffff,
            code,
        })
    }
    fn norm(&mut self) -> Result<(), SplitError> {
        if self.range < TOP {
            let b = *self
                .d
                .get(self.p)
                .ok_or_else(|| bad("input ends inside the stream"))?;
            self.p += 1;
            self.range <<= 8;
            self.code = self.code << 8 | u32::from(b);
        }
        Ok(())
    }
    fn bit(&mut self, p: &mut u16) -> Result<u32, SplitError> {
        self.norm()?;
        let bound = (self.range >> BITS) * u32::from(*p);
        if self.code < bound {
            self.range = bound;
            *p += (2048 - *p) >> 5;
            Ok(0)
        } else {
            self.range -= bound;
            self.code -= bound;
            *p -= *p >> 5;
            Ok(1)
        }
    }
    fn direct(&mut self, n: u32) -> Result<u32, SplitError> {
        let mut r = 0;
        for _ in 0..n {
            self.norm()?;
            self.range >>= 1;
            let t = if self.code >= self.range {
                self.code -= self.range;
                1
            } else {
                0
            };
            r = r << 1 | t;
        }
        Ok(r)
    }
    fn tree(&mut self, probs: &mut [u16], n: u32) -> Result<u32, SplitError> {
        let mut m = 1usize;
        for _ in 0..n {
            m = m << 1 | self.bit(&mut probs[m])? as usize;
        }
        Ok(m as u32 - (1 << n))
    }
    fn rtree(&mut self, probs: &mut [u16], n: u32) -> Result<u32, SplitError> {
        let (mut m, mut r) = (1usize, 0u32);
        for i in 0..n {
            let b = self.bit(&mut probs[m])?;
            m = m << 1 | b as usize;
            r |= b << i;
        }
        Ok(r)
    }
}

struct Len {
    choice: [u16; 2],
    low: Vec<[u16; 8]>,
    mid: Vec<[u16; 8]>,
    high: [u16; 256],
}

impl Len {
    fn new() -> Len {
        Len {
            choice: [1024; 2],
            low: vec![[1024; 8]; 16],
            mid: vec![[1024; 8]; 16],
            high: [1024; 256],
        }
    }
    fn decode(&mut self, rc: &mut Rc<'_>, ps: usize) -> Result<u32, SplitError> {
        if rc.bit(&mut self.choice[0])? == 0 {
            return rc.tree(&mut self.low[ps], 3);
        }
        if rc.bit(&mut self.choice[1])? == 0 {
            return Ok(8 + rc.tree(&mut self.mid[ps], 3)?);
        }
        Ok(16 + rc.tree(&mut self.high, 8)?)
    }
}

/// Decodes `raw` (the five property bytes, then the stream) to at most `size` bytes.
pub fn decompress(raw: &[u8], size: usize) -> Result<Vec<u8>, SplitError> {
    if raw.len() < 5 {
        return Err(bad("the block is shorter than its properties"));
    }
    let props = raw[0] as u32;
    if props >= 9 * 5 * 5 {
        return Err(bad("invalid properties"));
    }
    let (lc, lp, pb) = (props % 9, (props / 9) % 5, props / 45);
    let mut rc = Rc::new(&raw[5..])?;
    let mut out: Vec<u8> = Vec::with_capacity(size.min(1 << 28));
    let mut lit = vec![1024u16; 0x300 << (lc + lp)];
    let mut is_match = [[1024u16; 16]; 12];
    let (mut is_rep, mut is_rep0, mut is_rep1, mut is_rep2) =
        ([1024u16; 12], [1024u16; 12], [1024u16; 12], [1024u16; 12]);
    let mut is_rep0_long = [[1024u16; 16]; 12];
    let mut slot = vec![[1024u16; 64]; 4];
    let mut spec = [1024u16; 115];
    let mut align = [1024u16; 16];
    let (mut ml, mut rl) = (Len::new(), Len::new());
    let (mut state, mut reps) = (0usize, [0u32; 4]);
    let pbm = (1usize << pb) - 1;
    let lpm = (1usize << lp) - 1;
    while out.len() < size {
        let pos = out.len();
        let ps = pos & pbm;
        if rc.bit(&mut is_match[state][ps])? == 0 {
            let prev = out.last().copied().unwrap_or(0) as usize;
            let base = 0x300 * (((pos & lpm) << lc) + (prev >> (8 - lc)));
            let probs = &mut lit[base..base + 0x300];
            let mut sym = 1usize;
            if state >= 7 {
                let mut mb = out[pos - reps[0] as usize - 1] as usize;
                while sym < 0x100 {
                    let m = (mb >> 7) & 1;
                    mb <<= 1;
                    let b = rc.bit(&mut probs[((1 + m) << 8) + sym])? as usize;
                    sym = sym << 1 | b;
                    if m != b {
                        break;
                    }
                }
            }
            while sym < 0x100 {
                sym = sym << 1 | rc.bit(&mut probs[sym])? as usize;
            }
            out.push(sym as u8);
            state = if state < 4 {
                0
            } else if state < 10 {
                state - 3
            } else {
                state - 6
            };
            continue;
        }
        let len;
        if rc.bit(&mut is_rep[state])? == 0 {
            reps = [reps[0], reps[0], reps[1], reps[2]];
            len = ml.decode(&mut rc, ps)?;
            state = if state < 7 { 7 } else { 10 };
            let ls = (len.min(3)) as usize;
            let s = rc.tree(&mut slot[ls], 6)?;
            let dist = if s < 4 {
                s
            } else {
                let nb = (s >> 1) - 1;
                let mut d = (2 | (s & 1)) << nb;
                if s < 14 {
                    let base = (d - s) as usize;
                    let mut m = 1usize;
                    let mut r = 0u32;
                    for i in 0..nb {
                        let b = rc.bit(&mut spec[base + m])?;
                        m = m << 1 | b as usize;
                        r |= b << i;
                    }
                    d += r;
                } else {
                    d += rc.direct(nb - 4)? << 4;
                    d += rc.rtree(&mut align, 4)?;
                }
                d
            };
            reps[0] = dist;
            if dist == 0xffff_ffff {
                break;
            }
        } else {
            if pos == 0 {
                return Err(bad("corrupt input data"));
            }
            if rc.bit(&mut is_rep0[state])? == 0 {
                if rc.bit(&mut is_rep0_long[state][ps])? == 0 {
                    state = if state < 7 { 9 } else { 11 };
                    let b = out[pos - reps[0] as usize - 1];
                    out.push(b);
                    continue;
                }
            } else {
                let d;
                if rc.bit(&mut is_rep1[state])? == 0 {
                    d = reps[1];
                } else {
                    if rc.bit(&mut is_rep2[state])? == 0 {
                        d = reps[2];
                    } else {
                        d = reps[3];
                        reps[3] = reps[2];
                    }
                    reps[2] = reps[1];
                }
                reps[1] = reps[0];
                reps[0] = d;
            }
            len = rl.decode(&mut rc, ps)?;
            state = if state < 7 { 8 } else { 11 };
        }
        let len = len as usize + 2;
        let dist = reps[0] as usize + 1;
        if dist > out.len() {
            return Err(bad("corrupt input data"));
        }
        for _ in 0..len.min(size - out.len()) {
            let b = out[out.len() - dist];
            out.push(b);
        }
    }
    Ok(out)
}
