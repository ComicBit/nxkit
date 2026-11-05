//! MonoGame's compressed XNB files (`xnb_decompress`): LZX as XMemCompress writes it (a CHMLib style decoder with a
//! 2^16 window inside XNB block framing) and one raw LZ4 block.

use crate::SplitError;
use crate::shaders::u32le;

const EXTRA: [u8; 51] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13, 14, 14, 15, 15, 16, 16, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
];
const BASE: [u32; 51] = [
    0, 1, 2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64, 96, 128, 192, 256, 384, 512, 768, 1024, 1536,
    2048, 3072, 4096, 6144, 8192, 12288, 16384, 24576, 32768, 49152, 65536, 98304, 131072, 196608,
    262144, 393216, 524288, 655360, 786432, 917504, 1048576, 1179648, 1310720, 1441792, 1572864,
    1703936, 1835008, 1966080, 2097152,
];
const WS: usize = 1 << 16;

type R<T> = Result<T, SplitError>;

fn lzx(m: &str) -> SplitError {
    SplitError::Red(format!("LzxError: {m}"))
}

fn at(t: &mut [u32], i: usize, v: u32) -> R<()> {
    *t.get_mut(i)
        .ok_or_else(|| lzx("IndexError in a decode table"))? = v;
    Ok(())
}

/// `_lzx_table`: a huffman lookup table out of canonical code lengths.
fn table(nsyms: usize, nbits: u32, length: &[u8]) -> R<Vec<u32>> {
    let mut t = vec![0u32; (1 << nbits) + nsyms * 2];
    let (mut pos, mut bit_num, mut mask) = (0usize, 1u32, 1usize << nbits);
    let mut bit_mask = mask >> 1;
    let mut next = bit_mask;
    let len = |s: usize| length.get(s).copied().unwrap_or(0);
    while bit_num <= nbits {
        for sym in 0..nsyms {
            if u32::from(len(sym)) == bit_num {
                let leaf = pos;
                pos += bit_mask;
                if pos > mask {
                    return Err(lzx("decode table overrun"));
                }
                t[leaf..pos].fill(sym as u32);
            }
        }
        bit_mask >>= 1;
        bit_num += 1;
    }
    if pos != mask {
        t[pos..mask].fill(0);
        pos <<= 16;
        mask <<= 16;
        bit_mask = 1 << 15;
        while bit_num <= 16 {
            for sym in 0..nsyms {
                if u32::from(len(sym)) == bit_num {
                    let mut leaf = pos >> 16;
                    for fill in 0..(bit_num - nbits) {
                        if *t.get(leaf).ok_or_else(|| lzx("IndexError"))? == 0 {
                            at(&mut t, next << 1, 0)?;
                            at(&mut t, (next << 1) + 1, 0)?;
                            at(&mut t, leaf, next as u32)?;
                            next += 1;
                        }
                        leaf = (*t.get(leaf).ok_or_else(|| lzx("IndexError"))? as usize) << 1;
                        if (pos >> (15 - fill)) & 1 == 1 {
                            leaf += 1;
                        }
                    }
                    at(&mut t, leaf, sym as u32)?;
                    pos += bit_mask;
                    if pos > mask {
                        return Err(lzx("decode table overflow"));
                    }
                }
            }
            bit_mask >>= 1;
            bit_num += 1;
        }
    }
    if pos != mask && length.iter().take(nsyms).any(|&l| l != 0) {
        return Err(lzx("a bad or incomplete huffman table"));
    }
    Ok(t)
}

/// The bit reader over one frame (16-bit little-endian words, an overread past the end reads zeros).
struct Bits {
    d: Vec<u8>,
    ip: usize,
    bb: u64,
    bl: u32,
}

impl Bits {
    fn ensure(&mut self, n: u32) {
        while self.bl < n {
            let w = if self.ip + 1 < self.d.len() {
                u64::from(self.d[self.ip]) | u64::from(self.d[self.ip + 1]) << 8
            } else {
                0
            };
            self.bb = (self.bb << 16) | w;
            self.bl += 16;
            self.ip += 2;
        }
    }

    fn remove(&mut self, n: u32) {
        self.bl -= n;
        self.bb &= (1u64 << self.bl) - 1;
    }

