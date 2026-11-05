//! `fs_uint` with `int_outputs` (port of `dmsh_build.fs_uint`): the integer-output entry points `main0_u` / `main0_m`.

use super::text::{is_word, struct_group};
use std::collections::{BTreeMap, BTreeSet};

fn word_end(s: &str, i: usize) -> usize {
    s[i..]
        .char_indices()
        .find(|(_, c)| !is_word(*c))
        .map_or(s.len(), |(k, _)| i + k)
}

/// The start of the line holding the first `\bfragment\s+main0_out\s+main0\s*\(`, and the match end.
fn main_line(src: &str) -> Option<(usize, usize)> {
    for (i, _) in src.match_indices("fragment") {
        if src[..i].chars().next_back().is_some_and(is_word) {
            continue;
        }
        let r = &src[i + 8..];
        let t = r.trim_start();
        if t.len() == r.len() {
            continue;
        }
        let Some(t2) = t.strip_prefix("main0_out") else {
            continue;
        };
        let t3 = t2.trim_start();
        if t3.len() == t2.len() {
            continue;
        }
        let Some(t4) = t3.strip_prefix("main0") else {
            continue;
        };
        let t5 = t4.trim_start();
        if t5.starts_with('(') {
            let line = src[..i].rfind('\n').map_or(0, |k| k + 1);
            return Some((line, src.len() - t5.len() + 1));
        }
    }
    None
}

/// `re.sub(r'\bfragment\s+main0_out\s+main0\s*\(', REPL, s, count=1)`.
fn rename_main(s: &str, repl: &str) -> String {
    match main_line(s) {
        Some((_, end)) => {
            let start = s[..end].rfind("fragment").unwrap_or(0);
            format!("{}{repl}{}", &s[..start], &s[end..])
        }
        None => s.to_string(),
    }
}

/// `mem`: `^\s*(\w+)\s+(\w+)\s*(\[\[[^\]]*\]\])?;` (re.M) → (type, name, attribute or "").
fn members(body: &str) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for line in body.split('\n') {
        let t = line.trim_start();
        let a = word_end(t, 0);
        if a == 0 {
            continue;
        }
        let r = &t[a..];
        let r2 = r.trim_start();
        if r2.len() == r.len() {
            continue;
        }
        let b = word_end(r2, 0);
        if b == 0 {
            continue;
        }
        let r3 = r2[b..].trim_start();
        let (at, rest) = if let Some(x) = r3.strip_prefix("[[") {
            match x.find(']') {
                Some(k) if x[k..].starts_with("]]") => (&r3[..2 + k + 2], &x[k + 2..]),
                _ => continue,
            }
        } else {
            ("", r3)
        };
        if rest.starts_with(';') || (at.is_empty() && r2[b..].starts_with(';')) {
            out.push((t[..a].to_string(), r2[..b].to_string(), at.to_string()));
        }
    }
    out
}

/// `\bout\.NM(?:\.\w+)? = ([^;]+);` matches in `s` → the values.
fn writes_of<'a>(s: &'a str, nm: &str) -> Vec<&'a str> {
    let pat = format!("out.{nm}");
    let mut out = Vec::new();
    for (i, _) in s.match_indices(&pat) {
        if s[..i].chars().next_back().is_some_and(is_word) {
            continue;
        }
        let mut j = i + pat.len();
        if s[j..].starts_with('.') {
            let e = word_end(s, j + 1);
            if e > j + 1 {
                j = e;
            }
        }
        let Some(r) = s[j..].strip_prefix(" = ") else {
            continue;
        };
        if let Some(k) = r.find(';')
            && k > 0
        {
            out.push(&r[..k]);
        }
    }
    out
}

fn is_unum(s: &str) -> bool {
    s.len() > 1 && s.starts_with('_') && s[1..].bytes().all(|c| c.is_ascii_digit())
}

/// The `(u?int\d?|float\d?|bool)` type word at the start of `t` → its end.
fn ty_word(t: &str) -> Option<usize> {
    for base in ["uint", "int", "float", "bool"] {
        if let Some(r) = t.strip_prefix(base) {
            if base != "bool" && r.starts_with(|c: char| c.is_ascii_digit()) {
                let after = &r[1..];
                if after.starts_with(char::is_whitespace) {
                    return Some(base.len() + 1);
                }
            }
            if r.starts_with(char::is_whitespace) {
                return Some(base.len());
            }
        }
    }
    None
}

