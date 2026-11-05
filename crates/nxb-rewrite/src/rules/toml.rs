//! The TOML subset of the rule files: comments, `key = value` with strings, integers and arrays of strings, and
//! `[[a.b.c]]` tables (each header appends a table below the last element of every earlier array in its path).

use crate::error::{RewriteError, fail};

/// A value of the subset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Val {
    /// A string.
    Str(String),
    /// An integer.
    Int(i64),
    /// A boolean.
    Bool(bool),
    /// An array of strings.
    List(Vec<String>),
    /// An array of tables.
    Tables(Vec<Table>),
}

/// A table: keys in file order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Table(pub(crate) Vec<(String, Val)>);

impl Table {
    pub(crate) fn get(&self, k: &str) -> Option<&Val> {
        self.0.iter().find(|(n, _)| n == k).map(|(_, v)| v)
    }
    pub(crate) fn str(&self, k: &str) -> Result<&str, RewriteError> {
        match self.get(k) {
            Some(Val::Str(s)) => Ok(s),
            _ => fail(format!("rules: key {k:?} is missing or not a string")),
        }
    }
    pub(crate) fn int(&self, k: &str) -> Result<i64, RewriteError> {
        match self.get(k) {
            Some(Val::Int(i)) => Ok(*i),
            _ => fail(format!("rules: key {k:?} is missing or not an integer")),
        }
    }
    pub(crate) fn list(&self, k: &str) -> Result<&[String], RewriteError> {
        match self.get(k) {
            Some(Val::List(l)) => Ok(l),
            _ => fail(format!("rules: key {k:?} is missing or not a list")),
        }
    }
    pub(crate) fn tables(&self, k: &str) -> &[Table] {
        match self.get(k) {
            Some(Val::Tables(t)) => t,
            _ => &[],
        }
    }
}

/// Quotes a string.
pub(crate) fn quote(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\t' => o.push_str("\\t"),
            '\r' => o.push_str("\\r"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                o.push_str(&format!("\\u{:04X}", c as u32));
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn unquote(s: &str, line: usize) -> Result<(String, &str), RewriteError> {
    let mut it = s.char_indices();
    it.next();
    let mut o = String::new();
    while let Some((i, c)) = it.next() {
        match c {
            '"' => return Ok((o, &s[i + 1..])),
            '\\' => match it.next().map(|(_, c)| c) {
                Some('"') => o.push('"'),
                Some('\\') => o.push('\\'),
                Some('n') => o.push('\n'),
                Some('t') => o.push('\t'),
                Some('r') => o.push('\r'),
                Some('u') => {
                    let h: String = it.by_ref().take(4).map(|(_, c)| c).collect();
                    match u32::from_str_radix(&h, 16).ok().and_then(char::from_u32) {
                        Some(c) => o.push(c),
                        None => return fail(format!("rules: line {line}: bad \\u escape")),
                    }
                }
                _ => return fail(format!("rules: line {line}: bad escape")),
            },
            c => o.push(c),
        }
    }
    fail(format!("rules: line {line}: string without an end"))
}

fn value(s: &str, line: usize) -> Result<Val, RewriteError> {
    let s = s.trim();
    if s.starts_with('"') {
        let (v, rest) = unquote(s, line)?;
        return match rest.trim() {
            "" => Ok(Val::Str(v)),
            r if r.starts_with('#') => Ok(Val::Str(v)),
            _ => fail(format!("rules: line {line}: text after a string")),
        };
    }
    if let Some(mut r) = s.strip_prefix('[') {
        let mut l = Vec::new();
        loop {
            r = r.trim_start();
            if let Some(rest) = r.strip_prefix(']') {
                return match rest.trim() {
                    "" => Ok(Val::List(l)),
                    x if x.starts_with('#') => Ok(Val::List(l)),
                    _ => fail(format!("rules: line {line}: text after a list")),
                };
            }
            if !r.starts_with('"') {
                return fail(format!("rules: line {line}: lists hold strings"));
            }
            let (v, rest) = unquote(r, line)?;
            l.push(v);
            r = rest.trim_start();
            if let Some(rest) = r.strip_prefix(',') {
                r = rest;
            }
        }
    }
    let t = s.split('#').next().unwrap_or("").trim();
    match t {
        "true" => return Ok(Val::Bool(true)),
        "false" => return Ok(Val::Bool(false)),
        _ => {}
    }
    let n = t.replace('_', "");
    let v = if let Some(h) = n.strip_prefix("0x") {
        i64::from_str_radix(h, 16).ok()
    } else {
        n.parse::<i64>().ok()
    };
    v.map(Val::Int)
        .ok_or_else(|| RewriteError::Failed(format!("rules: line {line}: bad value {t:?}")))
}

/// Parses a file into its top-level table.
pub(crate) fn parse(text: &str) -> Result<Table, RewriteError> {
    let mut root = Table::default();
    let mut path: Vec<String> = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let line = n + 1;
        let t = raw.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if let Some(h) = t.strip_prefix("[[") {
            let h = h
                .split_once("]]")
                .ok_or_else(|| RewriteError::Failed(format!("rules: line {line}: bad header")))?
                .0;
            path = h.split('.').map(|p| p.trim().to_string()).collect();
            let mut cur = &mut root;
            for (i, p) in path.iter().enumerate() {
                let last = i + 1 == path.len();
                if cur.get(p).is_none() {
                    cur.0.push((p.clone(), Val::Tables(Vec::new())));
                }
                let Some(Val::Tables(ts)) = cur.0.iter_mut().find(|(k, _)| k == p).map(|(_, v)| v)
                else {
                    return fail(format!("rules: line {line}: {p:?} is not a table array"));
                };
                if last {
                    ts.push(Table::default());
                    break;
                }
                let Some(next) = ts.last_mut() else {
                    return fail(format!("rules: line {line}: header below nothing"));
                };
                cur = next;
            }
            continue;
        }
        let (k, v) = t
            .split_once('=')
            .ok_or_else(|| RewriteError::Failed(format!("rules: line {line}: no `=`")))?;
        let (k, v) = (k.trim().to_string(), value(v, line)?);
        let mut cur = &mut root;
        for p in &path {
            let Some(Val::Tables(ts)) = cur.0.iter_mut().find(|(n, _)| n == p).map(|(_, v)| v)
            else {
                return fail(format!("rules: line {line}: lost table path"));
            };
            let Some(next) = ts.last_mut() else {
                return fail(format!("rules: line {line}: lost table path"));
            };
            cur = next;
        }
        if cur.get(&k).is_some() {
            return fail(format!("rules: line {line}: key {k:?} twice"));
        }
        cur.0.push((k, v));
    }
    Ok(root)
}
