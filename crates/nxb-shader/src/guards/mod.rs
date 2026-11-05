//! The c1 guard and uniform extent tables (ports of `c1_guard.py` and `cb_extent.py`): which shaders' reads of their
//! embedded constants are not proven bounded, and how far into each uniform slot a shader reads.

mod defs;
pub mod interval;

use interval::{Definitions, Interval, definitions_of};
use nxb_pyjson::Value;
use std::collections::{BTreeMap, BTreeSet};

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn prev_word(s: &str, i: usize) -> bool {
    s[..i].chars().next_back().is_some_and(is_word)
}

fn next_word(s: &str, i: usize) -> bool {
    s[i..].chars().next().is_some_and(is_word)
}

/// `ACCESS.findall(source)`: `\b[a-z]+_c1\)\._m0\[([^\]]+)\]`.
fn c1_accesses(src: &str) -> BTreeSet<&str> {
    let mut out = BTreeSet::new();
    for (i, m) in src.match_indices("_c1)._m0[") {
        let run = src[..i]
            .bytes()
            .rev()
            .take_while(u8::is_ascii_lowercase)
            .count();
        if run == 0 || prev_word(src, i - run) {
            continue;
        }
        let a = i + m.len();
        if let Some(k) = src[a..].find(']')
            && k > 0
        {
            out.insert(&src[a..a + k]);
        }
    }
    out
}

/// `proven(index, definitions, span)`.
fn proven(index: &str, defs: &Definitions, span: Option<i128>) -> bool {
    let (low, high) = Interval::new(defs).expression(index);
    matches!((span, low, high), (Some(span), Some(l), Some(h)) if l >= 0 && 16 * (h + 1) <= span)
}

/// `c1_span`: bytes the host's shared bank gives the shader's c1 (from the reflection's `c1` hex), `None` without one.
pub fn c1_span(refl: Option<&Value>) -> Option<i128> {
    let refl = refl?;
    let len = refl
        .get("c1")
        .and_then(Value::as_str)
        .map_or(0, |s| s.chars().count() / 2) as i128;
    Some((len + 255) / 256 * 256 + 256)
}

/// `unproven`: true when one of the shader's c1 reads is not proven inside its span. `refl` is its `_b.json` (or `None`
/// when that cannot be read).
pub fn c1_unproven(src: &str, refl: Option<&Value>) -> bool {
    let acc = c1_accesses(src);
    if acc.is_empty() {
        return false;
    }
    let defs = definitions_of(src);
    let span = c1_span(refl);
    !acc.iter().all(|i| proven(i, &defs, span))
}

/// Count of `\bNAME\b` (non-overlapping).
fn count_word(src: &str, name: &str) -> usize {
    src.match_indices(name)
        .filter(|(i, _)| !prev_word(src, *i) && !next_word(src, i + name.len()))
        .count()
}

const MAX: i128 = 65536;

/// `extents(source, bindings)`: `{reg: bytes}` for the uniform bindings whose every read is a proven `_m0` index.
fn extents(src: &str, bindings: &[Value]) -> BTreeMap<i64, i128> {
    let mut out = BTreeMap::new();
    let mut defs: Option<Definitions> = None;
    for b in bindings {
        if b.get("kind").and_then(Value::as_str) != Some("cbuf") {
            continue;
        }
        let Some(reg) = b.get("reg") else { continue };
        let name = b.get("msl_name").and_then(Value::as_str).unwrap_or("");
        let uses = count_word(src, name);
        let prefix = format!("(*spvDescriptorSet0.{name})._m0[");
        let reads: Vec<&str> = src
            .match_indices(&prefix)
            .filter_map(|(i, _)| {
                let a = i + prefix.len();
                src[a..]
                    .find(']')
                    .filter(|k| *k > 0)
                    .map(|k| &src[a..a + k])
            })
            .collect();
        let declared = src
            .match_indices(name)
            .filter(|(i, _)| {
                let e = i + name.len();
                !prev_word(src, *i) && src[e..].trim_start().starts_with("[[id(")
            })
            .count();
        if uses != reads.len() + declared {
            continue;
        }
        let mut high: Option<i128> = Some(-1);
        let set: BTreeSet<&str> = reads.into_iter().collect();
        for index in set {
            let d = defs.get_or_insert_with(|| definitions_of(src));
            let (low, top) = Interval::new(d).expression(index);
            match (low, top) {
                (Some(l), Some(t)) if l >= 0 => high = high.map(|h| h.max(t)),
                _ => {
                    high = None;
                    break;
                }
            }
        }
        let Some(h) = high else { continue };
        if 16 * (h + 1) >= MAX {
            continue;
        }
        if let Some(r) = reg.as_i64() {
            out.insert(r, 16 * (h + 1));
        }
    }
    out
}

