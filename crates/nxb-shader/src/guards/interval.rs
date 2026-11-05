//! Interval arithmetic over the integer expressions of one shader's MSL (port of `c1_guard.Interval`, `tokens`,
//! `definitions_of`). Python's unbounded integers are `i128` here, an overflow reads as unbounded, which leads to the
//! same verdict (not proven) everywhere the bounds are used.

use std::collections::BTreeMap;

/// A bound pair.`None` is unbounded on that side.
pub type Bounds = (Option<i128>, Option<i128>);
const UNB: Bounds = (None, None);

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn word_end(s: &str, i: usize) -> usize {
    s[i..]
        .char_indices()
        .find(|(_, c)| !is_word(*c))
        .map_or(s.len(), |(k, _)| i + k)
}

fn digits_end(s: &str, i: usize) -> usize {
    i + s[i..].bytes().take_while(u8::is_ascii_digit).count()
}

fn prev_is_word(s: &str, i: usize) -> bool {
    s[..i].chars().next_back().is_some_and(is_word)
}

/// `re.sub(r"\bas_type<u(?:int|short|long)>\s*\(", "as_type_u(", text)` then `re.sub(r"\bas_type<\w+>\s*\(", "as_type(", ...)`.
fn rewrite_casts(text: &str) -> String {
    let pass = |t: &str, unsigned: bool| -> String {
        let mut out = String::with_capacity(t.len());
        let mut p = 0;
        while let Some(off) = t[p..].find("as_type<") {
            let i = p + off;
            let a = i + "as_type<".len();
            let e = word_end(t, a);
            let name = &t[a..e];
            let ok_name = if unsigned {
                ["uint", "ushort", "ulong"].contains(&name)
            } else {
                !name.is_empty()
            };
            let after = t[e..].strip_prefix('>').map(|r| {
                let r2 = r.trim_start_matches(|c: char| c.is_whitespace());
                (r2.starts_with('('), t.len() - r2.len())
            });
            match after {
                Some((true, paren)) if ok_name && !prev_is_word(t, i) => {
                    out.push_str(&t[p..i]);
                    out.push_str(if unsigned { "as_type_u(" } else { "as_type(" });
                    p = paren + 1;
                }
                _ => {
                    out.push_str(&t[p..a]);
                    p = a;
                }
            }
        }
        out.push_str(&t[p..]);
        out
    };
    pass(&pass(text, true), false)
}

/// The `TOKEN` alternatives, first that matches.
fn token_at(t: &str, i: usize) -> Option<usize> {
    let b = t.as_bytes();
    let c = *b.get(i)?;
    // 0[xX][0-9a-fA-F]+[uU]?
    if c == b'0' && matches!(b.get(i + 1), Some(b'x' | b'X')) {
        let h = i + 2 + t[i + 2..].bytes().take_while(u8::is_ascii_hexdigit).count();
        if h > i + 2 {
            return Some(h + usize::from(matches!(b.get(h), Some(b'u' | b'U'))));
        }
    }
    if c.is_ascii_digit() {
        let d = digits_end(t, i);
        // \d+\.\d*(?:[eE][-+]?\d+)?f?
        if b.get(d) == Some(&b'.') {
            let mut e = digits_end(t, d + 1);
            if matches!(b.get(e), Some(b'e' | b'E')) {
                let s = e + 1 + usize::from(matches!(b.get(e + 1), Some(b'-' | b'+')));
                let x = digits_end(t, s);
                if x > s {
                    e = x;
                }
            }
            return Some(e + usize::from(b.get(e) == Some(&b'f')));
        }
        // \d+[uU]?
        return Some(d + usize::from(matches!(b.get(d), Some(b'u' | b'U'))));
    }
    let ch = t[i..].chars().next()?;
    // [A-Za-z_]\w*(?:\.\w+)*
    if c.is_ascii_alphabetic() || c == b'_' {
        let mut e = word_end(t, i);
        while b.get(e) == Some(&b'.') {
            let f = word_end(t, e + 1);
            if f == e + 1 {
                break;
            }
            e = f;
        }
        return Some(e);
    }
    // \.\w+
    if c == b'.' {
        let f = word_end(t, i + 1);
        if f > i + 1 {
            return Some(f);
        }
    }
    for op in ["<<", ">>", "<=", ">=", "==", "!=", "&&", "||"] {
        if t[i..].starts_with(op) {
            return Some(i + 2);
        }
    }
    "-+*/%&|^~!?:(),<>[]".contains(ch).then_some(i + 1)
}

