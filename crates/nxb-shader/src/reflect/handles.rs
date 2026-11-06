//! Texture and sampler handle names (ports of `decode_tex`, `const_sampler`, `decode_ubo_tex`, `c2_array`).

use super::{C2_IMG, C2_IMG_SLOTS, C2_ROW, C2_SAMP, C2_SEPTEX, C2_TEX, SEPTEX_SLOT0, is_word};

/// `[a-z]+_` then `kinds` (a set of first letters) then `\w*?_` then the slot part.`cb_only` drops `tcb`.
/// `full` requires the hex to end the name. → (slots or None for `tcb`, handle).
fn handle_name(
    name: &str,
    kinds: &[char],
    cb_only: bool,
    full: bool,
) -> Option<(Option<i64>, i64)> {
    let low = name.bytes().take_while(u8::is_ascii_lowercase).count();
    if low == 0 {
        return None;
    }
    let r = name[low..].strip_prefix('_')?;
    let c = r.chars().next()?;
    if !kinds.contains(&c) {
        return None;
    }
    let r = &r[1..];
    // \w*? lazy: try every prefix of the word run.
    let run = r
        .char_indices()
        .find(|(_, c)| !is_word(*c))
        .map_or(r.len(), |(k, _)| k);
    for j in 0..=run {
        let t = &r[j..];
        let attempt = (|| {
            let t = t.strip_prefix('_')?;
            let (slots, t) = if let Some(x) = t.strip_prefix("cb") {
                let d = x.bytes().take_while(u8::is_ascii_digit).count();
                if d == 0 {
                    return None;
                }
                (Some(x[..d].parse::<i64>().ok()?), &x[d..])
            } else if !cb_only {
                (None, t.strip_prefix("tcb")?)
            } else {
                return None;
            };
            let t = t.strip_prefix('_')?;
            let h = t
                .bytes()
                .take_while(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(b))
                .count();
            if h == 0 || (full && h != t.len()) {
                return None;
            }
            Some((slots, i64::from_str_radix(&t[..h], 16).ok()?))
        })();
        if attempt.is_some() {
            return attempt;
        }
    }
    None
}

fn slot(word: i64, base: i64, cb: i64) -> Option<i64> {
    (cb == 2 && word * 4 >= base && (word * 4 - base) % C2_ROW == 0)
        .then(|| (word * 4 - base) / C2_ROW)
}

pub(super) fn is_image(name: &str) -> bool {
    let low = name.bytes().take_while(u8::is_ascii_lowercase).count();
    low > 0 && name[low..].starts_with("_i")
}

/// `decode_tex(name)` → (texture or image slot, sampler slot, handle type).
pub(super) fn decode_tex(name: &str) -> Option<(Option<i64>, Option<i64>, i64)> {
    let (slots, h) = handle_name(name, &['t', 'i'], false, false)?;
    let slots = slots.unwrap_or(-1);
    let tb = if slots >= 0 { slots & 0xffff } else { 2 };
    let sb = if slots >= 0 && (slots >> 16) != 0 {
        (slots >> 16) - 1
    } else {
        tb
    };
    let (o0, o1, ty) = (h & 0x3fff, (h >> 14) & 0x3fff, h >> 28);
    let mut words = vec![o0];
    if ty != 0 {
        words.push(o1);
    }
    if is_image(name) {
        let s = words.iter().find_map(|x| {
            (C2_IMG <= x * 4 && x * 4 < C2_IMG + C2_ROW * C2_IMG_SLOTS)
                .then(|| slot(*x, C2_IMG, tb))
                .flatten()
        });
        return Some((s, None, ty));
    }
    let tw: Vec<i64> = if ty == 3 { vec![o0] } else { words.clone() };
    let mut t = tw
        .iter()
        .find_map(|x| slot(*x, C2_TEX, tb).filter(|_| x * 4 < C2_SEPTEX));
    if t.is_none() {
        t = tw.iter().find_map(|x| {
            slot(*x, C2_SEPTEX, tb)
                .filter(|_| x * 4 < C2_SAMP)
                .map(|s| SEPTEX_SLOT0 + s)
        });
    }
    let s = if ty != 3 {
        words
            .iter()
            .find_map(|x| (x * 4 >= C2_SAMP).then(|| slot(*x, C2_SAMP, sb)).flatten())
    } else {
        None
    };
    Some((t, s, ty))
}

