//! Facts of a game's code modules and RomFS (`dm/games/profile_guess.py` `facts_of`, `nso_syms.py`): dynamic symbols,
//! sizes, SDK strings, engine markers, RomFS counts. Everything the profile rules read, nothing the rules decide.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;

/// A `collections.Counter` that remembers the order of first insertion (`most_common` breaks ties by it).
#[derive(Clone, Debug, Default)]
pub struct Counter {
    items: Vec<(String, u64)>,
    index: BTreeMap<String, usize>,
}

impl Counter {
    /// `c[k] += n`.
    pub fn add(&mut self, k: &str, n: u64) {
        match self.index.get(k) {
            Some(&i) => self.items[i].1 += n,
            None => {
                self.index.insert(k.to_string(), self.items.len());
                self.items.push((k.to_string(), n));
            }
        }
    }
    /// `c[k]` (0 when absent).
    pub fn get(&self, k: &str) -> u64 {
        self.index.get(k).map_or(0, |&i| self.items[i].1)
    }
    /// `c.most_common(n)`: by count, descending, ties in insertion order.
    pub fn most_common(&self, n: usize) -> Vec<(&str, u64)> {
        let mut v: Vec<(&str, u64)> = self.items.iter().map(|(k, c)| (k.as_str(), *c)).collect();
        v.sort_by_key(|a| std::cmp::Reverse(a.1));
        v.truncate(n);
        v
    }
    /// The keys with their counts in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, u64)> {
        self.items.iter().map(|(k, c)| (k.as_str(), *c))
    }
}

/// One dynamic symbol: name, defined, binding.
pub type Sym = (String, bool, u8);

/// `nso_syms.dynsyms`.
pub fn dynsyms(img: &[u8]) -> Result<Vec<Sym>, String> {
    let u = |o: usize, n: usize| -> Result<u64, String> {
        let s = o
            .checked_add(n)
            .and_then(|e| img.get(o..e))
            .ok_or("dynamic table outside the image")?;
        Ok(s.iter().rev().fold(0u64, |a, &x| a << 8 | u64::from(x)))
    };
    let mod0 = u(4, 4)? as usize;
    if img.get(mod0..mod0 + 4) != Some(b"MOD0") {
        return Err("no MOD0".into());
    }
    let dyn_ = (mod0 as i64 + i64::from(u(mod0 + 4, 4)? as u32 as i32)) as usize;
    let mut tags = BTreeMap::new();
    for i in 0..4096 {
        let (t, v) = (u(dyn_ + i * 16, 8)? as i64, u(dyn_ + i * 16 + 8, 8)?);
        if t == 0 {
            break;
        }
        tags.entry(t).or_insert(v);
    }
    let tag = |t: i64| tags.get(&t).map(|&v| v as usize).ok_or("no symbol table");
    let (symtab, strtab) = (tag(6)?, tag(5)?);
    let n = match tags.get(&4) {
        Some(&h) => u(h as usize + 4, 4)? as usize,
        None => strtab.wrapping_sub(symtab) / 24,
    };
    let mut out = Vec::new();
    for i in 1..n {
        let o = symtab + i * 24;
        let (name, info, shndx) = (u(o, 4)? as usize, u(o + 4, 1)? as u8, u(o + 6, 2)?);
        let s = img
            .get(strtab + name..)
            .ok_or("symbol name outside the image")?;
        let end = s.iter().position(|&c| c == 0).ok_or("unterminated name")?;
        if end > 0 {
            out.push((
                s[..end].iter().map(|&c| c as char).collect(),
                shndx != 0,
                info >> 4,
            ));
        }
    }
    Ok(out)
}

/// `nso_syms.defined`: `{name: (offset, size)}` of the defined dynamic symbols, empty without a table it can read.
pub fn defined(img: &[u8]) -> Vec<(String, (u64, u64))> {
    let mut out: Vec<(String, (u64, u64))> = Vec::new();
    let Ok(_) = dynsyms(img) else { return out };
    // the table again with values and sizes
    let u = |o: usize, n: usize| -> u64 {
        img.get(o..o + n).map_or(0, |s| {
            s.iter().rev().fold(0u64, |a, &x| a << 8 | u64::from(x))
        })
    };
    let mod0 = u(4, 4) as usize;
    let dyn_ = (mod0 as i64 + i64::from(u(mod0 + 4, 4) as u32 as i32)) as usize;
    let mut tags = BTreeMap::new();
    for i in 0..4096 {
        let (t, v) = (u(dyn_ + i * 16, 8) as i64, u(dyn_ + i * 16 + 8, 8));
        if t == 0 {
            break;
        }
        tags.entry(t).or_insert(v);
    }
    let (symtab, strtab) = (tags[&6] as usize, tags[&5] as usize);
    let n = match tags.get(&4) {
        Some(&h) => u(h as usize + 4, 4) as usize,
        None => strtab.wrapping_sub(symtab) / 24,
    };
    for i in 1..n {
        let o = symtab + i * 24;
        if u(o + 6, 2) != 0 {
            let s = &img[strtab + u(o, 4) as usize..];
            let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
            let name: String = s[..end].iter().map(|&c| c as char).collect();
            let v = (u(o + 8, 8), u(o + 16, 8));
            match out.iter_mut().find(|e| e.0 == name) {
                Some(e) => e.1 = v,
                None => out.push((name, v)),
            }
        }
    }
    out
}