    fn read(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        self.ensure(n);
        let v = (self.bb >> (self.bl - n)) as u32;
        self.remove(n);
        v
    }

    fn sym(&mut self, table: &[u32], nbits: u32, maxsyms: u32, lens: &[u8]) -> R<usize> {
        self.ensure(16);
        let mut i = *table
            .get((self.bb >> (self.bl - nbits)) as usize)
            .ok_or_else(|| lzx("IndexError"))?;
        if i >= maxsyms {
            let mut k = 0u32;
            loop {
                if i64::from(self.bl) - i64::from(nbits) - 1 - i64::from(k) < 0 {
                    return Err(lzx("a huffman code longer than the bitstream"));
                }
                i = (i << 1) | ((self.bb >> (self.bl - nbits - 1 - k)) & 1) as u32;
                k += 1;
                i = *table.get(i as usize).ok_or_else(|| lzx("IndexError"))?;
                if i < maxsyms {
                    break;
                }
            }
        }
        let l = *lens.get(i as usize).ok_or_else(|| lzx("IndexError"))?;
        self.remove(u32::from(l));
        Ok(i as usize)
    }

    /// `_read_lens`: code lengths as deltas against the previous ones.
    fn lens(&mut self, lens: &mut [u8], first: usize, last: usize) -> R<()> {
        let pre_len: Vec<u8> = (0..20).map(|_| self.read(4) as u8).collect();
        let pre = table(20, 6, &pre_len)?;
        let mut x = first;
        let put = |lens: &mut [u8], x: usize, v: u8| -> R<()> {
            *lens.get_mut(x).ok_or_else(|| lzx("IndexError"))? = v;
            Ok(())
        };
        while x < last {
            let z = self.sym(&pre, 6, 20, &pre_len)?;
            let delta = |lens: &[u8], x: usize, z: usize| -> R<u8> {
                let v = i32::from(*lens.get(x).ok_or_else(|| lzx("IndexError"))?) - z as i32;
                Ok(if v < 0 { v + 17 } else { v } as u8)
            };
            match z {
                17 | 18 => {
                    let y = if z == 17 {
                        self.read(4) + 4
                    } else {
                        self.read(5) + 20
                    };
                    for _ in 0..y {
                        put(lens, x, 0)?;
                        x += 1;
                    }
                }
                19 => {
                    let y = self.read(1) + 4;
                    let z = self.sym(&pre, 6, 20, &pre_len)?;
                    let v = delta(lens, x, z)?;
                    for _ in 0..y {
                        put(lens, x, v)?;
                        x += 1;
                    }
                }
                _ => {
                    let v = delta(lens, x, z)?;
                    put(lens, x, v)?;
                    x += 1;
                }
            }
        }
        Ok(())
    }
}

/// `LzxState`: the decoder state of a whole XNB.
struct State {
    window: Vec<u8>,
    posn: usize,
    r: [usize; 3],
    header_read: bool,
    block_type: u32,
    block_remaining: usize,
    block_length: usize,
    intel_filesize: i64,
    intel_curpos: i64,
    intel_started: bool,
    frames_read: u32,
    main_len: Vec<u8>,
    length_len: Vec<u8>,
    main_tbl: Vec<u32>,
    length_tbl: Vec<u32>,
    al_tbl: Vec<u32>,
    al_len: Vec<u8>,
    b: Bits,
}

impl State {
    fn new() -> State {
        State {
            window: vec![0; WS],
            posn: 0,
            r: [1; 3],
            header_read: false,
            block_type: 0,
            block_remaining: 0,
            block_length: 0,
            intel_filesize: 0,
            intel_curpos: 0,
            intel_started: false,
            frames_read: 0,
            main_len: vec![0; 256 + 400 + 64],
            length_len: vec![0; 249 + 1 + 64],
            main_tbl: Vec::new(),
            length_tbl: Vec::new(),
            al_tbl: Vec::new(),
            al_len: Vec::new(),
            b: Bits {
                d: Vec::new(),
                ip: 0,
                bb: 0,
                bl: 0,
            },
        }
    }

    fn le(&mut self) -> usize {
        let b = &mut self.b;
        let mut v = 0usize;
        for i in 0..4 {
            v |= usize::from(b.d.get(b.ip + i).copied().unwrap_or(0)) << (8 * i);
        }
        b.ip += 4;
        v
    }

