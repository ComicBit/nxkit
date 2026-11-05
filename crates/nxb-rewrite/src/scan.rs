//! Function boundaries from `.eh_frame_hdr`, data blocks in text, the linear scan for rewrite sites.

use std::collections::{BTreeMap, HashMap, HashSet};

use nxb_a64::decode;

use crate::error::fail;
use crate::rw::{R, Rw};

/// What a rewrite site is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    Svc,
    TpidrR,
    TpidrW,
    TpidrroR,
    Cntpct,
    Cntvct,
    Ctr,
    SpAlign,
    X18,
}

impl Kind {
    /// The name `sites.tsv` and the statistics use.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Kind::Svc => "svc",
            Kind::TpidrR => "tpidr_r",
            Kind::TpidrW => "tpidr_w",
            Kind::TpidrroR => "tpidrro_r",
            Kind::Cntpct => "cntpct",
            Kind::Cntvct => "cntvct",
            Kind::Ctr => "ctr",
            Kind::SpAlign => "sp_align",
            Kind::X18 => "x18",
        }
    }
}

fn u32at(img: &[u8], o: i64) -> R<i64> {
    match img.get(o as usize..o as usize + 4) {
        Some(s) if o >= 0 => Ok(i64::from(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))),
        _ => fail(format!("index error reading {o:#x}")),
    }
}

fn i32at(img: &[u8], o: i64) -> R<i64> {
    Ok(i64::from(u32at(img, o)? as u32 as i32))
}

/// Reads an encoded pointer (`rd` of `patch_nso.py`, `_read` of `nso_code.py`), returns the value and its size.
pub(crate) fn read_enc(img: &[u8], o: i64, enc: u32, ehs: i64, wide: bool) -> R<(i64, i64)> {
    let (fmt, app) = (enc & 0xf, enc & 0x70);
    let (mut v, sz) = match fmt {
        0x3 => (u32at(img, o)?, 4),
        0xb => (i32at(img, o)?, 4),
        0x4 | 0x0 if fmt == 4 || wide => match img.get(o as usize..o as usize + 8) {
            Some(s) if o >= 0 => (
                i64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]),
                8,
            ),
            _ => return fail(format!("index error reading {o:#x}")),
        },
        0xc if wide => match img.get(o as usize..o as usize + 8) {
            Some(s) if o >= 0 => (
                i64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]),
                8,
            ),
            _ => return fail(format!("index error reading {o:#x}")),
        },
        _ => return fail(format!("pointer encoding {enc:#x} at {o:#x}")),
    };
    if app == 0x10 {
        v += o;
    }
    if app == 0x30 {
        v += ehs;
    }
    Ok((v, sz))
}

fn uleb(img: &[u8], mut o: i64) -> R<(u64, i64)> {
    let (mut r, mut sh) = (0u64, 0);
    loop {
        let b = *img
            .get(o as usize)
            .ok_or_else(|| crate::error::RewriteError::Failed("index error in uleb".into()))?;
        o += 1;
        r |= u64::from(b & 0x7f) << sh;
        sh += 7;
        if b & 0x80 == 0 {
            return Ok((r, o));
        }
    }
}

/// `(eh_frame_hdr offset, its end)` from the MOD0 header.
pub(crate) fn eh_frame_hdr(img: &[u8]) -> R<(i64, i64)> {
    let m0 = u32at(img, 4)?;
    if img.get(m0 as usize..m0 as usize + 4) != Some(b"MOD0") {
        return fail("assertion failed: no MOD0");
    }
    Ok((m0 + i32at(img, m0 + 16)?, m0 + i32at(img, m0 + 20)?))
}