/// `nso_syms.area`: the namespace most of a module's exports are in.
pub fn area(s: &str) -> String {
    for p in ["_ZNK2nn", "_ZN2nn"] {
        if let Some(r) = s.strip_prefix(p) {
            let d: String = r.chars().take_while(char::is_ascii_digit).collect();
            let rest = &r[d.len()..];
            if !d.is_empty() && !rest.is_empty() {
                // `(\d+)(\w+)` with backtracking: the digits are all of the run
                let n: usize = d.parse().unwrap_or(0);
                let w: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if !w.is_empty() {
                    return format!("nn::{}", w.chars().take(n).collect::<String>());
                }
            }
        }
    }
    if s.starts_with("nvn") {
        "nvn".into()
    } else if s.starts_with("_Z") {
        "other C++".into()
    } else {
        "C / libc".into()
    }
}

/// `nso_syms.host_hooks(host, forced)`: the names the host resolves imports to. `dm` is the engine's `dm` folder.
pub fn host_hooks(dm: &Path, forced: bool) -> HashSet<String> {
    let mut hooks = HashSet::new();
    let list = |d: &Path, ext: &str| -> Vec<std::path::PathBuf> {
        let mut v: Vec<_> = std::fs::read_dir(d)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == ext) && p.is_file())
            .collect();
        v.sort();
        v
    };
    for d in [dm.join("host"), dm.join("gfx"), dm.join("gfx/nvn")] {
        for f in list(&d, "c") {
            let t = String::from_utf8_lossy(&std::fs::read(&f).unwrap_or_default()).into_owned();
            call_names(&t, "HLE", &mut hooks);
            if forced {
                call_names(&t, "HLE_FORCE", &mut hooks);
                call_names(&t, "HLE_FORCE_BIG", &mut hooks);
            }
            if f.file_name().is_some_and(|n| n == "hle_table.c") {
                call_words(&t, "L", &mut hooks);
            }
        }
    }
    for f in list(&dm.join("gfx/vk"), "m") {
        let t = String::from_utf8_lossy(&std::fs::read(&f).unwrap_or_default()).into_owned();
        for l in t.split('\n') {
            for p in ["VKB(", "VK("] {
                if let Some(r) = l.strip_prefix(p) {
                    let w: String = r
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    if w.starts_with("vk") && r[w.len()..].starts_with(',') {
                        hooks.insert(w);
                    }
                }
            }
        }
    }
    hooks
}

fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// `\bNAME\(\s*"([^"]+)"` over `t`.
fn call_names(t: &str, name: &str, out: &mut HashSet<String>) {
    let b = t.as_bytes();
    let pat = format!("{name}(");
    let mut from = 0;
    while let Some(i) = t[from..].find(&pat) {
        let at = from + i;
        from = at + 1;
        if at > 0 && is_word(b[at - 1]) {
            continue;
        }
        let mut j = at + pat.len();
        while j < b.len() && b[j].is_ascii_whitespace() {
            j += 1;
        }
        if b.get(j) != Some(&b'"') {
            continue;
        }
        if let Some(e) = t[j + 1..].find('"')
            && e > 0
        {
            out.insert(t[j + 1..j + 1 + e].to_string());
        }
    }
}

/// `\bNAME\((\w+)\)`.
fn call_words(t: &str, name: &str, out: &mut HashSet<String>) {
    let b = t.as_bytes();
    let pat = format!("{name}(");
    let mut from = 0;
    while let Some(i) = t[from..].find(&pat) {
        let at = from + i;
        from = at + 1;
        if at > 0 && is_word(b[at - 1]) {
            continue;
        }
        let s = at + pat.len();
        let mut e = s;
        while e < b.len() && is_word(b[e]) {
            e += 1;
        }
        if e > s && b.get(e) == Some(&b')') {
            out.insert(t[s..e].to_string());
        }
    }
}

// ---- byte scans (the regular expressions of facts_of) ----

/// Non-overlapping count of `pat` (`bytes.count`).
pub fn count(img: &[u8], pat: &[u8]) -> usize {
    let (mut n, mut i) = (0, 0);
    while let Some(p) = find(img, pat, i) {
        n += 1;
        i = p + pat.len();
    }
    n
}

