//! Engine finder, part four: the port of `ue_ring.py`. Unreal Engine 4's NVN dynamic buffers keep 3 copies. The
//! finder follows the chain from the name of `nvn.NumBufferedFrames` to the constructor's load of the copy count
//! and returns that load with the `mov wM, #4` that replaces it. Every link must be unique, or nothing changes.

use std::collections::BTreeSet;

use crate::finders::{adrp_page, is_adrp, word};

const NAME: &str = "nvn.NumBufferedFrames";
const COPIES: u32 = 4;
/// How far before the load the constructor's start of the object can be.
const BACK: usize = 0x200;

/// What `ue_ring.find` returns when its chain holds: the cut is one word.
pub(crate) struct UeRing {
    /// The load of the copy count.
    pub(crate) off: usize,
    /// Its word.
    pub(crate) old: u32,
    /// The `mov wM, #4` that replaces it.
    pub(crate) new: u32,
    pub(crate) why: String,
}

fn wd(img: &[u8], a: usize) -> Result<u32, String> {
    if a + 4 > img.len() {
        return Err("struct.error".into());
    }
    Ok(word(img, a))
}

fn find_from(img: &[u8], pat: &[u8], from: usize, to: usize) -> Option<usize> {
    let to = to.min(img.len());
    (from + pat.len() <= to)
        .then(|| img[from..to].windows(pat.len()).position(|w| w == pat))
        .flatten()
        .map(|p| from + p)
}

/// The start offsets of a regular expression of four bytes over `img[..end]`, found as `re.finditer` does: a match
/// is not overlapped by the next one, so an unaligned match hides the aligned one that starts inside it.
fn matches4(img: &[u8], end: usize, hit: impl Fn(&[u8]) -> bool) -> Vec<usize> {
    let end = end.min(img.len());
    let mut out = Vec::new();
    let mut i = 0;
    while i + 4 <= end {
        if hit(&img[i..i + 4]) {
            out.push(i);
            i += 4;
        } else {
            i += 1;
        }
    }
    out
}

fn hx(v: usize) -> String {
    format!("{v:#x}")
}

/// The address of the int registered under the name at offset `name` of the image: `adrp` and `add x1` of the
/// name, then `adrp` and `add x2` of the int within 8 instructions. `None` unless exactly one int is found.
fn registered(img: &[u8], text_end: usize, name: usize) -> Result<Option<i64>, String> {
    let add = (0x9100_0021u32 | ((name as u32 & 0xFFF) << 10)).to_le_bytes();
    let mut found = BTreeSet::new();
    let end = text_end.min(img.len());
    let mut from = 0usize;
    while let Some(a) = find_from(img, &add, from, end) {
        from = a + 1;
        if a % 4 != 0 || a < 4 {
            continue;
        }
        let w = wd(img, a - 4)?;
        if !is_adrp(w) || adrp_page(w, a as i64 - 4) != (name & !0xFFF) as i64 {
            continue;
        }
        let mut k = a + 4;
        while k < (a + 36).min(text_end.saturating_sub(4)) {
            let (w, x) = (wd(img, k)?, wd(img, k + 4)?);
            if is_adrp(w) && w & 31 == 2 && x & 0xFFC0_03FF == 0x9100_0042 {
                found.insert(adrp_page(w, k as i64) + i64::from((x >> 10) & 0xFFF));
                break;
            }
            k += 4;
        }
    }
    Ok((found.len() == 1).then(|| found.into_iter().next().unwrap_or(0)))
}

/// The offsets of the lock's rotation over the slots at +8, its count at +0x5c and its index at +0x60.
fn rotations(img: &[u8], text_end: usize) -> Result<Vec<usize>, String> {
    // str w, [x, #0x60]
    let at = matches4(img, text_end, |b| {
        (0x60..=0x63).contains(&b[1]) && b[2] == 0 && b[3] == 0xB9
    });
    let mut out = Vec::new();
    for a in at {
        if a % 4 != 0 || a < 32 || a + 12 > text_end {
            continue;
        }
        let mut w = [0u32; 10];
        for (k, v) in w.iter_mut().enumerate() {
            *v = wd(img, a - 28 + 4 * k)?;
        }
        let [c, cmp, blo, i0, inc, div, msub, st, ad, ld] = w;
        let (b, ri) = ((st >> 5) & 31, st & 31);
        let ok = c & 0xFFFF_FC00 == 0xB940_5C00
            && (c >> 5) & 31 == b
            && cmp == 0x7100_081F | (c & 31) << 5
            && blo & 0xFF00_001F == 0x5400_0003
            && i0 & 0xFFFF_FC00 == 0xB940_6000
            && (i0 >> 5) & 31 == b
            && inc & 0xFFFF_FC00 == 0x1100_0400
            && div & 0xFFE0_FC00 == 0x1AC0_0800
            && msub & 0xFFE0_8000 == 0x1B00_8000
            && msub & 31 == ri
            && ad & 0xFFE0_FC00 == 0x8B20_4C00
            && (ad >> 5) & 31 == b
            && (ad >> 16) & 31 == ri
            && ld & 0xFFC0_0000 == 0xF940_0000
            && (ld >> 5) & 31 == ad & 31
            && (ld >> 10) & 0xFFF == 1;
        if ok {
            out.push(a);
        }
    }
    Ok(out)
}