/// `WRITE.match(source, j)`: `\s*(?:=(?!=)|[-+*/%&|^]=|<<=|>>=|\+\+|--)`.
fn write_at(src: &str, j: usize) -> bool {
    let t = src[j..].trim_start();
    let b = t.as_bytes();
    (b.first() == Some(&b'=') && b.get(1) != Some(&b'='))
        || (b.len() >= 2 && b"-+*/%&|^".contains(&b[0]) && b[1] == b'=')
        || t.starts_with("<<=")
        || t.starts_with(">>=")
        || t.starts_with("++")
        || t.starts_with("--")
}

/// `unwritten(source, bindings)`: storage slots the shader never writes, sorted.
fn unwritten(src: &str, bindings: &[Value]) -> Vec<i64> {
    let mut out = BTreeSet::new();
    for b in bindings {
        if b.get("kind").and_then(Value::as_str) != Some("sbuf") {
            continue;
        }
        let Some(reg) = b.get("reg") else { continue };
        let name = format!(
            "spvDescriptorSet0.{}",
            b.get("msl_name").and_then(Value::as_str).unwrap_or("")
        );
        let deref = format!("(*{name}");
        let mut ok = true;
        let mut derefs = 0;
        for (i, _) in src.match_indices(&deref) {
            let mut j = i + deref.len();
            if next_word(src, j) {
                continue;
            }
            derefs += 1;
            if src[..i].ends_with('&') || !src[j..].starts_with(")._m0[") {
                ok = false;
                break;
            }
            j += ")._m0".len();
            let mut depth = 0i64;
            for (k, c) in src[j..].char_indices() {
                if c == '[' {
                    depth += 1;
                } else if c == ']' {
                    depth -= 1;
                } else if depth == 0 && !(c == '.' || c.is_alphanumeric() || c == '_') {
                    j += k;
                    break;
                }
                if j + k + c.len_utf8() == src.len() {
                    j = src.len();
                }
            }
            if write_at(src, j) {
                ok = false;
                break;
            }
        }
        let plain = src
            .match_indices(&name)
            .filter(|(i, _)| !next_word(src, i + name.len()))
            .count();
        if ok
            && plain == derefs
            && let Some(r) = reg.as_i64()
        {
            out.insert(r);
        }
    }
    out.into_iter().collect()
}

/// `cb_extent.row` for one shader: the table line's tokens (`reg:bytes` sorted by reg, then `s<reg>`), or `None`.
pub fn cb_row(src: &str, refl: &Value) -> Option<String> {
    let empty = Vec::new();
    let bindings = refl
        .get("bindings")
        .and_then(Value::as_list)
        .unwrap_or(&empty);
    let has = bindings
        .iter()
        .any(|b| matches!(b.get("kind").and_then(Value::as_str), Some("cbuf" | "sbuf")));
    if !has {
        return None;
    }
    let mut toks: Vec<String> = extents(src, bindings)
        .into_iter()
        .map(|(r, n)| format!("{r}:{n}"))
        .collect();
    toks.extend(
        unwritten(src, bindings)
            .into_iter()
            .map(|r| format!("s{r}")),
    );
    (!toks.is_empty()).then(|| toks.join(" "))
}