/// `[(function start, FDE offset)]` of the binary search table, empty without a valid header.
fn fde_table(img: &[u8]) -> R<Vec<(i64, i64)>> {
    let (ehs, ehe) = eh_frame_hdr(img)?;
    let b = |o: i64| img.get(o as usize).copied().unwrap_or(0);
    if ehe - ehs < 12 || b(ehs) != 1 || matches!(b(ehs + 3), 0xff | 0) {
        return Ok(Vec::new());
    }
    let (penc, cenc, tenc) = (
        u32::from(b(ehs + 1)),
        u32::from(b(ehs + 2)),
        u32::from(b(ehs + 3)),
    );
    let mut o = ehs + 4;
    o += read_enc(img, o, penc, ehs, true)?.1;
    let (n, s) = read_enc(img, o, cenc, ehs, true)?;
    o += s;
    let mut out = Vec::new();
    for _ in 0..n {
        let (a, s) = read_enc(img, o, tenc, ehs, true)?;
        o += s;
        let (f, s) = read_enc(img, o, tenc, ehs, true)?;
        o += s;
        out.push((a, f));
    }
    Ok(out)
}

/// Sorted `(start, end)` of every function the FDEs describe.
pub(crate) fn fde_ranges(img: &[u8]) -> R<Vec<(i64, i64)>> {
    let (ehs, _) = eh_frame_hdr(img)?;
    let mut cies: HashMap<i64, u32> = HashMap::new();
    let mut out = Vec::new();
    for (_, f) in fde_table(img)? {
        let cptr = f + 4;
        let cie = cptr - u32at(img, cptr)?;
        let enc = match cies.get(&cie) {
            Some(e) => *e,
            None => {
                let mut p = cie + 8;
                let ver = b8(img, p)?;
                p += 1;
                let zero = img[p as usize..]
                    .iter()
                    .position(|c| *c == 0)
                    .ok_or_else(|| {
                        crate::error::RewriteError::Failed("value error: no NUL".into())
                    })?;
                let aug = img[p as usize..p as usize + zero].to_vec();
                p += aug.len() as i64 + 1;
                p = uleb(img, p)?.1;
                while b8(img, p)? & 0x80 != 0 {
                    p += 1;
                }
                p += 1;
                p = if ver == 1 { p + 1 } else { uleb(img, p)?.1 };
                let mut enc = 0u32;
                if aug.first() == Some(&b'z') {
                    p = uleb(img, p)?.1;
                    for ch in &aug[1..] {
                        match ch {
                            b'R' => {
                                enc = u32::from(b8(img, p)?);
                                p += 1;
                            }
                            b'P' => {
                                let pe = u32::from(b8(img, p)?);
                                p += 1;
                                p += read_enc(img, p, pe, ehs, true)?.1;
                            }
                            b'L' => p += 1,
                            _ => {}
                        }
                    }
                }
                cies.insert(cie, enc);
                enc
            }
        };
        let (st, s) = read_enc(img, f + 8, enc, ehs, true)?;
        let (ln, _) = read_enc(img, f + 8 + s, enc & 0xf, ehs, true)?;
        out.push((st, st + ln));
    }
    out.sort_unstable();
    Ok(out)
}

fn b8(img: &[u8], o: i64) -> R<u8> {
    img.get(o as usize)
        .copied()
        .ok_or_else(|| crate::error::RewriteError::Failed("index error".into()))
}

const DATA_MIN: i64 = 0x1000;

/// Sorted `(start, end)` of the data blocks in text `[0, text_end)`.
pub(crate) fn data_blocks(img: &[u8], text_end: i64) -> R<Vec<(i64, i64)>> {
    let nro = img.get(0x10..0x14) == Some(b"NRO0");
    let mut gaps = Vec::new();
    let mut last = if nro { 0x80 } else { 0 };
    for (s, e) in fde_ranges(img)? {
        if s >= text_end {
            break;
        }
        if s > last {
            gaps.push((last, s));
        }
        last = last.max(e);
    }
    if last < text_end {
        gaps.push((last, text_end));
    }
    let mut out = if nro { vec![(8, 0x80)] } else { Vec::new() };
    for (s, e) in gaps {
        let (s, e) = ((s + 3) & !3, e & !3);
        if e - s < DATA_MIN {
            continue;
        }
        let mut undecodable = 0i64;
        let mut a = s;
        while a < e {
            let w = u32::from_le_bytes([
                img[a as usize],
                img[a as usize + 1],
                img[a as usize + 2],
                img[a as usize + 3],
            ]);
            if decode(w, a as u64).is_none() {
                undecodable += 1;
            }
            a += 4;
        }
        if undecodable * 4 > (e - s) / 4 {
            out.push((s, e));
        }
    }
    Ok(out)
}

