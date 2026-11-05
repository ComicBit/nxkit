//! A JSON parser to [`Value`] (what Python's `json.load` accepts for the chain's files: objects in order, ints and
//! floats told apart as Python does, `NaN`/`Infinity`).

use crate::value::Value;
use std::fmt;

/// Not valid JSON.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// Byte offset.
    pub at: usize,
    /// What was wrong.
    pub what: &'static str,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "JSON: {} at byte {}", self.what, self.at)
    }
}

impl std::error::Error for ParseError {}

struct P<'a> {
    s: &'a [u8],
    i: usize,
    depth: usize,
}

const MAX_DEPTH: usize = 256;

impl P<'_> {
    fn err(&self, what: &'static str) -> ParseError {
        ParseError { at: self.i, what }
    }

    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn lit(&mut self, w: &str, v: Value) -> Result<Value, ParseError> {
        if self.s[self.i..].starts_with(w.as_bytes()) {
            self.i += w.len();
            Ok(v)
        } else {
            Err(self.err("unknown literal"))
        }
    }

    fn value(&mut self) -> Result<Value, ParseError> {
        self.ws();
        let Some(&c) = self.s.get(self.i) else {
            return Err(self.err("unexpected end"));
        };
        match c {
            b'{' => self.object(),
            b'[' => self.list(),
            b'"' => self.string().map(Value::Str),
            b't' => self.lit("true", Value::Bool(true)),
            b'f' => self.lit("false", Value::Bool(false)),
            b'n' => self.lit("null", Value::Null),
            b'N' => self.lit("NaN", Value::Float(f64::NAN)),
            b'I' => self.lit("Infinity", Value::Float(f64::INFINITY)),
            b'-' if self.s[self.i..].starts_with(b"-Infinity") => {
                self.i += 9;
                Ok(Value::Float(f64::NEG_INFINITY))
            }
            b'-' | b'0'..=b'9' => self.number(),
            _ => Err(self.err("unexpected character")),
        }
    }

    fn enter(&mut self) -> Result<(), ParseError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.err("nested too deep"));
        }
        Ok(())
    }

    fn object(&mut self) -> Result<Value, ParseError> {
        self.enter()?;
        self.i += 1;
        let mut m = Vec::new();
        self.ws();
        if self.s.get(self.i) == Some(&b'}') {
            self.i += 1;
            self.depth -= 1;
            return Ok(Value::Object(m));
        }
        loop {
            self.ws();
            if self.s.get(self.i) != Some(&b'"') {
                return Err(self.err("expected a key"));
            }
            let k = self.string()?;
            self.ws();
            if self.s.get(self.i) != Some(&b':') {
                return Err(self.err("expected ':'"));
            }
            self.i += 1;
            let v = self.value()?;
            // Python keeps the last value of a repeated key, at the first key's place.
            match m.iter_mut().find(|(n, _): &&mut (String, Value)| *n == k) {
                Some(slot) => slot.1 = v,
                None => m.push((k, v)),
            }
            self.ws();
            match self.s.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    self.depth -= 1;
                    return Ok(Value::Object(m));
                }
                _ => return Err(self.err("expected ',' or '}'")),
            }
        }
    }

    fn list(&mut self) -> Result<Value, ParseError> {
        self.enter()?;
        self.i += 1;
        let mut l = Vec::new();
        self.ws();
        if self.s.get(self.i) == Some(&b']') {
            self.i += 1;
            self.depth -= 1;
            return Ok(Value::List(l));
        }
        loop {
            l.push(self.value()?);
            self.ws();
            match self.s.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    self.depth -= 1;
                    return Ok(Value::List(l));
                }
                _ => return Err(self.err("expected ',' or ']'")),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, ParseError> {
        let h = self
            .s
            .get(self.i..self.i + 4)
            .ok_or_else(|| self.err("short \\u escape"))?;
        let t = std::str::from_utf8(h).map_err(|_| self.err("bad \\u escape"))?;
        let v = u32::from_str_radix(t, 16).map_err(|_| self.err("bad \\u escape"))?;
        self.i += 4;
        Ok(v)
    }

    fn string(&mut self) -> Result<String, ParseError> {
        self.i += 1;
        let mut out = String::new();
        loop {
            let start = self.i;
            while self.i < self.s.len() && self.s[self.i] != b'"' && self.s[self.i] != b'\\' {
                self.i += 1;
            }
            out.push_str(
                std::str::from_utf8(&self.s[start..self.i]).map_err(|_| self.err("not UTF-8"))?,
            );
            match self.s.get(self.i) {
                Some(b'"') => {
                    self.i += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.i += 1;
                    let c = *self
                        .s
                        .get(self.i)
                        .ok_or_else(|| self.err("unexpected end"))?;
                    self.i += 1;
                    match c {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{08}'),
                        b'f' => out.push('\u{0c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let mut cp = self.hex4()?;
                            if (0xd800..0xdc00).contains(&cp)
                                && self.s[self.i..].starts_with(b"\\u")
                            {
                                self.i += 2;
                                let lo = self.hex4()?;
                                cp = 0x10000
                                    + ((cp - 0xd800) << 10)
                                    + (lo.wrapping_sub(0xdc00) & 0x3ff);
                            }
                            out.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
                        }
                        _ => return Err(self.err("bad escape")),
                    }
                }
                _ => return Err(self.err("unterminated string")),
            }
        }
    }

    fn number(&mut self) -> Result<Value, ParseError> {
        let start = self.i;
        let mut is_float = false;
        while let Some(&c) = self.s.get(self.i) {
            match c {
                b'0'..=b'9' | b'-' | b'+' => {}
                b'.' | b'e' | b'E' => is_float = true,
                _ => break,
            }
            self.i += 1;
        }
        let t = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| self.err("bad number"))?;
        if is_float {
            t.parse()
                .map(Value::Float)
                .map_err(|_| self.err("bad number"))
        } else {
            t.parse()
                .map(Value::Int)
                .map_err(|_| self.err("bad or too large integer"))
        }
    }
}

/// Parse a JSON document.
pub fn parse(text: &str) -> Result<Value, ParseError> {
    let mut p = P {
        s: text.as_bytes(),
        i: 0,
        depth: 0,
    };
    let v = p.value()?;
    p.ws();
    if p.i != p.s.len() {
        return Err(p.err("trailing data"));
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::write::{dumps, dumps_indent};

    #[test]
    fn round_trip() {
        let text = "{\n \"a\": [\n  1,\n  2.5,\n  -0.0,\n  \"x\\u00e9\\n\"\n ],\n \"b\": {},\n \"c\": null\n}";
        let v = parse(text).unwrap();
        assert_eq!(v.get("c"), Some(&Value::Null));
        assert_eq!(dumps_indent(&v, 1), text.replace("\\u00e9", "é"));
        assert_eq!(dumps(&parse("[1,2]").unwrap()), "[1, 2]");
        assert!(parse("[1,]").is_err());
        assert!(parse("{\"a\" 1}").is_err());
        assert!(parse(&"[".repeat(1000)).is_err());
    }
}