/// `tokens(text)`.`None` where Python raises `ValueError`.
fn tokens(text: &str) -> Option<Vec<String>> {
    let t = rewrite_casts(text);
    let mut out = Vec::new();
    let mut i = 0;
    while i < t.len() {
        let j = i + (t[i..].len() - t[i..].trim_start().len());
        if j >= t.len() {
            break;
        }
        let e = token_at(&t, j)?;
        out.push(t[j..e].to_string());
        i = e;
    }
    Some(out)
}

fn known(v: &[Option<i128>]) -> bool {
    v.iter().all(Option::is_some)
}

fn prec(op: &str) -> Option<u8> {
    Some(match op {
        "||" => 1,
        "&&" => 2,
        "|" => 3,
        "^" => 4,
        "&" => 5,
        "==" | "!=" => 6,
        "<" | ">" | "<=" | ">=" => 7,
        "<<" | ">>" => 8,
        "+" | "-" => 9,
        "*" | "/" | "%" => 10,
        _ => return None,
    })
}

pub use super::defs::{Definitions, definitions_of};

/// A Python `ValueError`/`IndexError` inside an expression: the whole expression is unbounded.
struct Fail;

/// One `Interval` evaluator (fresh per index, as in Python).
pub struct Interval<'a> {
    defs: &'a Definitions,
    memo: BTreeMap<String, Bounds>,
    depth: u32,
    toks: Vec<String>,
    pos: usize,
    stack: u32,
}

impl<'a> Interval<'a> {
    /// A fresh evaluator.
    pub fn new(defs: &'a Definitions) -> Self {
        Interval {
            defs,
            memo: BTreeMap::new(),
            depth: 0,
            toks: Vec::new(),
            pos: 0,
            stack: 0,
        }
    }

    fn variable(&mut self, name: &str) -> Bounds {
        if let Some(v) = self.memo.get(name) {
            return *v;
        }
        let Some((kind, text)) = self.defs.get(name) else {
            return UNB;
        };
        if self.depth > 32 {
            return UNB;
        }
        self.memo.insert(name.to_string(), UNB);
        self.depth += 1;
        let mut value = self.expression(&text.clone());
        self.depth -= 1;
        if kind == "bool" {
            value = (Some(0), Some(1));
        } else if kind.starts_with('u') && !value.0.is_some_and(|l| l >= 0) {
            value = (Some(0), None);
        }
        self.memo.insert(name.to_string(), value);
        value
    }

    /// `expression(text)`.
    pub fn expression(&mut self, text: &str) -> Bounds {
        let Some(t) = tokens(text) else {
            return UNB;
        };
        self.toks = t;
        self.pos = 0;
        match self.ternary() {
            Ok(v) if self.pos == self.toks.len() => v,
            _ => UNB,
        }
    }

    fn peek(&self) -> Option<&str> {
        self.toks.get(self.pos).map(String::as_str)
    }

    fn take(&mut self, want: Option<&str>) -> Result<String, Fail> {
        let t = self.toks.get(self.pos).cloned().ok_or(Fail)?;
        if want.is_some_and(|w| w != t) {
            return Err(Fail);
        }
        self.pos += 1;
        Ok(t)
    }

    fn guard(&mut self) -> Result<(), Fail> {
        // Python's recursion limit raises RecursionError, which reads as unbounded.
        self.stack += 1;
        if self.stack > 400 { Err(Fail) } else { Ok(()) }
    }

    fn ternary(&mut self) -> Result<Bounds, Fail> {
        self.guard()?;
        let r = (|| {
            let cond = self.binary(1)?;
            if self.peek() != Some("?") {
                return Ok(cond);
            }
            self.take(Some("?"))?;
            let a = self.ternary()?;
            self.take(Some(":"))?;
            let b = self.ternary()?;
            Ok((
                if known(&[a.0, b.0]) {
                    a.0.min(b.0)
                } else {
                    None
                },
                if known(&[a.1, b.1]) {
                    a.1.max(b.1)
                } else {
                    None
                },
            ))
        })();
        self.stack -= 1;
        r
    }

