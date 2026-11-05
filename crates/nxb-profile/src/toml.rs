//! The TOML subset a profile uses: tables (`[a.b]`, dotted keys), strings, integers, floats, booleans, arrays and inline
//! tables, with comments. Not supported (and refused with a line number): arrays of tables `[[x]]`, multi-line strings,
//! dates. Writing is deterministic: keys sorted, scalars before tables.

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// A table: sorted keys.
pub type Table = BTreeMap<String, Value>;

/// A TOML value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A string.
    Str(String),
    /// An integer.
    Int(i64),
    /// A float.
    Float(f64),
    /// A boolean.
    Bool(bool),
    /// An array.
    Array(Vec<Value>),
    /// A table.
    Table(Table),
}

/// A parse error with its line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// 1-based line.
    pub line: usize,
    /// What is wrong.
    pub msg: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.msg)
    }
}

impl std::error::Error for ParseError {}

struct P<'a> {
    s: &'a [u8],
    i: usize,
    line: usize,
}

type R<T> = Result<T, ParseError>;

impl P<'_> {
    fn err<T>(&self, msg: impl Into<String>) -> R<T> {
        Err(ParseError {
            line: self.line,
            msg: msg.into(),
        })
    }
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    fn spaces(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t')) {
            self.i += 1;
        }
    }
    /// Spaces, comments and line breaks.
    fn blank(&mut self) {
        loop {
            self.spaces();
            match self.peek() {
                Some(b'#') => {
                    while !matches!(self.peek(), None | Some(b'\n')) {
                        self.i += 1;
                    }
                }
                Some(b'\r') => self.i += 1,
                Some(b'\n') => {
                    self.i += 1;
                    self.line += 1;
                }
                _ => return,
            }
        }
    }
    fn end_of_line(&mut self) -> R<()> {
        self.spaces();
        if self.peek() == Some(b'#') {
            self.blank();
            return Ok(());
        }
        match self.peek() {
            None => Ok(()),
            Some(b'\r' | b'\n') => {
                self.blank();
                Ok(())
            }
            Some(_) => self.err("unexpected text after a value"),
        }
    }

    fn key(&mut self) -> R<String> {
        self.spaces();
        match self.peek() {
            Some(b'"') => self.string(),
            Some(b'\'') => self.literal(),
            _ => {
                let st = self.i;
                while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
                {
                    self.i += 1;
                }
                if st == self.i {
                    return self.err("a key is expected");
                }
                Ok(String::from_utf8_lossy(&self.s[st..self.i]).into_owned())
            }
        }
    }

    fn key_path(&mut self) -> R<Vec<String>> {
        let mut p = vec![self.key()?];
        loop {
            self.spaces();
            if self.peek() == Some(b'.') {
                self.i += 1;
                p.push(self.key()?);
            } else {
                return Ok(p);
            }
        }
    }

    fn literal(&mut self) -> R<String> {
        self.i += 1;
        let st = self.i;
        while !matches!(self.peek(), None | Some(b'\'' | b'\n')) {
            self.i += 1;
        }
        if self.peek() != Some(b'\'') {
            return self.err("a string is not closed");
        }
        let v = String::from_utf8_lossy(&self.s[st..self.i]).into_owned();
        self.i += 1;
        Ok(v)
    }

    fn string(&mut self) -> R<String> {
        self.i += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            match self.peek() {
                None | Some(b'\n') => return self.err("a string is not closed"),
                Some(b'"') => {
                    self.i += 1;
                    return String::from_utf8(out).or_else(|_| self.err("a string is not UTF-8"));
                }
                Some(b'\\') => {
                    self.i += 1;
                    let c = self.peek();
                    self.i += 1;
                    match c {
                        Some(b'"') => out.push(b'"'),
                        Some(b'\\') => out.push(b'\\'),
                        Some(b'n') => out.push(b'\n'),
                        Some(b't') => out.push(b'\t'),
                        Some(b'r') => out.push(b'\r'),
                        Some(b'u') => {
                            let hex = self.s.get(self.i..self.i + 4).unwrap_or(&[]);
                            let n = std::str::from_utf8(hex)
                                .ok()
                                .and_then(|h| u32::from_str_radix(h, 16).ok());
                            let Some(ch) = n.and_then(char::from_u32) else {
                                return self.err("bad \\u escape");
                            };
                            self.i += 4;
                            let mut b = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut b).as_bytes());
                        }
                        _ => return self.err("unknown escape"),
                    }
                }
                Some(c) => {
                    out.push(c);
                    self.i += 1;
                }
            }
        }
    }

    fn value(&mut self) -> R<Value> {
        self.spaces();
        match self.peek() {
            Some(b'"') => self.string().map(Value::Str),
            Some(b'\'') => self.literal().map(Value::Str),
            Some(b'[') => {
                self.i += 1;
                let mut v = Vec::new();
                loop {
                    self.blank();
                    if self.peek() == Some(b']') {
                        self.i += 1;
                        return Ok(Value::Array(v));
                    }
                    v.push(self.value()?);
                    self.blank();
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b']') => {}
                        _ => return self.err("`,` or `]` is expected in an array"),
                    }
                }
            }
            Some(b'{') => {
                self.i += 1;
                let mut t = Table::new();
                self.spaces();
                if self.peek() == Some(b'}') {
                    self.i += 1;
                    return Ok(Value::Table(t));
                }
                loop {
                    let path = self.key_path()?;
                    self.spaces();
                    if self.peek() != Some(b'=') {
                        return self.err("`=` is expected");
                    }
                    self.i += 1;
                    let v = self.value()?;
                    insert(&mut t, &path, v).or_else(|m| self.err(m))?;
                    self.spaces();
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(Value::Table(t));
                        }
                        _ => return self.err("`,` or `}` is expected in an inline table"),
                    }
                }
            }
            Some(_) => {
                let st = self.i;
                while !matches!(
                    self.peek(),
                    None | Some(b',' | b']' | b'}' | b'#' | b'\n' | b'\r' | b' ' | b'\t')
                ) {
                    self.i += 1;
                }
                let w = String::from_utf8_lossy(&self.s[st..self.i]).into_owned();
                match w.as_str() {
                    "true" => return Ok(Value::Bool(true)),
                    "false" => return Ok(Value::Bool(false)),
                    _ => {}
                }
                let n = w.replace('_', "");
                if let Ok(i) = n.parse::<i64>() {
                    Ok(Value::Int(i))
                } else if let (Ok(f), true) =
                    (n.parse::<f64>(), n.contains(|c: char| c.is_ascii_digit()))
                {
                    Ok(Value::Float(f))
                } else {
                    self.err(format!("{w:?} is not a value"))
                }
            }
            None => self.err("a value is expected"),
        }
    }
}