/// `const_sampler(name)`.
pub(super) fn const_sampler(name: &str) -> Option<i64> {
    let (_, h) = handle_name(name, &['t'], false, false)?;
    (h >> 28 == 3).then_some((h >> 14) & 0x3fff)
}

/// ((slot, offset), sampler: Ok((slot, offset)) or Err(constant id)).
pub(super) type UboTex = ((i64, i64), Result<(i64, i64), i64>);

/// `decode_ubo_tex(name)` → ((slot, offset), sampler: Ok((slot, offset)) or Err(constant id)).
pub(super) fn decode_ubo_tex(name: &str) -> Option<UboTex> {
    let (slots, h) = handle_name(name, &['t'], true, false)?;
    let slots = slots?;
    let tb = slots & 0xffff;
    let sb = if slots >> 16 != 0 {
        (slots >> 16) - 1
    } else {
        tb
    };
    let (o0, o1, ty) = (h & 0x3fff, (h >> 14) & 0x3fff, h >> 28);
    if !(3..=16).contains(&tb) || ![0, 1, 3].contains(&ty) || (ty == 1 && !(3..=16).contains(&sb)) {
        return None;
    }
    let s = match ty {
        3 => Err(o1),
        1 => Ok((sb - 3, o1 * 4)),
        _ => Ok((tb - 3, o0 * 4)),
    };
    Some(((tb - 3, o0 * 4), s))
}

/// `decode_ubo_img(name)`: an image whose handle is in the game's own uniform buffer
/// (`<stage>_i<..>_cb<n>_<hex word>`) → (uniform slot, byte offset). `re.match(r'[a-z]+_i\w*?_cb(\d+)_([0-9A-F]+)')`.
pub(super) fn decode_ubo_img(name: &str) -> Option<(i64, i64)> {
    let low = name.bytes().take_while(u8::is_ascii_lowercase).count();
    if low == 0 || !name[low..].starts_with("_i") {
        return None;
    }
    let from = low + 2;
    // the lazy `\w*?` tries every `_cb` start from the left, as long as only word characters lie between
    let mut k = from - 1;
    loop {
        let rest = name.get(k + 1..)?;
        let at = k + 1 + rest.find("_cb")?;
        if !name[from..at].chars().all(is_word) {
            return None;
        }
        let r = &name[at + 3..];
        let nd = r.bytes().take_while(u8::is_ascii_digit).count();
        if nd > 0 && r[nd..].starts_with('_') {
            let h = &r[nd + 1..];
            let nh = h
                .bytes()
                .take_while(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(b))
                .count();
            if nh > 0 {
                let n: i64 = r[..nd].parse().ok()?;
                let w = i64::from_str_radix(&h[..nh], 16).ok()?;
                return (3..=16).contains(&n).then_some((n - 3, w * 4));
            }
        }
        k = at;
    }
}

/// `c2_array(name, ty)` → (offset, sampler word offset).
pub(super) fn c2_array(name: &str, ty: &str) -> Option<(i64, Option<i64>)> {
    let n = name.strip_suffix("Smplr").unwrap_or(name);
    let (slots, h) = handle_name(n, &['t'], true, true)?;
    let slots = slots?;
    if !ty.starts_with("array<") {
        return None;
    }
    if h >> 28 == 0 && slots == 2 {
        return Some(((h & 0x3fff) * 4, None));
    }
    if h >> 28 == 1 && slots & 0xffff == 2 && (slots >> 16) - 1 == 2 {
        return Some(((h & 0x3fff) * 4, Some(((h >> 14) & 0x3fff) * 4)));
    }
    None
}
