//! `dmsh_wide.wide_loads`: storage words the translator split, read whole again.

use std::collections::{BTreeMap, BTreeSet};

/// `_IDENT`: `_[0-9A-Za-z_]+` at the start → its end.
fn ident(s: &str) -> Option<usize> {
    let r = s.strip_prefix('_')?;
    let n = r
        .bytes()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == b'_')
        .count();
    (n > 0).then_some(n + 1)
}

/// `_BUF`: `[a-z]p_s\d+` at the start → its end.
fn buf(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    if b.len() < 5 || !b[0].is_ascii_lowercase() || &b[1..4] != b"p_s" {
        return None;
    }
    let d = b[4..].iter().take_while(|c| c.is_ascii_digit()).count();
    (d > 0).then_some(4 + d)
}

/// `_ADD.match`: `^\s*int (ID) = (ID) \+ (-?\d+);\s*$`.
fn add(ln: &str) -> Option<(&str, &str, i64)> {
    let t = ln.trim_start().strip_prefix("int ")?;
    let a = ident(t)?;
    let r = t[a..].strip_prefix(" = ")?;
    let b = ident(r)?;
    let k = r[b..].strip_prefix(" + ")?;
    let k = k.trim_end().strip_suffix(';')?;
    let v: i64 = k.parse().ok()?;
    let ok = k
        .strip_prefix('-')
        .unwrap_or(k)
        .bytes()
        .all(|c| c.is_ascii_digit())
        && !k.is_empty();
    ok.then(|| (&t[..a], &r[..b], v))
}

/// `_IDX.match`: `^\s*uint (ID) = uint\(int\(uint\((ID)\) >> uint\(2\)\)\);\s*$`.
fn idx(ln: &str) -> Option<(&str, &str)> {
    let t = ln.trim_start().strip_prefix("uint ")?;
    let a = ident(t)?;
    let r = t[a..].strip_prefix(" = uint(int(uint(")?;
    let b = ident(r)?;
    let tail = r[b..].trim_end();
    (tail == ") >> uint(2)));").then(|| (&t[..a], &r[..b]))
}

/// `_OP` at the start of `s`: `(?:[-+*/%|&^]|<<|>>)?=(?!=)`.
fn op(s: &str) -> bool {
    let b = s.as_bytes();
    let k = if s.starts_with("<<") || s.starts_with(">>") {
        2
    } else if b.first().is_some_and(|c| b"-+*/%|&^".contains(c)) {
        1
    } else {
        0
    };
    let eq = |k: usize| b.get(k) == Some(&b'=') && b.get(k + 1) != Some(&b'=');
    eq(k) || (k > 0 && eq(0))
}

/// `_WRITE.search(ln)`.
fn writes(ln: &str) -> bool {
    if ln.contains("atomic_") || ln.contains("_barrier(") || ln.contains(".write(") {
        return true;
    }
    if ln
        .match_indices(']')
        .any(|(i, _)| op(ln[i + 1..].trim_start()))
    {
        return true;
    }
    let t = ln.trim_start();
    if t.starts_with('*') {
        return true;
    }
    ident(t).is_some_and(|e| op(t[e..].trim_start()))
}

/// `_LOAD.finditer(ln)`: `\(\*spvDescriptorSet0\.(BUF)\)\._m0\[int\((ID)\)\]` → (start, end, buffer, index var).
fn loads(ln: &str) -> Vec<(usize, usize, &str, &str)> {
    const P: &str = "(*spvDescriptorSet0.";
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(off) = ln[from..].find(P) {
        let i = from + off;
        let a = i + P.len();
        let hit = (|| {
            let e = a + buf(&ln[a..])?;
            let r = ln[e..].strip_prefix(")._m0[int(")?;
            let b = e + ")._m0[int(".len();
            let v = b + ident(r)?;
            ln[v..]
                .starts_with(")]")
                .then(|| (v + 2, &ln[a..e], &ln[b..v]))
        })();
        match hit {
            Some((end, bf, var)) => {
                out.push((i, end, bf, var));
                from = end;
            }
            None => from = i + 1,
        }
    }
    out
}

/// `_RO.findall(msl)` (re.M): `^\s*const device ID\* (BUF) \[\[id\(\d+\)\]\];`.
fn read_only(msl: &str) -> BTreeSet<&str> {
    let mut out = BTreeSet::new();
    for line in msl.split('\n') {
        let Some(t) = line.trim_start().strip_prefix("const device ") else {
            continue;
        };
        let Some(e) = ident(t) else { continue };
        let Some(r) = t[e..].strip_prefix("* ") else {
            continue;
        };
        let Some(b) = buf(r) else { continue };
        if let Some(x) = r[b..].strip_prefix(" [[id(") {
            let d = x.bytes().take_while(u8::is_ascii_digit).count();
            if d > 0 && x[d..].starts_with(")]];") {
                out.insert(&r[..b]);
            }
        }
    }
    out
}