struct Kinds<'a> {
    types: BTreeMap<&'a str, &'a str>,
    defs: BTreeMap<&'a str, Vec<&'a str>>,
}

impl<'a> Kinds<'a> {
    fn new(body: &'a str) -> Self {
        let mut types = BTreeMap::new();
        let mut defs: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for line in body.split('\n') {
            let t = line.trim_start();
            // types: ^\s*(u?int\d?|float\d?|bool)\s+(_\d+)\b
            let mut name_at = 0;
            if let Some(e) = ty_word(t) {
                let r = t[e..].trim_start();
                let n = r.find(|c: char| !is_word(c)).unwrap_or(r.len());
                let nm = &r[..n];
                let numeric = nm.len() > 1
                    && nm.starts_with('_')
                    && nm[1..].bytes().all(|c| c.is_ascii_digit());
                if numeric {
                    types.insert(nm, &t[..e]);
                }
                name_at = t.len() - r.len();
            }
            // defs: ^\s*(?:TYPE\s+)?(_\d+)\s*=\s*([^;]+);
            for start in [name_at, 0] {
                let r = &t[start..];
                if !r.starts_with('_') {
                    continue;
                }
                let d = r[1..].bytes().take_while(u8::is_ascii_digit).count();
                if d == 0 {
                    continue;
                }
                let nm = &r[..1 + d];
                let rest = r[1 + d..].trim_start();
                if let Some(x) = rest.strip_prefix('=') {
                    let x = x.trim_start();
                    if let Some(k) = x.find(';')
                        && k > 0
                    {
                        defs.entry(nm).or_default().push(x[..k].trim());
                        break;
                    }
                }
            }
        }
        Kinds { types, defs }
    }

    fn kinds(&self, e: &str, seen: &mut BTreeSet<String>) -> BTreeSet<&'static str> {
        let mut out = BTreeSet::new();
        if is_unum(e) {
            if self.types.get(e).is_some_and(|t| t.starts_with("float")) {
                out.insert("float");
                return out;
            }
            if !seen.insert(e.to_string()) {
                return out;
            }
            for x in self.defs.get(e).map(Vec::as_slice).unwrap_or(&[]) {
                out.extend(self.kinds(x, seen));
            }
            return out;
        }
        let num = e.strip_suffix('u').unwrap_or(e);
        let digits = num.strip_prefix('-').unwrap_or(num);
        if !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit()) {
            if let Ok(v) = digits.parse::<u128>()
                && v > 0
                && v < 0x80_0000
            {
                out.insert("int");
            }
            return out;
        }
        if let Some(q) = e.find('?')
            && q > 0
            && let Some(c) = e[q + 1..].find(':')
        {
            let a = e[q + 1..q + 1 + c].trim();
            let b = e[q + 1 + c + 1..].trim_start();
            if !a.is_empty() && !b.is_empty() && !b.contains('\n') {
                let mut k = self.kinds(a, seen);
                k.extend(self.kinds(b, seen));
                return k;
            }
        }
        if let Some(inner) = as_type_int(e) {
            let numlike = inner.starts_with(|c: char| c.is_ascii_digit() || c == '.')
                || inner
                    .strip_prefix('-')
                    .is_some_and(|r| r.starts_with(|c: char| c.is_ascii_digit() || c == '.'));
            let fp = inner
                .match_indices("fp_c")
                .any(|(i, _)| inner[i + 4..].starts_with(|c: char| c.is_ascii_digit()));
            let input = inner
                .match_indices("in.")
                .any(|(i, _)| !inner[..i].chars().next_back().is_some_and(is_word));
            if !(fp || input || numlike) {
                out.insert("float");
            }
            return out;
        }
        if !(e.starts_with("(*spvDescriptorSet0") || e.starts_with("spvDescriptorSet0")) {
            out.insert("int");
        }
        out
    }
}

/// `re.fullmatch(r'as_type<u?int\d?>\((.*)\)', e)` → the inner text.
fn as_type_int(e: &str) -> Option<&str> {
    let r = e.strip_prefix("as_type<")?;
    let r = r.strip_prefix('u').unwrap_or(r).strip_prefix("int")?;
    let r = r.strip_prefix(|c: char| c.is_ascii_digit()).unwrap_or(r);
    let inner = r.strip_prefix(">(")?.strip_suffix(')')?;
    (!inner.contains('\n')).then_some(inner)
}