    fn binary(&mut self, level: u8) -> Result<Bounds, Fail> {
        let mut left = self.unary()?;
        while let Some(p) = self.peek().and_then(prec) {
            if p < level {
                break;
            }
            let op = self.take(None)?;
            let right = self.binary(p + 1)?;
            left = combine(&op, left, right);
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Bounds, Fail> {
        self.guard()?;
        let r = (|| match self.peek() {
            Some("-") => {
                self.take(None)?;
                let (l, h) = self.unary()?;
                Ok((
                    h.and_then(|h| h.checked_neg()),
                    l.and_then(|l| l.checked_neg()),
                ))
            }
            Some("+") => {
                self.take(None)?;
                self.unary()
            }
            Some("!") => {
                self.take(None)?;
                self.unary()?;
                Ok((Some(0), Some(1)))
            }
            Some("~" | "*") => {
                self.take(None)?;
                self.unary()?;
                Ok(UNB)
            }
            _ => {
                let v = self.primary()?;
                self.postfix(v)
            }
        })();
        self.stack -= 1;
        r
    }

    fn postfix(&mut self, mut v: Bounds) -> Result<Bounds, Fail> {
        while let Some(t) = self.peek() {
            if t != "[" && !t.starts_with('.') {
                break;
            }
            if self.take(None)? == "[" {
                self.ternary()?;
                self.take(Some("]"))?;
            }
            v = UNB;
        }
        Ok(v)
    }

    fn primary(&mut self) -> Result<Bounds, Fail> {
        let t = self.take(None)?;
        if t == "(" {
            let v = self.ternary()?;
            self.take(Some(")"))?;
            return Ok(v);
        }
        if let Some(v) = int_literal(&t)? {
            return Ok((Some(v), Some(v)));
        }
        let b = t.as_bytes();
        if b[0].is_ascii_digit() {
            return Ok(UNB); // a float literal
        }
        let ident =
            (b[0].is_ascii_alphabetic() || b[0] == b'_') && token_at(&t, 0) == Some(t.len());
        if !ident {
            return Err(Fail);
        }
        if self.peek() == Some("(") {
            self.take(Some("("))?;
            let mut args = Vec::new();
            if self.peek() != Some(")") {
                args.push(self.ternary()?);
                while self.peek() == Some(",") {
                    self.take(Some(","))?;
                    args.push(self.ternary()?);
                }
            }
            self.take(Some(")"))?;
            return Ok(call(&t, &args));
        }
        if t.len() > 1 && t.starts_with('_') && t[1..].bytes().all(|c| c.is_ascii_digit()) {
            let saved = (std::mem::take(&mut self.toks), self.pos);
            let v = self.variable(&t);
            (self.toks, self.pos) = saved;
            return Ok(v);
        }
        if t == "true" || t == "false" {
            return Ok((Some(0), Some(1)));
        }
        Ok(UNB)
    }
}

/// `re.fullmatch(r"0[xX][0-9a-fA-F]+[uU]?|\d+[uU]?", t)` then `int(t.rstrip("uU"), 0)`.`Err` where `int` raises
/// (a decimal with a leading zero).
fn int_literal(t: &str) -> Result<Option<i128>, Fail> {
    let s = t.trim_end_matches(['u', 'U']);
    if t.len() - s.len() > 1 {
        return Ok(None);
    }
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        if !h.is_empty() && h.bytes().all(|c| c.is_ascii_hexdigit()) {
            return i128::from_str_radix(h, 16).map(Some).map_err(|_| Fail);
        }
        return Ok(None);
    }
    if s.is_empty() || !s.bytes().all(|c| c.is_ascii_digit()) {
        return Ok(None);
    }
    // int('017', 0) raises, int('00', 0) is 0.
    if s.len() > 1 && s.starts_with('0') && s.bytes().any(|c| c != b'0') {
        return Err(Fail);
    }
    s.parse().map(Some).map_err(|_| Fail)
}

fn call(name: &str, args: &[Bounds]) -> Bounds {
    match (name, args) {
        ("as_type_u", [_]) => (Some(0), None),
        ("int" | "uint" | "short" | "ushort" | "long" | "ulong", [(l, h)]) => {
            if name.starts_with('u') && !l.is_some_and(|l| l >= 0) {
                (Some(0), None)
            } else {
                (*l, *h)
            }
        }
        ("min", [(al, ah), (bl, bh)]) => {
            let hs: Vec<i128> = [*ah, *bh].into_iter().flatten().collect();
            (
                if known(&[*al, *bl]) {
                    (*al).min(*bl)
                } else {
                    None
                },
                hs.into_iter().min(),
            )
        }
        ("max", [(al, ah), (bl, bh)]) => {
            let ls: Vec<i128> = [*al, *bl].into_iter().flatten().collect();
            (
                ls.into_iter().max(),
                if known(&[*ah, *bh]) {
                    (*ah).max(*bh)
                } else {
                    None
                },
            )
        }
        ("clamp", [_, lo, hi]) => (lo.0, hi.1),
        _ => UNB,
    }
}

fn combine(op: &str, a: Bounds, b: Bounds) -> Bounds {
    let ((al, ah), (bl, bh)) = (a, b);
    let add = |x: Option<i128>, y: Option<i128>| x.zip(y).and_then(|(x, y)| x.checked_add(y));
    let sub = |x: Option<i128>, y: Option<i128>| x.zip(y).and_then(|(x, y)| x.checked_sub(y));
    match op {
        "||" | "&&" | "==" | "!=" | "<" | ">" | "<=" | ">=" => (Some(0), Some(1)),
        "+" => (add(al, bl), add(ah, bh)),
        "-" => (sub(al, bh), sub(ah, bl)),
        "*" => {
            let (Some(al), Some(ah), Some(bl), Some(bh)) = (al, ah, bl, bh) else {
                return UNB;
            };
            let p: Option<Vec<i128>> = [al, al, ah, ah]
                .iter()
                .zip([bl, bh, bl, bh])
                .map(|(x, y)| x.checked_mul(y))
                .collect();
            match p {
                Some(p) => (p.iter().min().copied(), p.iter().max().copied()),
                None => UNB,
            }
        }
        "/" | ">>" | "<<" => {
            let (Some(al), Some(ah), Some(bl), Some(bh)) = (al, ah, bl, bh) else {
                return UNB;
            };
            if bl != bh || bl < 0 || (op == "/" && bl == 0) {
                return UNB;
            }
            match op {
                "<<" => {
                    let sh = u32::try_from(bl).ok().filter(|s| *s < 100);
                    match sh {
                        Some(s)
                            if al.checked_shl(s).is_some_and(|v| v >> s == al)
                                && ah.checked_shl(s).is_some_and(|v| v >> s == ah) =>
                        {
                            (Some(al << s), Some(ah << s))
                        }
                        _ => UNB,
                    }
                }
                ">>" => {
                    let s = u32::try_from(bl.min(127)).unwrap_or(127);
                    (Some(al >> s), Some(ah >> s))
                }
                _ if al >= 0 => (Some(al / bl), Some(ah.div_euclid(bl))),
                _ => UNB,
            }
        }
        "%" => {
            let (Some(bl), Some(bh)) = (bl, bh) else {
                return UNB;
            };
            if bl != bh || bl <= 0 {
                return UNB;
            }
            if al.is_some_and(|l| l >= 0) {
                (Some(0), Some(bl - 1))
            } else {
                (Some(-(bl - 1)), Some(bl - 1))
            }
        }
        "&" => {
            let caps: Vec<i128> = [a, b]
                .into_iter()
                .filter_map(|(l, h)| match (l, h) {
                    (Some(l), Some(h)) if l >= 0 => Some(h),
                    _ => None,
                })
                .collect();
            caps.into_iter().min().map_or(UNB, |c| (Some(0), Some(c)))
        }
        "|" | "^" => {
            let (Some(al), Some(ah), Some(bl), Some(bh)) = (al, ah, bl, bh) else {
                return UNB;
            };
            if al < 0 || bl < 0 {
                return UNB;
            }
            let m = ah.max(bh);
            let bits = if m <= 0 { 0 } else { 128 - m.leading_zeros() };
            if bits >= 127 {
                return UNB;
            }
            (Some(0), Some((1i128 << bits) - 1))
        }
        _ => UNB,
    }
}
