//! Engine finders, part two (ports of `ue_malloc.py` and `snowdrop_pools.py`), and the relocation table they read
//! (`loader_census.got_values`).

use std::collections::{BTreeMap, BTreeSet};

use crate::finders::{adrp_page, is_adrp, word};

const RET: u32 = 0xD65F_03C0;

fn u64at(img: &[u8], o: usize) -> Result<u64, String> {
    img.get(o..o + 8)
        .map(|s| u64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]))
        .ok_or_else(|| "struct.error".to_string())
}

/// `got_values(img)`: `{address: the address it holds once relocated}` for the relative relocations.
pub(crate) fn got_values(img: &[u8]) -> Result<BTreeMap<i64, i64>, String> {
    let mut out = BTreeMap::new();
    let mod0 = word(img, 4) as usize;
    if img.get(mod0..mod0 + 4) != Some(b"MOD0") {
        return Ok(out);
    }
    let dyn_ = (mod0 as i64 + i64::from(word(img, mod0 + 4) as i32)) as usize;
    let mut tags: BTreeMap<u64, u64> = BTreeMap::new();
    for k in 0..4096 {
        let (t, v) = (u64at(img, dyn_ + k * 16)?, u64at(img, dyn_ + k * 16 + 8)?);
        if t == 0 {
            break;
        }
        tags.entry(t).or_insert(v);
    }
    let tag = |t: u64| tags.get(&t).copied().unwrap_or(0);
    for k in 0..(tag(8) / 24) as usize {
        let base = tag(7) as usize + k * 24;
        let (off, info, add) = (
            u64at(img, base)?,
            u64at(img, base + 8)?,
            u64at(img, base + 16)? as i64,
        );
        let ty = info & 0xFFFF_FFFF;
        if ty == 1027 {
            out.insert(off as i64, add);
        } else if (ty == 257 || ty == 1025) && tags.contains_key(&6) {
            let sym = tag(6) as usize + (info >> 32) as usize * 24 + 6;
            let shndx = img
                .get(sym..sym + 2)
                .map_or(0, |s| u16::from_le_bytes([s[0], s[1]]));
            let value = u64at(img, sym + 2)? as i64;
            if shndx != 0 {
                out.insert(off as i64, value + add);
            }
        }
    }
    let mut at = 0u64;
    for k in 0..(tag(35) / 8) as usize {
        let e = u64at(img, tag(36) as usize + k * 8)?;
        if e & 1 == 0 {
            at = e;
            out.insert(at as i64, u64at(img, at as usize)? as i64);
            at += 8;
        } else {
            for b in 0..63 {
                if (e >> (b + 1)) & 1 == 1 {
                    let a = at + b * 8;
                    out.insert(a as i64, u64at(img, a as usize)? as i64);
                }
            }
            at += 63 * 8;
        }
    }
    Ok(out)
}

fn find_from(img: &[u8], pat: &[u8], from: usize, to: usize) -> Option<usize> {
    let to = to.min(img.len());
    (from + pat.len() <= to)
        .then(|| img[from..to].windows(pat.len()).position(|w| w == pat))
        .flatten()
        .map(|p| from + p)
}

/// What `ue_malloc.find` returns when its chain holds.
pub(crate) struct UeMalloc {
    pub(crate) call: usize,
    pub(crate) vt: i64,
    pub(crate) slot: i64,
    pub(crate) why: String,
}

fn layout(slot: i64) -> Option<&'static str> {
    match slot {
        19 => Some("Unreal Engine 4.26 / 4.27"),
        22 => Some("Unreal Engine 5"),
        26 => Some("Unreal Engine 5.6"),
        _ => None,
    }
}