    /// `decompress`: one frame of `outlen` bytes.
    fn decompress(&mut self, src: &[u8], outlen: usize) -> R<Vec<u8>> {
        self.b = Bits {
            d: src.to_vec(),
            ip: 0,
            bb: 0,
            bl: 0,
        };
        let mut togo = outlen;
        if !self.header_read {
            if self.b.read(1) != 0 {
                self.intel_filesize = i64::from(self.b.read(16)) << 16 | i64::from(self.b.read(16));
            }
            self.header_read = true;
        }
        while togo > 0 {
            if self.block_remaining == 0 {
                if self.block_type == 3 {
                    if self.block_length & 1 == 1 {
                        self.b.ip += 1;
                    }
                    self.b.bb = 0;
                    self.b.bl = 0;
                }
                self.block_type = self.b.read(3);
                let hi = self.b.read(16) as usize;
                self.block_length = (hi << 8) | self.b.read(8) as usize;
                self.block_remaining = self.block_length;
                if self.block_type == 2 {
                    self.al_len = (0..8).map(|_| self.b.read(3) as u8).collect();
                    self.al_tbl = table(8, 7, &self.al_len)?;
                }
                if self.block_type == 1 || self.block_type == 2 {
                    self.b.lens(&mut self.main_len, 0, 256)?;
                    self.b.lens(&mut self.main_len, 256, 512)?;
                    self.main_tbl = table(656, 12, &self.main_len)?;
                    if self.main_len[0xe8] != 0 {
                        self.intel_started = true;
                    }
                    self.b.lens(&mut self.length_len, 0, 249)?;
                    self.length_tbl = table(250, 12, &self.length_len)?;
                } else if self.block_type == 3 {
                    self.intel_started = true;
                    self.b.ensure(16);
                    if self.b.bl > 16 {
                        self.b.ip -= 2;
                    }
                    for k in 0..3 {
                        self.r[k] = self.le();
                    }
                } else {
                    return Err(lzx(&format!("block type {}", self.block_type)));
                }
            }
            let (ip, len) = (self.b.ip, self.b.d.len());
            if ip > len && (ip > len + 2 || self.b.bl < 16) {
                return Err(lzx("the block ended early"));
            }
            let mut run = self.block_remaining.min(togo);
            togo -= run;
            self.block_remaining -= run;
            self.posn &= WS - 1;
            if self.posn + run > WS {
                return Err(lzx("a run straddles the window"));
            }
            let mut posn = self.posn;
            if self.block_type == 3 {
                let ip = self.b.ip;
                let s = self
                    .b
                    .d
                    .get(ip..ip + run)
                    .ok_or_else(|| lzx("the uncompressed block runs past the block"))?;
                self.window[posn..posn + run].copy_from_slice(s);
                self.b.ip += run;
                posn += run;
            } else {
                while run > 0 {
                    let me = self.b.sym(&self.main_tbl, 12, 656, &self.main_len)?;
                    if me < 256 {
                        self.window[posn] = me as u8;
                        posn += 1;
                        run -= 1;
                        continue;
                    }
                    let me = me - 256;
                    let mut ml = me & 7;
                    if ml == 7 {
                        ml += self.b.sym(&self.length_tbl, 12, 250, &self.length_len)?;
                    }
                    ml += 2;
                    let mut mo = me >> 3;
                    if mo > 2 {
                        if self.block_type == 2 {
                            let mut extra = u32::from(EXTRA[mo]);
                            mo = BASE[mo] as usize - 2;
                            if extra > 3 {
                                extra -= 3;
                                let v = (self.b.read(extra) as usize) << 3;
                                mo += v + self.b.sym(&self.al_tbl, 7, 8, &self.al_len)?;
                            } else if extra == 3 {
                                mo += self.b.sym(&self.al_tbl, 7, 8, &self.al_len)?;
                            } else if extra > 0 {
                                mo += self.b.read(extra) as usize;
                            } else {
                                mo = 1;
                            }
                        } else if mo != 3 {
                            mo = BASE[mo] as usize - 2 + self.b.read(u32::from(EXTRA[mo])) as usize;
                        } else {
                            mo = 1;
                        }
                        self.r = [mo, self.r[0], self.r[1]];
                    } else if mo == 0 {
                        mo = self.r[0];
                    } else if mo == 1 {
                        mo = self.r[1];
                        self.r[1] = self.r[0];
                        self.r[0] = mo;
                    } else {
                        mo = self.r[2];
                        self.r[2] = self.r[0];
                        self.r[0] = mo;
                    }
                    posn += ml;
                    if posn > WS {
                        return Err(lzx("a match overruns the window"));
                    }
                    run = run.saturating_sub(ml);
                    let mut src = posn as i64 - ml as i64 - mo as i64;
                    if src < 0 {
                        src += WS as i64;
                    }
                    let dst = posn - ml;
                    if mo >= ml && src >= 0 && src as usize + ml <= WS {
                        self.window
                            .copy_within(src as usize..src as usize + ml, dst);
                    } else {
                        for i in 0..ml {
                            self.window[dst + i] =
                                self.window[(src + i as i64).rem_euclid(WS as i64) as usize];
                        }
                    }
                }
            }
            self.posn = posn;
        }
        let end = if self.posn != 0 { self.posn } else { WS };
        let start = end
            .checked_sub(outlen)
            .ok_or_else(|| lzx("a frame longer than the window"))?;
        let mut out = self.window[start..end].to_vec();
        self.frames_read += 1;
        if self.frames_read <= 32768 && self.intel_filesize != 0 {
            if outlen <= 6 || !self.intel_started {
                self.intel_curpos += outlen as i64;
            } else {
                let (mut cur, size) = (self.intel_curpos, self.intel_filesize);
                self.intel_curpos = cur + outlen as i64;
                let (mut pos, e) = (0usize, outlen - 10);
                while pos < e {
                    let b = out[pos];
                    pos += 1;
                    if b != 0xe8 {
                        cur += 1;
                        continue;
                    }
                    let a = i64::from(i32::from_le_bytes([
                        out[pos],
                        out[pos + 1],
                        out[pos + 2],
                        out[pos + 3],
                    ]));
                    if -cur <= a && a < size {
                        let rel = if a >= 0 { a - cur } else { a + size };
                        out[pos..pos + 4]
                            .copy_from_slice(&((rel & 0xffff_ffff) as u32).to_le_bytes());
                    }
                    pos += 4;
                    cur += 5;
                }
            }
        }
        Ok(out)
    }
}