/// `re.fullmatch(r'as_type<float\d?>\((.*)\)', w)`.
fn as_type_float(w: &str) -> Option<&str> {
    let r = w.strip_prefix("as_type<float")?;
    let r = r.strip_prefix(|c: char| c.is_ascii_digit()).unwrap_or(r);
    let inner = r.strip_prefix(">(")?.strip_suffix(')')?;
    (!inner.contains('\n')).then_some(inner)
}

/// `-?[0-9.]+(?:e[-+]?\d+)?f?` fullmatch → its float value.
fn float_literal(w: &str) -> Option<f64> {
    let s = w.strip_suffix('f').unwrap_or(w);
    let m = s.strip_prefix('-').unwrap_or(s);
    let (mant, exp) = match m.find('e') {
        Some(k) => (&m[..k], Some(&m[k + 1..])),
        None => (m, None),
    };
    if mant.is_empty() || !mant.bytes().all(|c| c.is_ascii_digit() || c == b'.') {
        return None;
    }
    if let Some(x) = exp {
        let d = x.strip_prefix(['-', '+']).unwrap_or(x);
        if d.is_empty() || !d.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
    }
    w.strip_suffix('f').unwrap_or(w).parse().ok()
}

fn int_outputs(body: &str, cols: &[(String, String)]) -> Vec<String> {
    let k = Kinds::new(body);
    let mut found: Vec<(String, BTreeSet<&str>)> = Vec::new();
    for (nm, _) in cols {
        let mut set = BTreeSet::new();
        for w in writes_of(body, nm) {
            let w = w.trim();
            if let Some(a) = as_type_float(w) {
                set.extend(k.kinds(a.trim(), &mut BTreeSet::new()));
            } else if let Some(v) = float_literal(w) {
                if v.abs() > 0.0 && v.abs() < 1.17549e-38 {
                    set.insert("int");
                }
            } else {
                set.insert("float");
            }
        }
        found.push((nm.clone(), set));
    }
    let clean: Vec<String> = found
        .iter()
        .filter(|(_, s)| s.len() == 1 && s.contains("int"))
        .map(|(n, _)| n.clone())
        .collect();
    if !clean.is_empty() {
        return clean;
    }
    found
        .into_iter()
        .filter(|(_, s)| s.contains("int"))
        .map(|(n, _)| n)
        .collect()
}

/// `re.fullmatch(r'-?[0-9.]+f?|float\d\([-0-9., ]*\)', w)`.
fn constant_write(w: &str) -> bool {
    let m = w.strip_prefix('-').unwrap_or(w);
    let m = m.strip_suffix('f').unwrap_or(m);
    if !m.is_empty() && m.bytes().all(|c| c.is_ascii_digit() || c == b'.') {
        return true;
    }
    w.strip_prefix("float")
        .and_then(|r| r.strip_prefix(|c: char| c.is_ascii_digit()))
        .and_then(|r| r.strip_prefix('('))
        .and_then(|r| r.strip_suffix(')'))
        .is_some_and(|r| r.bytes().all(|c| b"-0123456789., ".contains(&c)))
}

