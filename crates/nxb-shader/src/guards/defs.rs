//! `definitions_of` (port of `c1_guard.definitions_of` with its `ASSIGNMENT` and `DEFINITION` patterns).

use std::collections::BTreeMap;

/// `(kind, text)` of each integer temporary written only where it is declared.
pub type Definitions = BTreeMap<String, (String, String)>;

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn digits_end(s: &str, i: usize) -> usize {
    i + s[i..].bytes().take_while(u8::is_ascii_digit).count()
}

fn prev_is_word(s: &str, i: usize) -> bool {
    s[..i].chars().next_back().is_some_and(is_word)
}

/// `definitions_of(source)`.
pub fn definitions_of(src: &str) -> Definitions {
    let mut writes: BTreeMap<&str, usize> = BTreeMap::new();
    for n in assignments(src) {
        *writes.entry(n).or_default() += 1;
    }
    let mut decls: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for (kind, name, text) in definitions(src) {
        let e = decls.entry(name).or_default();
        let t = text.trim();
        if !e.contains(&(kind, t)) {
            e.push((kind, t));
        }
        *counts.entry(name).or_default() += 1;
    }
    decls
        .into_iter()
        .filter(|(n, d)| d.len() == 1 && writes.get(n).copied().unwrap_or(0) == counts[n])
        .map(|(n, d)| (n.to_string(), (d[0].0.to_string(), d[0].1.to_string())))
        .collect()
}

/// `_\d+` at `i` (`'_'` then digits) → end.
fn unum(s: &str, i: usize) -> Option<usize> {
    if !s[i..].starts_with('_') {
        return None;
    }
    let d = digits_end(s, i + 1);
    (d > i + 1).then_some(d)
}

/// `ASSIGNMENT.finditer`: names written (assignment, compound assignment, `++`, `--`).
fn assignments(s: &str) -> Vec<&str> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let lb_ok =
            !(i > 0 && (is_word(s[..i].chars().next_back().unwrap_or(' ')) || b[i - 1] == b'.'));
        if lb_ok && let Some(e) = unum(s, i) {
            let j = e + (s[e..].len() - s[e..].trim_start().len());
            // alt 1: (?:<<|>>|[-+*/%&|^])?=(?!=)
            let k = if s[j..].starts_with("<<") || s[j..].starts_with(">>") {
                j + 2
            } else if s[j..].starts_with(['-', '+', '*', '/', '%', '&', '|', '^']) {
                j + 1
            } else {
                j
            };
            let alt1 = |k: usize| b.get(k) == Some(&b'=') && b.get(k + 1) != Some(&b'=');
            if alt1(k) {
                out.push(&s[i..e]);
                i = k + 1;
                continue;
            }
            if k != j && alt1(j) {
                out.push(&s[i..e]);
                i = j + 1;
                continue;
            }
            // alt 2: \s*(?:\+\+|--)
            if s[j..].starts_with("++") || s[j..].starts_with("--") {
                out.push(&s[i..e]);
                i = j + 2;
                continue;
            }
        }
        // alt 3: (?:\+\+|--)\s*(_\d+)\b
        if s[i..].starts_with("++") || s[i..].starts_with("--") {
            let j = i + 2 + (s[i + 2..].len() - s[i + 2..].trim_start().len());
            if let Some(e) = unum(s, j)
                && !s[e..].chars().next().is_some_and(is_word)
            {
                out.push(&s[j..e]);
                i = e;
                continue;
            }
        }
        i += s[i..].chars().next().map_or(1, char::len_utf8);
    }
    out
}

/// `DEFINITION.findall`: `\b(u?int|u?short|u?long|bool)\s+(_\d+)\s*=\s*([^;]+);`.
fn definitions(s: &str) -> Vec<(&str, &str, &str)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < s.len() {
        let mut hit = None;
        if !prev_is_word(s, i) {
            for kind in ["int", "uint", "short", "ushort", "long", "ulong", "bool"] {
                if !s[i..].starts_with(kind) {
                    continue;
                }
                let a = i + kind.len();
                let w = a + (s[a..].len() - s[a..].trim_start().len());
                if w == a {
                    continue;
                }
                let Some(e) = unum(s, w) else { continue };
                let q = e + (s[e..].len() - s[e..].trim_start().len());
                if !s[q..].starts_with('=') {
                    continue;
                }
                let Some(semi) = s[q + 1..].find(';').map(|k| q + 1 + k) else {
                    continue;
                };
                if semi == q + 1 {
                    continue;
                }
                hit = Some(((kind, &s[w..e], &s[q + 1..semi]), semi + 1));
                break;
            }
        }
        match hit {
            Some((m, end)) => {
                out.push(m);
                i = end;
            }
            None => i += s[i..].chars().next().map_or(1, char::len_utf8),
        }
    }
    out
}
