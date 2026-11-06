//! Engine finders that run on every module without a build ID (ports of `ue_cvar.py` and `fps_target.py`, word
//! level, as the scripts do): Unreal console variables by their registered names, and a target frame rate by the
//! code that reads and sets it. They only find places for the fixups table, the image stays as it is.

use std::collections::{BTreeMap, BTreeSet, HashMap};

const RET: u32 = 0xD65F_03C0;

pub(crate) fn word(img: &[u8], a: usize) -> u32 {
    img.get(a..a + 4)
        .map_or(0, |s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

pub(crate) fn adrp_page(w: u32, pc: i64) -> i64 {
    let mut imm = (i64::from((w >> 5) & 0x7FFFF) << 2 | i64::from((w >> 29) & 3)) << 12;
    if imm & (1 << 32) != 0 {
        imm -= 1 << 33;
    }
    (pc & !0xFFF) + imm
}

pub(crate) fn is_adrp(w: u32) -> bool {
    w & 0x9F00_0000 == 0x9000_0000
}

/// `add xD, xN, #imm` (64-bit, optionally shifted by 12): (d, n, imm).
fn add_imm(w: u32) -> Option<(u32, u32, i64)> {
    (w & 0xFF80_0000 == 0x9100_0000).then(|| {
        (
            w & 31,
            (w >> 5) & 31,
            i64::from((w >> 10) & 0xFFF) << (12 * ((w >> 22) & 1)),
        )
    })
}

fn find_from(img: &[u8], pat: &[u8], from: usize, to: usize) -> Option<usize> {
    let to = to.min(img.len());
    (from + pat.len() <= to)
        .then(|| img[from..to].windows(pat.len()).position(|w| w == pat))
        .flatten()
        .map(|p| from + p)
}

/// The object registered under the name whose reference ends at word `j` (`registered_at`).
fn registered_at(img: &[u8], text_end: usize, j: usize) -> Option<i64> {
    let w = |a: usize| word(img, a);
    let name_add = add_imm(w(j))?;
    if name_add.0 != 1 {
        return None;
    }
    let (mut val, mut name, mut registered): (HashMap<u32, i64>, bool, bool) =
        (HashMap::new(), false, false);
    let mut t = j.saturating_sub(64);
    while t < text_end.min(j + 192) {
        let u = w(t);
        let call = u & 0xFC00_0000 == 0x9400_0000 || u & 0xFFFF_FC1F == 0xD63F_0000;
        let br = u & 0xFC00_0000 == 0x1400_0000;
        if t < j && (call || br || u == RET) {
            val.clear();
            t += 4;
            continue;
        }
        if call {
            if !registered {
                if !name {
                    return None;
                }
                if u >> 26 == 0x25 {
                    return val.get(&0).copied();
                }
                registered = true;
            }
            for r in 0..19 {
                val.remove(&r);
            }
            name = false;
            t += 4;
            continue;
        }
        if br || u == RET {
            break;
        }
        if registered
            && u & 0xFFC0_001F == 0xF900_0000
            && (u >> 10) & 0xFFF == 2
            && val.contains_key(&((u >> 5) & 31))
        {
            return val.get(&((u >> 5) & 31)).copied();
        }
        let (a, d) = (add_imm(u), u & 31);
        let mut v: Option<i64> = None;
        if is_adrp(u) {
            if registered && d == 1 {
                break;
            }
            v = Some(adrp_page(u, t as i64));
        } else if let Some(a) = a {
            v = val.get(&a.1).map(|x| x + a.2);
        } else if u & 0xFFE0_FFE0 == 0xAA00_03E0 {
            v = val.get(&((u >> 16) & 31)).copied();
        } else if u & 0x0A00_0000 == 0x0800_0000 {
            if (u >> 22) & 1 == 0 {
                t += 4;
                continue;
            }
            if u & 0x3A00_0000 == 0x2800_0000 {
                val.remove(&((u >> 10) & 31));
            }
        } else if u & 0x1C00_0000 == 0x1400_0000 {
            t += 4;
            continue;
        }
        if d == 1 {
            name = t == j;
        }
        match v {
            None => {
                val.remove(&d);
            }
            Some(v) => {
                val.insert(d, v);
            }
        }
        t += 4;
    }
    None
}

/// `ue_cvar.find(name)`: (offset or None, reason).
fn cvar_find(
    img: &[u8],
    text_end: usize,
    ro_off: usize,
    data_off: usize,
    name: &str,
) -> (Option<i64>, String) {
    let mut objs: BTreeSet<i64> = BTreeSet::new();
    for z in [2usize, 4] {
        let mut pat: Vec<u8> = name
            .bytes()
            .flat_map(|c| std::iter::once(c).chain(std::iter::repeat_n(0, z - 1)))
            .collect();
        pat.extend(std::iter::repeat_n(0, z));
        let mut s = find_from(img, &pat, ro_off, data_off);
        while let Some(at) = s {
            if at % z == 0 {
                let lo = (at & 0xFFF) as i64;
                let page = (at & !0xFFF) as i64;
                let mut j = 0usize;
                while j + 4 <= text_end {
                    let w = word(img, j);
                    if (w >> 22) & 1 == 0
                        && let Some(a) = add_imm(w)
                        && a.2 == lo
                        && (j.saturating_sub(20)..j).step_by(4).any(|i| {
                            let wi = word(img, i);
                            is_adrp(wi) && wi & 31 == a.1 && adrp_page(wi, i as i64) == page
                        })
                        && let Some(o) = registered_at(img, text_end, j)
                    {
                        objs.insert(o);
                    }
                    j += 4;
                }
            }
            s = find_from(img, &pat, at + z, data_off);
        }
    }
    if objs.is_empty() {
        return (None, format!("{name}: not registered"));
    }
    if objs.len() > 1 {
        let l: Vec<String> = objs.iter().map(|o| format!("{o:#x}")).collect();
        return (
            None,
            format!(
                "[RED] {name}: registered at {} objects ({})",
                objs.len(),
                l.join(", ")
            ),
        );
    }
    let o = *objs.iter().next().unwrap_or(&0);
    (Some(o), format!("{name} at {o:#x}"))
}

/// The variables the host may write, in the order of `ue_cvar.NAMES`.
const NAMES: [&str; 5] = [
    "t.MaxFPS",
    "rhi.SyncInterval",
    "nvn.SyncInterval",
    "r.Streaming.PoolSize",
    "Project.FrameRateLimit",
];

/// `ue_cvar.find_all`: ({name: offset}, the reasons of those not found, a `[RED]` one first).
pub(crate) fn cvar_find_all(
    img: &[u8],
    text_end: usize,
    ro_off: usize,
    data_off: usize,
) -> (Vec<(String, i64)>, Vec<String>) {
    let (mut found, mut why) = (Vec::new(), Vec::new());
    for n in NAMES {
        match cvar_find(img, text_end, ro_off, data_off, n) {
            (Some(o), _) => found.push((n.to_string(), o)),
            (None, r) => why.push(r),
        }
    }
    why.sort_by_key(|r| !r.starts_with("[RED] "));
    (found, why)
}

type Stores = ((i64, i64), Vec<(usize, i64)>);

const MIN_READERS: usize = 8;
const SIXTY: i64 = 0x4270_0000;

fn movz_w(w: u32) -> Option<(u32, i64)> {
    (w & 0xFF80_0000 == 0x5280_0000).then(|| {
        (
            w & 31,
            i64::from((w >> 5) & 0xFFFF) << (16 * ((w >> 21) & 1)),
        )
    })
}

/// The general register an instruction writes, roughly.
fn writes(u: u32) -> Option<u32> {
    if u & 0x0A00_0000 == 0x0800_0000 {
        if (u >> 22) & 1 == 0 || (u >> 26) & 1 != 0 {
            return None;
        }
        return Some(u & 31);
    }
    if u & 0x1C00_0000 == 0x1400_0000 {
        return None;
    }
    if u & 0x1F00_0000 == 0x1E00_0000
        && (u >> 29) & 1 == 0
        && ![0x1E26_0000, 0x9E66_0000].contains(&(u & 0xFFFF_FC00))
    {
        return None;
    }
    Some(u & 31)
}

/// `range(start, stop, -4)` over word offsets, as a list of offsets (`start` and the stop in bytes).
fn down(start: usize, stop: usize) -> impl Iterator<Item = usize> {
    let mut k = start as i64;
    std::iter::from_fn(move || {
        let r = (k > stop as i64 && k >= 0).then_some(k as usize);
        k -= 4;
        r
    })
}

/// `fps_target.find`: (pointer offset, field offset, reason) or (None, None, why not).
pub(crate) fn fps_find(
    img: &[u8],
    text_end: usize,
    data_off: usize,
    got: &dyn Fn() -> Result<BTreeMap<i64, i64>, String>,
) -> Result<(Option<i64>, Option<i64>, String), String> {
    let w = |a: usize| word(img, a);
    let mut rel: Option<BTreeMap<i64, i64>> = None;
    let page_of = |i: usize, a: u32| -> Option<i64> {
        for k in down(i.wrapping_sub(4), i.saturating_sub(24)) {
            let u = w(k);
            if is_adrp(u) && u & 31 == a {
                return Some(adrp_page(u, k as i64));
            }
            if writes(u) == Some(a) {
                return None;
            }
        }
        None
    };
    let global_of =
        |i: usize, b: u32, rel: &mut Option<BTreeMap<i64, i64>>| -> Result<Option<i64>, String> {
            for k in down(i.wrapping_sub(4), i.saturating_sub(40)) {
                let u = w(k);
                if u & 31 == b && u & 0xFFC0_0000 == 0xF940_0000 {
                    if (u >> 10) & 0xFFF != 0 {
                        return Ok(None);
                    }
                    let a = (u >> 5) & 31;
                    for k2 in down(k.wrapping_sub(4), k.saturating_sub(32)) {
                        let v = w(k2);
                        if v & 31 != a {
                            continue;
                        }
                        if v & 0xFFC0_0000 == 0x9100_0000 && (v >> 5) & 31 == a {
                            return Ok(page_of(k2, a).map(|p| p + i64::from((v >> 10) & 0xFFF)));
                        }
                        if v & 0xFFC0_0000 == 0xF940_0000 && (v >> 5) & 31 == a {
                            let Some(p) = page_of(k2, a) else {
                                return Ok(None);
                            };
                            if rel.is_none() {
                                *rel = Some(got()?);
                            }
                            return Ok(rel
                                .as_ref()
                                .and_then(|r| r.get(&(p + i64::from((v >> 10) & 0xFFF) * 8)))
                                .copied());
                        }
                        return Ok(None);
                    }
                    return Ok(None);
                }
                if writes(u) == Some(b) {
                    return Ok(None);
                }
            }
            Ok(None)
        };
    let mut readers: Vec<((i64, i64), Vec<usize>)> = Vec::new();
    let mut stores: Vec<Stores> = Vec::new();
    let mut i = 0usize;
    while i < text_end {
        let u = w(i);
        if u & 0xFFC0_0000 == 0xBD40_0000 {
            let (t, b, f) = (u & 31, (u >> 5) & 31, i64::from((u >> 10) & 0xFFF) * 4);
            let conv = (i + 4..text_end.min(i + 24))
                .step_by(4)
                .find(|&k| w(k) & 0xFFFF_FFE0 == (0x5E21_D800 | t << 5));
            if let Some(conv) = conv {
                let c = w(conv) & 31;
                let div = (conv + 4..text_end.min(conv + 20))
                    .step_by(4)
                    .find(|&k| w(k) & 0xFFE0_FC00 == 0x1E20_1800 && (w(k) >> 16) & 31 == c);
                if let Some(div) = div {
                    let m = (w(div) >> 5) & 31;
                    let fm = down(div.wrapping_sub(4), div.saturating_sub(48))
                        .find(|&k| w(k) & 0xFFFF_FC1F == (0x1E27_0000 | m));
                    if let Some(fm) = fm
                        && down(fm.wrapping_sub(4), fm.saturating_sub(32))
                            .any(|k| movz_w(w(k)) == Some(((w(fm) >> 5) & 31, SIXTY)))
                        && let Some(g) = global_of(i, b, &mut rel)?
                        && g >= data_off as i64
                    {
                        match readers.iter_mut().find(|r| r.0 == (g, f)) {
                            Some(r) => r.1.push(i),
                            None => readers.push(((g, f), vec![i])),
                        }
                    }
                }
            }
        } else if u & 0xFFC0_0000 == 0xB900_0000 {
            let (kk, b, f) = (u & 31, (u >> 5) & 31, i64::from((u >> 10) & 0xFFF) * 4);
            let val = down(i.wrapping_sub(4), i.saturating_sub(32))
                .find_map(|k| movz_w(w(k)).filter(|z| z.0 == kk).map(|z| z.1));
            if matches!(val, Some(30 | 60))
                && let Some(g) = global_of(i, b, &mut rel)?
                && g >= data_off as i64
            {
                let v = val.unwrap_or(0);
                match stores.iter_mut().find(|r| r.0 == (g, f)) {
                    Some(r) => r.1.push((i, v)),
                    None => stores.push(((g, f), vec![(i, v)])),
                }
            }
        }
        i += 4;
    }
    let st_of = |k: (i64, i64)| {
        stores
            .iter()
            .find(|s| s.0 == k)
            .map(|s| &s.1[..])
            .unwrap_or(&[])
    };
    let found: Vec<(i64, i64)> = readers
        .iter()
        .filter(|(k, r)| r.len() >= MIN_READERS && st_of(*k).iter().any(|s| s.1 == 30))
        .map(|(k, _)| *k)
        .collect();
    if found.is_empty() {
        let best = readers
            .iter()
            .fold(None, |b: Option<&((i64, i64), Vec<usize>)>, x| match b {
                Some(y) if y.1.len() >= x.1.len() => Some(y),
                _ => Some(x),
            });
        let extra = best.map_or(String::new(), |(k, r)| {
            let vs: Vec<String> = st_of(*k).iter().map(|s| s.1.to_string()).collect();
            format!(
                " (most readers: {} of [{:#x}]+{:#x}, stores [{}])",
                r.len(),
                k.0,
                k.1,
                vs.join(", ")
            )
        });
        return Ok((None, None, format!("no target frame rate{extra}")));
    }
    if found.len() > 1 {
        let l: Vec<String> = found
            .iter()
            .map(|(g, f)| format!("[{g:#x}]+{f:#x}"))
            .collect();
        return Ok((
            None,
            None,
            format!("[RED] target frame rate: {} qualify", l.join(", ")),
        ));
    }
    let (g, f) = found[0];
    let n = readers
        .iter()
        .find(|r| r.0 == (g, f))
        .map_or(0, |r| r.1.len());
    let sets: Vec<String> = st_of((g, f))
        .iter()
        .map(|(a, v)| format!("{v} at {a:#x}"))
        .collect();
    Ok((
        Some(g),
        Some(f),
        format!(
            "target frame rate at [{g:#x}]+{f:#x}: {n} readers of 60 / target, set to {}",
            sets.join(", ")
        ),
    ))
}