/// The scan results.
pub(crate) struct Scan {
    pub(crate) starts: Vec<i64>,
    pub(crate) data_blocks: Vec<(i64, i64)>,
    pub(crate) kinds: BTreeMap<i64, (Kind, i64)>,
    pub(crate) spin_cand: BTreeMap<i64, Vec<i64>>,
}

const SPIN_WIN: i64 = 512;

impl Rw {
    /// Function starts from the `.eh_frame_hdr` table.
    pub(crate) fn function_starts(&self) -> R<Vec<i64>> {
        let img = &self.img;
        let m0 = u32at(img, 4)?;
        if img.get(m0 as usize..m0 as usize + 4) != Some(b"MOD0") {
            return fail("assertion failed");
        }
        let ehs = m0 + i32at(img, m0 + 16)?;
        let b = |o: i64| -> R<u32> { Ok(u32::from(b8(img, o)?)) };
        let (penc, cenc, tenc) = (b(ehs + 1)?, b(ehs + 2)?, b(ehs + 3)?);
        let mut o = ehs + 4;
        o += read_enc(img, o, penc, ehs, false)?.1;
        let (nfde, s) = read_enc(img, o, cenc, ehs, false)?;
        o += s;
        let mut set = HashSet::new();
        for _ in 0..nfde {
            let (a, s) = read_enc(img, o, tenc, ehs, false)?;
            o += s;
            o += read_enc(img, o, tenc, ehs, false)?.1;
            set.insert(a);
        }
        let mut v: Vec<i64> = set.into_iter().filter(|x| *x < self.text_end).collect();
        v.sort_unstable();
        Ok(v)
    }

    /// The linear scan: data blocks, x18 words, site kinds and spin wait candidates.
    pub(crate) fn scan(&self) -> R<Scan> {
        let starts = self.function_starts()?;
        let blocks = data_blocks(&self.img, self.text_end)?;
        let mut code_ranges = Vec::new();
        let mut from = 0;
        for (s, e) in &blocks {
            code_ranges.push((from, *s));
            from = *e;
        }
        code_ranges.push((from, self.text_end));
        // zip([0] + ends, starts + [TEXT_END])
        let mut kinds = BTreeMap::new();
        let mut spin_cand: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
        for (s, e) in &code_ranges {
            let mut o = *s;
            while o < *e {
                let w = self.w32(o);
                let t = w >> 24;
                if matches!(
                    t,
                    0x16 | 0x17 | 0x54 | 0x34 | 0x35 | 0xb4 | 0xb5 | 0x36 | 0x37 | 0xb6 | 0xb7
                ) {
                    let d: i64 = if t == 0x16 || t == 0x17 {
                        ((i64::from(w & 0x3ff_ffff)) - (1 << 26)) * 4
                    } else if t & 0x7e == 0x36 {
                        if w & 0x40000 != 0 {
                            ((i64::from((w >> 5) & 0x3fff)) - (1 << 14)) * 4
                        } else {
                            4
                        }
                    } else if t == 0x54 {
                        if w & 0x80_0000 != 0 && w & 0x10 == 0 {
                            ((i64::from((w >> 5) & 0x7_ffff)) - (1 << 19)) * 4
                        } else {
                            4
                        }
                    } else if w & 0x80_0000 != 0 {
                        ((i64::from((w >> 5) & 0x7_ffff)) - (1 << 19)) * 4
                    } else {
                        4
                    };
                    if (-SPIN_WIN..=0).contains(&d) {
                        spin_cand.entry(o + d).or_default().push(o);
                    }
                }
                let kind = if w & 0xffe0_001f == 0xd400_0001 {
                    Some((Kind::Svc, i64::from((w >> 5) & 0xffff)))
                } else if w & 0xffff_ffe0 == 0xd53b_d040 {
                    Some((Kind::TpidrR, 0))
                } else if w & 0xffff_ffe0 == 0xd51b_d040 {
                    Some((Kind::TpidrW, 0))
                } else if w & 0xffff_ffe0 == 0xd53b_d060 {
                    Some((Kind::TpidrroR, 0))
                } else if w & 0xffff_ffe0 == 0xd53b_e020 {
                    Some((Kind::Cntpct, i64::from(w & 31)))
                } else if w & 0xffff_ffe0 == 0xd53b_e040 {
                    Some((Kind::Cntvct, i64::from(w & 31)))
                } else if w & 0xffff_ffe0 == 0xd53b_0020 {
                    Some((Kind::Ctr, i64::from(w & 31)))
                } else if w & 0xffe0_ffff == 0xcb20_63ff && (w >> 16) & 31 != 18 {
                    Some((Kind::SpAlign, i64::from((w >> 16) & 31)))
                } else if decode(w, o as u64).is_some_and(|i| i.has18()) {
                    Some((Kind::X18, 0))
                } else {
                    None
                };
                if let Some(k) = kind {
                    kinds.insert(o, k);
                }
                o += 4;
            }
        }
        Ok(Scan {
            starts,
            data_blocks: blocks,
            kinds,
            spin_cand,
        })
    }
}