/// `xnb_decompress`: the content of a compressed XNB (flags 0x80 LZX, 0x40 LZ4). `first_block` stops after the
/// first LZX frame.
pub fn xnb_decompress(d: &[u8], first_block: bool) -> R<Vec<u8>> {
    let idx = || SplitError::Red("IndexError: the XNB ends inside a block header".into());
    let dec_size = u32le(d, 10).ok_or_else(idx)? as usize;
    let end = u32le(d, 6).ok_or_else(idx)? as usize;
    let body = d.get(14..end.min(d.len())).unwrap_or(&[]);
    let flags = d.get(5).copied().unwrap_or(0);
    if flags & 0x80 != 0 {
        let mut st = State::new();
        let mut out = Vec::new();
        let mut p = 0usize;
        let g = |i: usize| body.get(i).copied().ok_or_else(idx);
        while out.len() < dec_size {
            let hi = g(p)?;
            p += 1;
            let (dst, src);
            if hi == 0xff {
                dst = usize::from(g(p)?) << 8 | usize::from(g(p + 1)?);
                src = usize::from(g(p + 2)?) << 8 | usize::from(g(p + 3)?);
                p += 4;
            } else {
                dst = 0x8000;
                src = usize::from(hi) << 8 | usize::from(g(p)?);
                p += 1;
            }
            if src == 0 || dst == 0 {
                break;
            }
            let frame = body.get(p..(p + src).min(body.len())).unwrap_or(&[]);
            out.extend(st.decompress(frame, dst)?);
            p += src;
            if first_block {
                break;
            }
        }
        return Ok(out);
    }
    if flags & 0x40 != 0 {
        return crate::lz4::block(body, dec_size);
    }
    Ok(d.get(14..).unwrap_or(&[]).to_vec())
}