/// `bytes.find(pat, from)`.
pub fn find(img: &[u8], pat: &[u8], from: usize) -> Option<usize> {
    if pat.is_empty() || from + pat.len() > img.len() {
        return None;
    }
    let first = pat[0];
    let mut i = from;
    let last = img.len() - pat.len();
    while i <= last {
        i += img[i..=last].iter().position(|&c| c == first)?;
        if &img[i..i + pat.len()] == pat {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn run(img: &[u8], i: usize, f: impl Fn(u8) -> bool) -> usize {
    let mut j = i;
    while j < img.len() && f(img[j]) {
        j += 1;
    }
    j
}

fn dig_us(c: u8) -> bool {
    c.is_ascii_digit() || c == b'_'
}

/// `NintendoSDK_(?!nnSdk)\w+-[0-9_]+`: the matches without the digits (`rsplit('-', 1)[0]`).
pub fn sdk_components(img: &[u8]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let pre = b"NintendoSDK_";
    let mut i = 0;
    while let Some(p) = find(img, pre, i) {
        let s = p + pre.len();
        i = p + 1;
        if img[s..].starts_with(b"nnSdk") {
            continue;
        }
        let w = run(img, s, is_word);
        if w > s && img.get(w) == Some(&b'-') {
            let d = run(img, w + 1, dig_us);
            if d > w + 1 {
                out.insert(String::from_utf8_lossy(&img[p..w]).into_owned());
                i = d;
            }
        }
    }
    out
}

/// `SDK MW\+Nintendo\+([\w]+-[0-9_]+)`: the groups.
pub fn mw_components(img: &[u8]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let pre = b"SDK MW+Nintendo+";
    let mut i = 0;
    while let Some(p) = find(img, pre, i) {
        let s = p + pre.len();
        i = p + 1;
        let w = run(img, s, is_word);
        if w > s && img.get(w) == Some(&b'-') {
            let d = run(img, w + 1, dig_us);
            if d > w + 1 {
                out.insert(String::from_utf8_lossy(&img[s..d]).into_owned());
                i = d;
            }
        }
    }
    out
}

/// `(?i)NintendoSDK_nnSdk-(?:[a-z]+_)?([0-9_]+)`: the groups.
pub fn sdk_versions(img: &[u8]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let pre = b"nintendosdk_nnsdk-";
    let low: Vec<u8> = img.iter().map(u8::to_ascii_lowercase).collect();
    let mut i = 0;
    while let Some(p) = find(&low, pre, i) {
        let s = p + pre.len();
        i = p + 1;
        let l = run(img, s, |c| c.is_ascii_alphabetic());
        let mut m = None;
        if l > s && img.get(l) == Some(&b'_') {
            let d = run(img, l + 1, dig_us);
            if d > l + 1 {
                m = Some((l + 1, d));
            }
        }
        if m.is_none() {
            let d = run(img, s, dig_us);
            if d > s {
                m = Some((s, d));
            }
        }
        if let Some((a, d)) = m {
            out.insert(String::from_utf8_lossy(&img[a..d]).into_owned());
            i = d;
        }
    }
    out
}

/// `20[12][0-9]\.[0-9]+\.[0-9]+f[0-9]`: the matches.
pub fn unity_versions(img: &[u8]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut i = 0;
    while let Some(p) = find(img, b"20", i) {
        i = p + 1;
        let g = |k: usize| img.get(p + k).copied().unwrap_or(0);
        if !(g(2) == b'1' || g(2) == b'2') || !g(3).is_ascii_digit() || g(4) != b'.' {
            continue;
        }
        let a = run(img, p + 5, |c| c.is_ascii_digit());
        if a == p + 5 || img.get(a) != Some(&b'.') {
            continue;
        }
        let b = run(img, a + 1, |c| c.is_ascii_digit());
        if b == a + 1
            || img.get(b) != Some(&b'f')
            || !img.get(b + 1).is_some_and(u8::is_ascii_digit)
        {
            continue;
        }
        out.insert(String::from_utf8_lossy(&img[p..b + 2]).into_owned());
        i = b + 2;
    }
    out
}

/// `GL_SRC`: `\0#version [0-9]`, counted.
pub fn gl_src_count(img: &[u8]) -> usize {
    let pat = b"\0#version ";
    let (mut n, mut i) = (0, 0);
    while let Some(p) = find(img, pat, i) {
        i = p + 1;
        if img.get(p + pat.len()).is_some_and(u8::is_ascii_digit) {
            n += 1;
            i = p + pat.len() + 1;
        }
    }
    n
}

/// `glsl_sources`: the `#version ` strings that hold a `void main(`.
pub fn glsl_sources(img: &[u8]) -> usize {
    let pat = b"#version ";
    let (mut n, mut i) = (0, 0);
    while let Some(p) = find(img, pat, i) {
        let e = run(img, p, |c| c != 0);
        if has_void_main(&img[p..e]) {
            n += 1;
        }
        i = e.max(p + 1);
    }
    n
}

fn has_void_main(s: &[u8]) -> bool {
    let ws = |c: u8| matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c);
    let mut i = 0;
    while let Some(p) = find(s, b"void", i) {
        i = p + 1;
        if p > 0 && is_word(s[p - 1]) {
            continue;
        }
        let a = run(s, p + 4, ws);
        if a == p + 4 || !s[a..].starts_with(b"main") {
            continue;
        }
        let b = run(s, a + 4, ws);
        if s.get(b) == Some(&b'(') {
            return true;
        }
    }
    false
}

/// `glsl_text`: a file's first bytes can be GLSL source (text, no control byte).
pub fn glsl_text(head: &[u8]) -> bool {
    !head.is_empty()
        && !head
            .iter()
            .any(|&c| matches!(c, 0..=8 | 0x0b | 0x0e..=0x1f | 0x7f))
}