/// The constructor before the load ends the slots at +0x28: `stur q, [x0, #0x18]` and a store to #0x28 with
/// writeback (`str xzr` or `str q`). With `base`, also `str xzr, [base, #0x5c]!`.
fn layout_holds(img: &[u8], load: usize, base: Option<u32>) -> Result<bool, String> {
    let (mut q18, mut loc, mut kept) = (false, false, false);
    let mut a = load.saturating_sub(BACK);
    while a < load {
        let w = wd(img, a)?;
        q18 |= w & 0xFFFF_FFE0 == 0x3C81_8000;
        loc |= w & 0xFFFF_FC1F == 0xF802_8C1F || w & 0xFFFF_FC00 == 0x3C82_8C00;
        if let Some(b) = base {
            kept |= w == 0xF805_CC1F | b << 5;
        }
        a += 4;
    }
    Ok(q18 && loc && (base.is_none() || kept))
}

/// The constructor's load of the count: the instruction that gives wM the variable's value, on the path of
/// `tbz wU, #1`. Returns `(offset, M)`.
fn loads(img: &[u8], text_end: usize, var: i64) -> Result<Vec<(usize, u32)>, String> {
    let mut got: Option<std::collections::BTreeMap<i64, i64>> = None;
    let mut out = Vec::new();
    // tbz w, #1
    for a in matches4(img, text_end, |b| {
        (0x08..=0x0F).contains(&b[2]) && b[3] == 0x36
    }) {
        if a % 4 != 0 || a + 20 > text_end {
            continue;
        }
        let (adrp, w2, w3, w4) = (
            wd(img, a + 4)?,
            wd(img, a + 8)?,
            wd(img, a + 12)?,
            wd(img, a + 16)?,
        );
        if !is_adrp(adrp) {
            continue;
        }
        let page = adrp_page(adrp, a as i64 + 4);
        let n = adrp & 31;
        let (load, st) = if w2 & 0xFFC0_0000 == 0xB940_0000 && (w2 >> 5) & 31 == n {
            if page + i64::from((w2 >> 10) & 0xFFF) * 4 != var {
                continue;
            }
            (a + 8, w3)
        } else if w2 & 0xFFC0_0000 == 0xF940_0000
            && (w2 >> 5) & 31 == n
            && w2 & 31 == n
            && w3 & 0xFFFF_FC00 == 0xB940_0000
            && (w3 >> 5) & 31 == n
        {
            if got.is_none() {
                got = Some(crate::finders2::got_values(img)?);
            }
            let slot = page + i64::from((w2 >> 10) & 0xFFF) * 8;
            if got.as_ref().and_then(|g| g.get(&slot)) != Some(&var) {
                continue;
            }
            (a + 12, w4)
        } else {
            continue;
        };
        let rm = wd(img, load)? & 31;
        if st & 0xFFC0_001F != 0xB900_0000 | rm {
            continue;
        }
        let (field, base) = (((st >> 10) & 0xFFF) * 4, (st >> 5) & 31);
        if (field == 0x5C && layout_holds(img, load, None)?)
            || (field == 0 && layout_holds(img, load, Some(base))?)
        {
            out.push((load, rm));
        }
    }
    Ok(out)
}

/// `ue_ring.find(img, text_end, ro_off, data_off)`: `Ok(Ok(found))` when the chain holds, else `Ok(Err(why))`
/// (`[RED] ` first when the engine's ring of 3 is there and this chain does not find its count). An `Err` is the
/// place where the script stops with an exception.
pub(crate) fn ue_ring_find(
    img: &[u8],
    text_end: usize,
    ro_off: usize,
    data_off: usize,
) -> Result<Result<UeRing, String>, String> {
    let mut pat: Vec<u8> = NAME.encode_utf16().flat_map(u16::to_le_bytes).collect();
    pat.extend([0, 0]);
    let Some(name) = find_from(img, &pat, ro_off, data_off) else {
        return Ok(Err(format!("no {NAME}")));
    };
    let Some(var) = registered(img, text_end, name)? else {
        return Ok(Err(format!(
            "[RED] Unreal's {NAME} but not one registration of it"
        )));
    };
    if var < 0 || var as usize + 4 > img.len() {
        return Ok(Err(format!(
            "[RED] Unreal's {NAME} at {var:#x}, outside the image"
        )));
    }
    let val = wd(img, var as usize)?;
    if val != 3 {
        return Ok(Err(format!("Unreal's {NAME} is {val}: no ring of 3")));
    }
    let rot = rotations(img, text_end)?;
    if rot.is_empty() {
        return Ok(Err(format!(
            "[RED] Unreal's {NAME} is 3 but no lock rotates over a buffer's slots"
        )));
    }
    let found = loads(img, text_end, var)?;
    if found.len() != 1 {
        let l: Vec<String> = found.iter().map(|(a, _)| hx(*a)).collect();
        return Ok(Err(format!(
            "[RED] Unreal's {NAME} is 3 but {} loads of a dynamic buffer's copy count ({})",
            found.len(),
            l.join(", ")
        )));
    }
    let (off, rm) = found[0];
    Ok(Ok(UeRing {
        off,
        old: wd(img, off)?,
        new: 0x5280_0000 | COPIES << 5 | rm,
        why: format!(
            "Unreal's dynamic buffers: {NAME} at {var:#x} loaded at {off:#x}, {} lock rotations ({}): {COPIES} copies",
            rot.len(),
            hx(rot[0])
        ),
    }))
}