/// A word read: (line, (start, end, index variable)).
type Word<'a> = (usize, (usize, usize, &'a str));
/// A line edit: (start, end, replacement, declaration to put before the line).
type Edit = (usize, usize, String, Option<String>);

/// `_root(v, adds)`.
fn root<'a>(mut v: &'a str, adds: &BTreeMap<&'a str, (&'a str, i64)>) -> (&'a str, i64) {
    let mut off = 0;
    for _ in 0..64 {
        let Some((n, k)) = adds.get(v) else { break };
        v = n;
        off += k;
    }
    (v, off)
}

/// `wide_loads(msl)`.
pub fn wide_loads(msl: &str) -> String {
    if !msl.contains(" >> uint(2)") {
        return msl.to_string();
    }
    let ro = read_only(msl);
    if ro.is_empty() {
        return msl.to_string();
    }
    let lines: Vec<&str> = msl.split('\n').collect();
    let mut adds: BTreeMap<&str, (&str, i64)> = BTreeMap::new();
    let mut idxs: BTreeMap<&str, &str> = BTreeMap::new();
    for ln in &lines {
        if let Some((a, b, k)) = add(ln) {
            adds.insert(a, (b, k));
            continue;
        }
        if let Some((a, b)) = idx(ln) {
            idxs.insert(a, b);
        }
    }
    // (block, buffer, value) → {offset: (line, (start, end, index var))}; keys in first-seen order as Python's dict.
    let mut order: Vec<(usize, &str, &str)> = Vec::new();
    let mut words: BTreeMap<(usize, &str, &str), BTreeMap<i64, Word<'_>>> = BTreeMap::new();
    let mut block = 0usize;
    for (i, ln) in lines.iter().enumerate() {
        if ln.contains('{') || ln.contains('}') || writes(ln) {
            block += 1;
            continue;
        }
        let ms = loads(ln);
        if ms.len() != 1 || !ro.contains(ms[0].2) {
            continue;
        }
        let Some(iv) = idxs.get(ms[0].3) else {
            continue;
        };
        let (v, off) = root(iv, &adds);
        if off % 4 == 0 {
            let k = (block, ms[0].2, v);
            if !words.contains_key(&k) {
                order.push(k);
            }
            words
                .entry(k)
                .or_default()
                .entry(off)
                .or_insert((i, (ms[0].0, ms[0].1, ms[0].3)));
        }
    }
    let mut edits: BTreeMap<usize, Vec<Edit>> = BTreeMap::new();
    let mut n = 0;
    for k in &order {
        let by_off = &words[k];
        let bases: BTreeSet<i64> = by_off.keys().map(|o| o - o.rem_euclid(16)).collect();
        for base in bases {
            let mut run = Vec::new();
            while run.len() < 4
                && let Some(x) = by_off.get(&(base + 4 * run.len() as i64))
            {
                run.push(*x);
            }
            if run.len() < 2 || run.iter().any(|(i, _)| *i < run[0].0) {
                continue;
            }
            let name = format!("_dmw{n}");
            let vec = ["", "", "packed_uint2", "packed_uint3", "packed_uint4"][run.len()];
            n += 1;
            let decl = format!(
                "{vec} {name} = *(const device {vec}*)&(*spvDescriptorSet0.{})._m0[int({})];",
                k.1, run[0].1.2
            );
            for (w, (i, m)) in run.iter().enumerate() {
                edits.entry(*i).or_default().push((
                    m.0,
                    m.1,
                    format!("{name}.{}", &"xyzw"[w..w + 1]),
                    (w == 0).then(|| decl.clone()),
                ));
            }
        }
    }
    let mut out: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    for (i, mut es) in edits {
        es.sort_by(|a, b| b.cmp(a));
        let mut ln = lines[i].to_string();
        let mut pre = Vec::new();
        for (s, e, rep, decl) in es {
            ln.replace_range(s..e, &rep);
            if let Some(d) = decl {
                pre.push(d);
            }
        }
        let indent = &lines[i][..lines[i].len() - lines[i].trim_start().len()];
        let mut full = String::new();
        for d in pre {
            full.push_str(indent);
            full.push_str(&d);
            full.push('\n');
        }
        full.push_str(&ln);
        out[i] = full;
    }
    out.join("\n")
}
