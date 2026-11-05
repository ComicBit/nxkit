//! The compile unit (port of `dmsh_side.entries` and `dmsh_side.unit`).

use super::text::{is_word, skip_word, skip_ws, strip};
use super::{alpha, logic};
use std::collections::BTreeSet;

/// One fragment entry point of a source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    /// Text before `fragment` on its line (an attribute such as `[[ early_fragment_tests ]] `).
    pub(crate) attr: String,
    /// Return type.
    pub(crate) ret: String,
    /// Name.
    pub(crate) name: String,
    /// Parameters as written.
    pub(crate) params: Vec<String>,
    /// Parameters without attributes, joined by `, `.
    pub(crate) decl: String,
    /// Argument names, joined by `, `.
    pub(crate) args: String,
    start: usize,
    head: usize,
    end: usize,
}

/// `_close(src, i, o, c)`: the index after the bracket closing the one opened just before `i`.
fn close(src: &[u8], mut i: usize, o: u8, c: u8) -> Option<usize> {
    let mut depth: i64 = 1;
    while depth != 0 && i < src.len() {
        depth += i64::from(src[i] == o) - i64::from(src[i] == c);
        i += 1;
    }
    (depth == 0).then_some(i)
}

/// `_params(s)`: commas inside `<>`, `()` or `[]` do not separate (depth may go negative, as in Python).
fn params(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (mut at, mut depth) = (0usize, 0i64);
    for (i, ch) in s.char_indices() {
        depth += i64::from("<([".contains(ch)) - i64::from(">)]".contains(ch));
        if ch == ',' && depth == 0 {
            out.push(strip(&s[at..i]).to_string());
            at = i + 1;
        }
    }
    if !strip(&s[at..]).is_empty() {
        out.push(strip(&s[at..]).to_string());
    }
    out
}

/// `re.sub(r'\s*\[\[.*\]\]$', '', p)` for a stripped parameter: cut the first `[[` (with the white space before it)
/// whose rest has no newline and ends with `]]`.
fn strip_attr(p: &str) -> &str {
    for (j, _) in p.match_indices("[[") {
        let rest = &p[j..];
        if rest.len() >= 4 && rest.ends_with("]]") && !rest[2..rest.len() - 2].contains('\n') {
            let i = p[..j].trim_end_matches(super::text::is_ws).len();
            return &p[..i];
        }
    }
    p
}

/// `(\w+)$`: the trailing identifier.
fn last_word(d: &str) -> Option<&str> {
    let start = d
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_word(*c))
        .last()
        .map(|(i, _)| i)?;
    Some(&d[start..])
}

/// `_ENTRY`: `^((?:\[\[[^\]]*\]\]\s*)?)fragment\s+(\w+)\s+(\w+)\s*\(` (re.M) at line start `l`:
/// (attr, ret, name, index after the `(`).
fn entry_at(s: &str, l: usize) -> Option<(String, String, String, usize)> {
    let mut p = l;
    if s[p..].starts_with("[[") {
        let close = s[p + 2..].find(']').map(|k| p + 2 + k)?;
        if s[close..].starts_with("]]") {
            let after = skip_ws(s, close + 2);
            if s[after..].starts_with("fragment") {
                p = after;
            }
        }
    }
    let attr = s[l..p].to_string();
    let rest = s[p..].strip_prefix("fragment")?;
    let a = s.len() - rest.len();
    let b = skip_ws(s, a);
    let c = skip_word(s, b);
    let d = skip_ws(s, c);
    let e = skip_word(s, d);
    let f = skip_ws(s, e);
    if b == a || c == b || d == c || e == d || !s[f..].starts_with('(') {
        return None;
    }
    Some((attr, s[b..c].to_string(), s[d..e].to_string(), f + 1))
}

