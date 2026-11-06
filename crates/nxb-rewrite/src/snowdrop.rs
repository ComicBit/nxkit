//! Snowdrop's three-pool page allocator (port of `snowdrop_pools.py`): the finder and the cuts that size its pools.

use std::collections::BTreeSet;

use crate::finders::word;

fn find_from(img: &[u8], pat: &[u8], from: usize, to: usize) -> Option<usize> {
    let to = to.min(img.len());
    (from + pat.len() <= to)
        .then(|| img[from..to].windows(pat.len()).position(|w| w == pat))
        .flatten()
        .map(|p| from + p)
}

const G4: u64 = 0x1_0000_0000;
const G12: u64 = 0x3_0000_0000;
const W1_ZERO: u32 = 0x2A1F_03E1;
const X2_ZERO: u32 = 0xAA1F_03E2;
const W5_64K: [u32; 2] = [0x52A0_0025, 0x3210_03E5];
const WINDOW: usize = 48;
/// The default pool sizes in MB (`snowdrop_pools.DEFAULT`).
pub(crate) const SNOWDROP_DEFAULT: [u64; 3] = [2048, 512, 1280];

fn load_imm(w: u32) -> Option<(u32, u64)> {
    if w & 0xFFE0_0000 == 0xD2C0_0000 {
        return Some((w & 31, u64::from((w >> 5) & 0xFFFF) << 32));
    }
    if w & 0xFFC0_03E0 == 0xB240_03E0 {
        let (immr, imms) = ((w >> 16) & 63, (w >> 10) & 63);
        let v: u128 = if imms < 63 {
            (1u128 << (imms + 1)) - 1
        } else {
            (1u128 << 64) - 1
        };
        let r = ((v >> immr) | (v << (64 - immr))) & ((1u128 << 64) - 1);
        return Some((w & 31, r as u64));
    }
    None
}

fn loaded(w: u32, value: u64) -> bool {
    load_imm(w).is_some_and(|x| x.1 == value)
}

fn is_bl(w: u32) -> bool {
    w & 0xFC00_0000 == 0x9400_0000
}

fn bl_target(w: u32, pc: i64) -> i64 {
    let imm = i64::from(w & 0x3FF_FFFF);
    pc + (if imm & (1 << 25) != 0 {
        imm - (1 << 26)
    } else {
        imm
    }) * 4
}

fn str_fields(w: u32, rt: u32) -> Option<(u32, Vec<i64>)> {
    if w & 0xFFC0_0000 == 0xF900_0000 && w & 31 == rt {
        return Some(((w >> 5) & 31, vec![i64::from((w >> 10) & 0xFFF) * 8]));
    }
    if w & 0xFFC0_0000 == 0xA900_0000 && w & 31 == rt && (w >> 10) & 31 == rt {
        let imm = i64::from((w >> 15) & 0x7F);
        let o = (if imm & 0x40 != 0 { imm - 0x80 } else { imm }) * 8;
        return Some(((w >> 5) & 31, vec![o, o + 8]));
    }
    None
}

fn stores_pair_at(w: u32, rn: u32, off: i64) -> bool {
    if w & 0xFFC0_0000 != 0xA900_0000 || (w >> 5) & 31 != rn {
        return false;
    }
    let imm = i64::from((w >> 15) & 0x7F);
    (if imm & 0x40 != 0 { imm - 0x80 } else { imm }) * 8 == off
}

/// The sites of the structure, as word indexes (`check`'s dict).
pub(crate) struct Snow {
    sizes: usize,
    end: usize,
    total: usize,
    pages: usize,
    w1_meta: usize,
    x2_meta: usize,
    w1: usize,
    x2: usize,
    field: i64,
    xs: u32,
}