/// `ue_malloc.find(img, text_end, ro_off, data_off)`: `Ok(Ok(found))`, or `Ok(Err(why))` (`[RED] ` first when the
/// allocator is there and cannot be replaced).
pub(crate) fn ue_malloc_find(
    img: &[u8],
    text_end: usize,
    ro_off: usize,
    data_off: usize,
) -> Result<Result<UeMalloc, String>, String> {
    let w = |a: usize| word(img, a);
    let name: Vec<u8> = "Binned3"
        .bytes()
        .flat_map(|c| [c, 0])
        .chain([0, 0])
        .collect();
    let mut names = Vec::new();
    let mut s = find_from(img, &name, 0, data_off);
    while let Some(at) = s {
        if at >= ro_off && at % 2 == 0 && (at == ro_off || img[at - 2..at] == [0, 0]) {
            names.push(at);
        }
        s = find_from(img, &name, at + 1, data_off);
    }
    if names.is_empty() {
        return Ok(Err("no FMallocBinned3 name".into()));
    }
    if names.len() > 1 {
        let l: Vec<String> = names.iter().map(|x| format!("{x:#x}")).collect();
        return Ok(Err(format!(
            "[RED] {} FMallocBinned3 names: {}",
            names.len(),
            l.join(", ")
        )));
    }
    let s = names[0];
    let mut tail = (0x9100_0000u32 | ((s as u32 & 0xFFF) << 10))
        .to_le_bytes()
        .to_vec();
    tail.extend(RET.to_le_bytes());
    let mut getters = Vec::new();
    let mut a = find_from(img, &tail, 0, text_end);
    while let Some(at) = a {
        if at % 4 == 0 && at >= 4 {
            let p = w(at - 4);
            if is_adrp(p) && p & 31 == 0 && adrp_page(p, at as i64 - 4) == (s & !0xFFF) as i64 {
                getters.push(at - 4);
            }
        }
        a = find_from(img, &tail, at + 1, text_end);
    }
    if getters.len() != 1 {
        return Ok(Err(format!(
            "[RED] FMallocBinned3 name at {s:#x}: {} getters",
            getters.len()
        )));
    }
    let g = getters[0];
    let rel = got_values(img)?;
    let value = |a: i64| -> Result<i64, String> {
        match rel.get(&a) {
            Some(v) => Ok(*v),
            None => Ok(u64at(
                img,
                usize::try_from(a).map_err(|_| "struct.error".to_string())?,
            )? as i64),
        }
    };
    let mut slots: Vec<i64> = rel
        .iter()
        .filter(|(o, v)| **v == g as i64 && **o >= data_off as i64)
        .map(|(o, _)| *o)
        .collect();
    let gb = (g as u64).to_le_bytes();
    let mut a = find_from(img, &gb, data_off, usize::MAX);
    while let Some(at) = a {
        if at % 8 == 0 && !slots.contains(&(at as i64)) {
            slots.push(at as i64);
        }
        a = find_from(img, &gb, at + 8, usize::MAX);
    }
    if slots.len() != 1 {
        return Ok(Err(format!(
            "[RED] FMallocBinned3 getter at {g:#x}: {} vtable slots",
            slots.len()
        )));
    }
    let is_code = |v: i64| 0 < v && v < text_end as i64 && v % 4 == 0;
    let mut vt = slots[0];
    while vt - 8 >= data_off as i64 && is_code(value(vt - 8)?) {
        vt -= 8;
    }
    let slot = (slots[0] - vt) / 8;
    if (value(vt - 16)?, value(vt - 8)?) != (0, 0) {
        return Ok(Err(format!(
            "[RED] FMallocBinned3 vtable at {vt:#x}: no offset to top and RTTI words of 0 before it"
        )));
    }
    let Some(lay) = layout(slot) else {
        return Ok(Err(format!(
            "[RED] FMallocBinned3 vtable at {vt:#x}: the name getter is slot {slot}, a slot layout the host does not know"
        )));
    };
    let dtors: BTreeSet<i64> = [value(vt)?, value(vt + 8)?].into_iter().collect();
    // direct calls and jumps by target
    let (mut bl, mut b): (BTreeMap<i64, Vec<i64>>, BTreeMap<i64, Vec<i64>>) =
        (BTreeMap::new(), BTreeMap::new());
    let mut a = 0usize;
    while a + 4 <= text_end {
        let x = w(a);
        if matches!(x >> 24, 0x94..=0x97 | 0x14..=0x17) {
            let t = a as i64 + ((i64::from(x & 0x3FF_FFFF) ^ 0x200_0000) - 0x200_0000) * 4;
            (if x >> 26 == 0x25 { &mut bl } else { &mut b })
                .entry(t)
                .or_default()
                .push(a as i64);
        }
        a += 4;
    }
    let lo = (vt & 0xFFF) as u32;
    let mut ctors: BTreeSet<i64> = BTreeSet::new();
    let mut a = 0usize;
    while a + 4 <= text_end {
        let x = w(a);
        if (x >> 16) & 0xFF == lo >> 6 && x >> 24 == 0x91 && (x >> 10) & 0xFFF == lo {
            let (n, d) = ((x >> 5) & 31, x & 31);
            let adrp_ok = (a.saturating_sub(32)..a).step_by(4).any(|p| {
                let y = w(p);
                is_adrp(y) && y & 31 == n && adrp_page(y, p as i64) == vt & !0xFFF
            });
            let str_ok = (a + 4..text_end.min(a + 48))
                .step_by(4)
                .any(|q| w(q) & 0xFFFF_FC1F == (0xF900_0000 | d));
            if adrp_ok && str_ok {
                let start = bl
                    .keys()
                    .chain(b.keys())
                    .chain(dtors.iter())
                    .copied()
                    .filter(|t| *t <= a as i64)
                    .max();
                let bad = match start {
                    None => true,
                    Some(st) => (st.max(0) as usize..a).step_by(4).any(|q| w(q) == RET),
                };
                if bad {
                    return Ok(Err(format!(
                        "[RED] FMallocBinned3 vtable at {vt:#x}: no function start for its store at {a:#x}"
                    )));
                }
                if let Some(st) = start
                    && !dtors.contains(&st)
                {
                    ctors.insert(st);
                }
            }
        }
        a += 4;
    }
    if ctors.len() != 1 {
        let l: Vec<String> = ctors.iter().map(|x| format!("{x:#x}")).collect();
        return Ok(Err(format!(
            "[RED] FMallocBinned3 vtable at {vt:#x}: {} constructors ({})",
            ctors.len(),
            l.join(", ")
        )));
    }
    let c = *ctors.iter().next().unwrap_or(&0);
    let (nbl, nb) = (
        bl.get(&c).map_or(0, Vec::len),
        b.get(&c).map_or(0, Vec::len),
    );
    if nbl != 1 || nb != 0 {
        return Ok(Err(format!(
            "[RED] FMallocBinned3 constructor at {c:#x}: {nbl} calls and {nb} jumps, not one call"
        )));
    }
    let call = bl[&c][0];
    Ok(Ok(UeMalloc {
        call: call as usize,
        vt,
        slot,
        why: format!(
            "FMallocBinned3 ({lay}): its constructor at {c:#x}, called at {call:#x}; its vtable {vt:#x}, the name getter slot {slot}"
        ),
    }))
}

/// Whether the image names `SwitchVirtualAddressSpaceSize` in its read-only data (`ue_region.find`'s first test).
pub(crate) fn has_region_option(img: &[u8], ro_off: usize, data_off: usize) -> bool {
    find_from(img, b"SwitchVirtualAddressSpaceSize", ro_off, data_off).is_some()
}