/// `entries(msl)`: fragment entry points in source order.`Err` on a shape the port does not know.
pub(crate) fn entries(msl: &str) -> Result<Vec<Entry>, String> {
    let b = msl.as_bytes();
    let mut out = Vec::new();
    let mut next_from = 0usize;
    // `^` matches at line starts only: the attribute group starts exactly there (no leading white space).
    let starts = std::iter::once(0).chain(msl.match_indices('\n').map(|(i, _)| i + 1));
    for l in starts {
        if l < next_from || l >= msl.len() {
            continue;
        }
        let Some((attr, ret, name, open)) = entry_at(msl, l) else {
            continue;
        };
        let head = close(b, open, b'(', b')');
        let body = head.and_then(|h| msl[h..].find('{').map(|k| h + k));
        let end = body.and_then(|bo| close(b, bo + 1, b'{', b'}'));
        let (Some(head), Some(end)) = (head, end) else {
            return Err(format!("no body for {name}"));
        };
        let ps = params(&msl[open..head - 1]);
        let (mut decl, mut args) = (Vec::new(), Vec::new());
        for p in &ps {
            let d = strip_attr(p);
            let a = last_word(d);
            match a {
                Some(a) if !p.contains("function_constant") => {
                    decl.push(d.to_string());
                    args.push(a.to_string());
                }
                _ => {
                    let short: String = p.chars().take(80).collect();
                    return Err(format!("a parameter of {name}: {short}"));
                }
            }
        }
        out.push(Entry {
            attr,
            ret,
            name,
            params: ps,
            decl: decl.join(", "),
            args: args.join(", "),
            start: l,
            head,
            end,
        });
        // finditer continues after the match, which ends at the `(`.
        next_from = open;
    }
    Ok(out)
}

/// A compile unit and the names of its side entry points.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unit {
    /// The unit's source.
    pub source: String,
    /// Alpha test entry points, in source order.
    pub alpha: Vec<String>,
    /// Logic operation entry points, in source order.
    pub logic: Vec<String>,
}

impl Unit {
    /// The functions of the side library (`info['alpha'] + info['logic']`).
    pub fn side(&self) -> Vec<String> {
        self.alpha.iter().chain(&self.logic).cloned().collect()
    }
}

/// `unit(msl)`: the compile unit, or why the shader gets none.
pub fn unit(msl: &str) -> Result<Unit, String> {
    let es = entries(msl)?;
    if es.first().is_none_or(|e| e.name != "main0") {
        return Err("no fragment entry point main0".into());
    }
    let (no_alpha, no_logic) = (alpha::refused(msl), logic::refused(msl));
    let mut alphas: Vec<(String, Option<String>)> = if no_alpha.is_none() {
        es.iter()
            .map(|e| (format!("{}_a", e.name), alpha::entry(msl, e)))
            .collect()
    } else {
        Vec::new()
    };
    let mut no_alpha = no_alpha;
    if alphas.iter().any(|(_, t)| t.is_none()) {
        alphas.clear();
        no_alpha = Some("an entry point without a colour 0".into());
    }
    let logics: Vec<(String, (String, BTreeSet<u32>))> = if no_logic.is_none() {
        es.iter()
            .filter_map(|e| logic::entry(msl, e).map(|o| (format!("{}_l", e.name), o)))
            .collect()
    } else {
        Vec::new()
    };
    if alphas.is_empty() && logics.is_empty() {
        return Err(no_alpha
            .or(no_logic)
            .unwrap_or_else(|| "no colour output".into()));
    }
    let targets: BTreeSet<u32> = logics
        .iter()
        .flat_map(|(_, (_, t))| t.iter().copied())
        .collect();
    let first = es[0].start;
    let mut src = String::with_capacity(msl.len() * 2);
    src.push_str(&msl[..first]);
    src.push_str(alpha::HELPER);
    src.push_str(logic::HELPER);
    src.push_str(&logic::constants(&targets));
    let mut at = first;
    for e in &es {
        src.push_str(&msl[at..e.start]);
        src.push_str(&format!("static {} dm_body_{}({})", e.ret, e.name, e.decl));
        src.push_str(&msl[e.head..e.end]);
        at = e.end;
    }
    src.push_str(&msl[at..]);
    src.push('\n');
    for e in &es {
        src.push_str(&format!(
            "{}fragment {} {}({})\n{{\n    return dm_body_{}({});\n}}\n",
            e.attr,
            e.ret,
            e.name,
            e.params.join(", "),
            e.name,
            e.args
        ));
    }
    for (_, t) in &alphas {
        if let Some(t) = t {
            src.push_str(t);
        }
    }
    for (_, (t, _)) in &logics {
        src.push_str(t);
    }
    Ok(Unit {
        source: src,
        alpha: alphas.into_iter().map(|(n, _)| n).collect(),
        logic: logics.into_iter().map(|(n, _)| n).collect(),
    })
}