fn check(w: &[u32], a: usize, k: usize) -> Result<Snow, String> {
    let calls: Vec<usize> = (a..(k + 8).min(w.len())).filter(|&j| is_bl(w[j])).collect();
    if calls.len() < 4 || !(calls[2] < k && k < calls[3]) {
        return Err("not four calls with the page count before the last".into());
    }
    let (b0, b1, b2, b3) = (calls[0], calls[1], calls[2], calls[3]);
    let xa = w[a] & 31;
    let g12: Vec<usize> = (b1..b2).filter(|&j| loaded(w[j], G12)).collect();
    if g12.len() != 2 || w[g12[1]] & 31 != 0 {
        return Err("the second reservation is not 12 GB from x0".into());
    }
    let xb = w[g12[0]] & 31;
    if !(19..=28).contains(&xb) {
        return Err("the 12 GB end is not in a saved register".into());
    }
    let movs: Vec<u32> = (a.saturating_sub(8)..b0)
        .filter(|&j| w[j] & 0xFFFF_FFE0 == 0xAA00_03E0 && (19..=28).contains(&(w[j] & 31)))
        .map(|j| w[j] & 31)
        .collect();
    if movs.len() != 1 {
        return Err(format!(
            "{} moves of the object to a saved register",
            movs.len()
        ));
    }
    let xs = movs[0];
    let mut fields: BTreeSet<i64> = BTreeSet::new();
    for &x in &w[a + 1..b0] {
        if let Some((base, offs)) = str_fields(x, xa)
            && (base == 0 || base == xs)
        {
            fields.extend(offs);
        }
    }
    let f = fields.iter().next().copied().unwrap_or(0);
    if fields != BTreeSet::from([f, f + 8, f + 0x10]) {
        let l: Vec<String> = fields.iter().map(|x| format!("{x:#x}")).collect();
        return Err(format!(
            "the 4 GB goes to fields [{}]",
            l.iter()
                .map(|s| format!("'{s}'"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let targets: BTreeSet<i64> = [
        bl_target(w[b1], b1 as i64 * 4),
        bl_target(w[b2], b2 as i64 * 4),
    ]
    .into_iter()
    .collect();
    if targets.len() != 1 {
        return Err("the two reservations call different functions".into());
    }
    let mut args = Vec::new();
    for (lo, hi) in [(b0, b1), (b1, b2)] {
        let w1: Vec<usize> = (lo..hi).filter(|&j| w[j] == W1_ZERO).collect();
        let x2: Vec<usize> = (lo..hi).filter(|&j| w[j] == X2_ZERO).collect();
        if w1.len() != 1 || x2.len() != 1 || w1[0] > x2[0] {
            return Err("a reservation without mov w1, wzr then mov x2, xzr".into());
        }
        args.extend([w1[0], x2[0]]);
    }
    if !(b1..b2).any(|j| stores_pair_at(w[j], xs, f + 0x28)) {
        return Err("the metadata's base does not go beside the fields".into());
    }
    if !(b2..b3).any(|j| stores_pair_at(w[j], xs, f + 0x18)) {
        return Err("the region's base does not go beside the fields".into());
    }
    if !(b2..b3).any(|j| w[j] & 0xFFE0_FC00 == 0x8B00_0000 && (w[j] >> 16) & 31 == xb) {
        return Err("the region's end is not an add of the 12 GB register".into());
    }
    let r = *targets.iter().next().unwrap_or(&0) / 4;
    if !(0 <= r && (r as usize) < w.len().saturating_sub(12)) {
        return Err("the reservation function is out of the text".into());
    }
    let r = r as usize;
    let end = (r..r + 12).find(|&j| is_bl(w[j]));
    match end {
        Some(e) if (r..e).any(|j| w[j] & 0xFFE0_FFFF == 0xAA00_03E1) => {}
        _ => return Err("the reservation function does not overwrite x1".into()),
    }
    Ok(Snow {
        sizes: a,
        end: g12[0],
        total: g12[1],
        pages: k,
        w1_meta: args[0],
        x2_meta: args[1],
        w1: args[2],
        x2: args[3],
        field: f,
        xs,
    })
}

/// `snowdrop_pools.find(img, text_end)`: `(Some(sites), why)` or `(None, why)`.
pub(crate) fn snowdrop_find(img: &[u8], text_end: usize) -> (Option<Snow>, String) {
    let n = text_end / 4;
    let w: Vec<u32> = (0..n).map(|i| word(img, i * 4)).collect();
    let mut cands = Vec::new();
    for pat in W5_64K {
        let needle = pat.to_le_bytes();
        let mut at = find_from(img, &needle, 0, text_end);
        while let Some(p) = at {
            if p % 4 == 0 {
                let k = p / 4;
                for a in (k.saturating_sub(WINDOW - 1)..k).rev() {
                    if loaded(w[a], G4) {
                        if (a..k).filter(|&j| loaded(w[j], G12)).count() >= 2 {
                            cands.push((a, k));
                        }
                        break;
                    }
                }
            }
            at = find_from(img, &needle, p + 1, text_end);
        }
    }
    if cands.is_empty() {
        return (None, "no Snowdrop three-pool allocator".into());
    }
    let (mut good, mut bad) = (Vec::new(), Vec::new());
    for (a, k) in cands {
        match check(&w, a, k) {
            Ok(s) => good.push(s),
            Err(why) => bad.push((a * 4, why)),
        }
    }
    if good.len() > 1 {
        let l: Vec<String> = good.iter().map(|s| format!("{:#x}", s.sizes * 4)).collect();
        return (
            None,
            format!(
                "[RED] {} Snowdrop three-pool allocators: {}",
                good.len(),
                l.join(", ")
            ),
        );
    }
    let Some(g) = good.pop() else {
        let l: Vec<String> = bad.iter().map(|(o, w)| format!("{o:#x}: {w}")).collect();
        return (
            None,
            format!(
                "[RED] Snowdrop three-pool allocator shape, but {}",
                l.join("; ")
            ),
        );
    };
    let why = format!(
        "Snowdrop three-pool allocator at {:#x}, size fields at +{:#x}",
        g.sizes * 4,
        g.field
    );
    (Some(g), why)
}

fn movz_x(rd: u32, value: u64) -> u32 {
    0xD2A0_0000 | ((value >> 16) as u32) << 5 | rd
}

fn str_x(rt: u32, rn: u32, off: i64) -> u32 {
    0xF900_0000 | ((off / 8) as u32) << 10 | rn << 5 | rt
}

/// `snowdrop_pools.cuts`: (offset, old hex, new hex, why), sorted by offset.
pub(crate) fn snowdrop_cuts(
    img: &[u8],
    s: &Snow,
    sizes: [u64; 3],
) -> Vec<(usize, String, String, String)> {
    let [large, p512, p64] = sizes.map(|x| x << 20);
    let total = large + p512 + p64;
    let xa = word(img, s.sizes * 4) & 31;
    let xb = word(img, s.end * 4) & 31;
    let mut new: Vec<(usize, u32, String)> = vec![
        (
            s.sizes * 4,
            movz_x(xa, p512),
            format!(
                "each range {} MB; the two that differ are set below",
                sizes[1]
            ),
        ),
        (
            s.end * 4,
            movz_x(xb, total),
            format!("combined range end {} MB", total >> 20),
        ),
        (
            s.w1 * 4,
            movz_x(1, large),
            "the large range; the reservation discards incoming x1".into(),
        ),
        (
            s.total * 4,
            movz_x(0, total),
            format!("combined reservation {} MB", total >> 20),
        ),
        (
            s.x2 * 4,
            str_x(1, s.xs, s.field + 0x10),
            format!(
                "the large range field {} MB; the reservation never reads x2",
                sizes[0]
            ),
        ),
        (
            s.pages * 4,
            0x5280_0005 | ((large >> 16) as u32) << 5,
            format!(
                "the large allocator's maximum 64 KB page count, matching its {} MB",
                sizes[0]
            ),
        ),
    ];
    if p64 != p512 {
        new.push((
            s.w1_meta * 4,
            movz_x(1, p64),
            "the 64 KB range; the reservation discards incoming x1".into(),
        ));
        new.push((
            s.x2_meta * 4,
            str_x(1, s.xs, s.field),
            format!(
                "the 64 KB range field {} MB; the reservation never reads x2",
                sizes[2]
            ),
        ));
    }
    new.sort_by_key(|x| x.0);
    new.into_iter()
        .map(|(o, w, why)| {
            let old: String = img[o..o + 4].iter().map(|b| format!("{b:02x}")).collect();
            let nw: String = w.to_le_bytes().iter().map(|b| format!("{b:02x}")).collect();
            (o, old, nw, format!("Snowdrop allocator: {why}"))
        })
        .collect()
}