/// Big functions are analysed in pieces of about this size.
pub(crate) const BIG_FN: i64 = 0x40000;
const PIECE: i64 = 0x40000;
const PIECE_B: i64 = 0x10000;

/// `[(start, end)]` covering `[fa, fb)`, and the far `b` / `bl` in it as `[(site, target)]`.
/// Address ranges.
pub(crate) type Ranges = Vec<(i64, i64)>;

#[allow(clippy::needless_late_init, clippy::needless_range_loop)]
pub(crate) fn pieces(rw: &Rw, fa: i64, fb: i64) -> (Ranges, Ranges) {
    let n = ((fb - fa) / 4) as usize;
    let w = |i: usize| rw.w32(fa + 4 * i as i64);
    let mut cross = vec![0i64; n + 2];
    let mut far = Vec::new();
    for i in 0..n {
        let x = w(i);
        let t = x >> 24;
        let d: i64;
        if (t == 0x54 && x & 0x10 == 0) || t & 0x7e == 0x34 {
            let v = i64::from((x >> 5) & 0x7_ffff);
            d = v - ((v & 0x4_0000) << 1);
        } else if t & 0x7e == 0x36 {
            let v = i64::from((x >> 5) & 0x3fff);
            d = v - ((v & 0x2000) << 1);
        } else if t & 0x7c == 0x14 {
            let v = i64::from(x & 0x3ff_ffff);
            let dd = v - ((v & 0x200_0000) << 1);
            if t & 0x80 != 0 || 4 * dd.abs() > PIECE_B {
                far.push((fa + 4 * i as i64, fa + 4 * (i as i64 + dd)));
                continue;
            }
            d = dd;
        } else {
            continue;
        }
        let i = i as i64;
        let lo = i.min(i + d).max(-1);
        let hi = i.max(i + d).min(n as i64);
        cross[(lo + 1) as usize] += 1;
        cross[(hi + 1) as usize] -= 1;
    }
    let mut cand = Vec::new();
    let mut c = 0;
    for k in 0..n {
        c += cross[k];
        let x = if k > 0 { w(k - 1) } else { 0 };
        if k > 0
            && c == 0
            && (matches!(x & 0xffff_fc1f, 0xd65f_0000 | 0xd61f_0000) || x >> 26 == 0x05)
        {
            cand.push(k as i64);
        }
    }
    let mut cuts = Vec::new();
    let mut s = 0i64;
    let nn = n as i64;
    while s + PIECE / 4 < nn {
        let right = |v: i64| cand.partition_point(|c| *c <= v) as i64;
        let mut j = right(s + PIECE / 4) - 1;
        if j < 0 || cand[j as usize] <= s {
            j = right(s + PIECE / 4);
        }
        if j >= cand.len() as i64 {
            break;
        }
        s = cand[j as usize];
        cuts.push(s);
    }
    let mut b = vec![0i64];
    b.extend(cuts);
    b.push(nn);
    let ps = b
        .windows(2)
        .map(|p| (fa + 4 * p[0], fa + 4 * p[1]))
        .collect();
    (ps, far)
}