fn insert(t: &mut Table, path: &[String], v: Value) -> Result<(), String> {
    let Some((first, rest)) = path.split_first() else {
        return Err("empty key".into());
    };
    if rest.is_empty() {
        return match t.insert(first.clone(), v) {
            None => Ok(()),
            Some(_) => Err(format!("key {first:?} is set twice")),
        };
    }
    match t
        .entry(first.clone())
        .or_insert_with(|| Value::Table(Table::new()))
    {
        Value::Table(sub) => insert(sub, rest, v),
        _ => Err(format!("{first:?} is not a table")),
    }
}

fn table_at<'t>(root: &'t mut Table, path: &[String]) -> Result<&'t mut Table, String> {
    let mut cur = root;
    for p in path {
        cur = match cur
            .entry(p.clone())
            .or_insert_with(|| Value::Table(Table::new()))
        {
            Value::Table(t) => t,
            _ => return Err(format!("{p:?} is not a table")),
        };
    }
    Ok(cur)
}

/// Parse a document into its root table.
pub fn parse(text: &str) -> Result<Table, ParseError> {
    let mut p = P {
        s: text.as_bytes(),
        i: 0,
        line: 1,
    };
    let mut root = Table::new();
    let mut at: Vec<String> = Vec::new();
    let mut headers = std::collections::BTreeSet::new();
    loop {
        p.blank();
        match p.peek() {
            None => return Ok(root),
            Some(b'[') => {
                p.i += 1;
                if p.peek() == Some(b'[') {
                    return p.err("arrays of tables are not supported");
                }
                at = p.key_path()?;
                p.spaces();
                if p.peek() != Some(b']') {
                    return p.err("`]` is expected");
                }
                p.i += 1;
                if !headers.insert(at.clone()) {
                    return p.err("table is defined twice");
                }
                table_at(&mut root, &at).or_else(|m| p.err(m))?;
                p.end_of_line()?;
            }
            Some(_) => {
                let path = p.key_path()?;
                p.spaces();
                if p.peek() != Some(b'=') {
                    return p.err("`=` is expected after a key");
                }
                p.i += 1;
                let v = p.value()?;
                let line = p.line;
                let t = table_at(&mut root, &at).or_else(|m| p.err(m))?;
                insert(t, &path, v).map_err(|msg| ParseError { line, msg })?;
                p.end_of_line()?;
            }
        }
    }
}

fn key_text(k: &str) -> String {
    if !k.is_empty()
        && k.bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        k.to_string()
    } else {
        quote(k)
    }
}

fn quote(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\t' => o.push_str("\\t"),
            '\r' => o.push_str("\\r"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn inline(v: &Value) -> String {
    match v {
        Value::Str(s) => quote(s),
        Value::Int(i) => i.to_string(),
        Value::Float(f) if f.is_finite() => {
            let t = format!("{f:?}");
            if t.contains(['.', 'e', 'E']) {
                t
            } else {
                format!("{t}.0")
            }
        }
        Value::Float(_) => "0.0".into(),
        Value::Bool(b) => b.to_string(),
        Value::Array(a) => format!("[{}]", a.iter().map(inline).collect::<Vec<_>>().join(", ")),
        Value::Table(t) if t.is_empty() => "{}".into(),
        Value::Table(t) => format!(
            "{{ {} }}",
            t.iter()
                .map(|(k, v)| format!("{} = {}", key_text(k), inline(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn emit(out: &mut String, path: &str, t: &Table) {
    let mut subs = Vec::new();
    for (k, v) in t {
        match v {
            Value::Table(sub) if !sub.is_empty() => subs.push((k, sub)),
            Value::Array(a) if a.iter().any(|x| matches!(x, Value::Table(_))) => {
                let _ = writeln!(out, "{} = [", key_text(k));
                for x in a {
                    let _ = writeln!(out, "    {},", inline(x));
                }
                out.push_str("]\n");
            }
            _ => {
                let _ = writeln!(out, "{} = {}", key_text(k), inline(v));
            }
        }
    }
    for (k, sub) in subs {
        let p = if path.is_empty() {
            key_text(k)
        } else {
            format!("{path}.{}", key_text(k))
        };
        let _ = write!(out, "\n[{p}]\n");
        emit(out, &p, sub);
    }
}

/// Write a root table as a document. `parse(&write(t)) == t`.
pub fn write(root: &Table) -> String {
    let mut out = String::new();
    emit(&mut out, "", root);
    out.trim_start_matches('\n').to_string()
}