/// `fs_uint(src)` → (source with `main0_u` (and `main0_m`), the `main0_m` mask), or `None`.
pub fn fs_uint(src: &str) -> Option<(String, u32)> {
    let (m_start, _) = main_line(src)?;
    let (gs, ge) = struct_group(src, "struct main0_out")?;
    let mem = members(&src[gs..ge]);
    let cols: Vec<(String, String)> = mem
        .iter()
        .filter(|(ty, _, at)| at.contains("color(") && ty.starts_with("float"))
        .map(|(ty, nm, _)| (nm.clone(), ty.clone()))
        .collect();
    let col_ty = |nm: &str| cols.iter().find(|(n, _)| n == nm).map(|(_, t)| t.clone());
    let from_int: BTreeSet<String> = cols
        .iter()
        .filter(|(nm, _)| {
            writes_of(src, nm).iter().any(|w| {
                w.strip_prefix("as_type<float")
                    .map(|r| r.strip_prefix(|c: char| c.is_ascii_digit()).unwrap_or(r))
                    .is_some_and(|r| r.starts_with(">("))
            })
        })
        .map(|(nm, _)| nm.clone())
        .collect();
    let writes: Vec<&str> = cols.iter().flat_map(|(nm, _)| writes_of(src, nm)).collect();
    let flat: BTreeSet<&str> = src
        .match_indices(" [[user(locn")
        .filter_map(|(i, _)| {
            let r = &src[i + 12..];
            let d = r.bytes().take_while(u8::is_ascii_digit).count();
            if d == 0 || !r[d..].starts_with("), flat]]") {
                return None;
            }
            let s = src[..i]
                .char_indices()
                .rev()
                .take_while(|(_, c)| is_word(*c))
                .last()
                .map(|(k, _)| k)?;
            Some(&src[s..i])
        })
        .collect();
    let passed = |w: &str| -> bool {
        let w = w.trim();
        let mut target = w.to_string();
        if !w.is_empty() && w.chars().all(is_word) {
            for f in ["float ", "float2 ", "float3 ", "float4 "] {
                let pat = format!("{f}{w} = in.");
                if let Some(i) = src
                    .match_indices(&pat)
                    .map(|(i, _)| i)
                    .find(|i| !src[..*i].chars().next_back().is_some_and(is_word))
                {
                    let r = &src[i + pat.len() - 3..];
                    if let Some(k) = r.find(';') {
                        let cand = &r[..k];
                        if valid_in_ref(cand) {
                            target = cand.to_string();
                            break;
                        }
                    }
                }
            }
        }
        in_member(&target).is_some_and(|m| flat.contains(m))
    };
    let mut body = &src[m_start..];
    if let Some(k) = body.find("return out;") {
        body = &body[..k + 1];
    }
    let ints = int_outputs(body, &cols);
    let all_const = !writes.is_empty() && writes.iter().all(|w| constant_write(w.trim()));
    let all_passed = !writes.is_empty() && writes.iter().all(|w| passed(w));
    if from_int.is_empty() && ints.is_empty() && !(all_const || all_passed) {
        return None;
    }
    let uty = |ty: &str| format!("u{}", ty.replace("float", "int"));
    let entry = |sfx: &str, as_uint: &dyn Fn(&str) -> bool| -> String {
        let mut st = format!("struct main0_out_{sfx}\n{{\n");
        for (ty, nm, at) in &mem {
            let t = if as_uint(nm) {
                uty(&col_ty(nm).unwrap_or_default())
            } else {
                ty.clone()
            };
            st.push_str(&format!("    {t} {nm} {at};\n"));
        }
        st.push_str("};\n\n");
        let vals: Vec<String> = mem
            .iter()
            .map(|(_, nm, _)| {
                if as_uint(nm) {
                    format!(
                        "as_type<{}>(out.{nm})",
                        uty(&col_ty(nm).unwrap_or_default())
                    )
                } else {
                    format!("out.{nm}")
                }
            })
            .collect();
        let ret = format!("return main0_out_{sfx}{{{}}};", vals.join(", "));
        let tail = rename_main(
            &src[m_start..],
            &format!("fragment main0_out_{sfx} main0_{sfx}("),
        );
        format!("\n{st}{}", tail.replace("return out;", &ret))
    };
    let col_names: BTreeSet<String> = cols.iter().map(|(n, _)| n.clone()).collect();
    let mut out = format!("{src}{}", entry("u", &|n| col_names.contains(n)));
    let as_int: BTreeSet<String> = if ints.is_empty() {
        from_int.clone()
    } else {
        ints.into_iter().collect()
    };
    let mut mask = 0u32;
    if !as_int.is_empty() && col_names.iter().any(|n| !as_int.contains(n)) {
        out.push_str(&entry("m", &|n| as_int.contains(n)));
        for (_, nm, at) in &mem {
            if as_int.contains(nm)
                && let Some(i) = at.find("color(")
            {
                let r = &at[i + 6..];
                let d = r.bytes().take_while(u8::is_ascii_digit).count();
                if let Ok(c) = r[..d].parse::<u32>() {
                    mask += 1 << c;
                }
            }
        }
    }
    Some((out, mask))
}

/// `in\.\w+(?:\.\w+)?` fullmatch.
fn valid_in_ref(s: &str) -> bool {
    in_member(s).is_some()
}

/// `re.fullmatch(r'in\.(\w+)(?:\.\w+)?', s)` → the member.
fn in_member(s: &str) -> Option<&str> {
    let r = s.strip_prefix("in.")?;
    let e = r.find(|c: char| !is_word(c)).unwrap_or(r.len());
    if e == 0 {
        return None;
    }
    let rest = &r[e..];
    if rest.is_empty() {
        return Some(&r[..e]);
    }
    let x = rest.strip_prefix('.')?;
    (!x.is_empty() && x.chars().all(is_word)).then(|| &r[..e])
}
