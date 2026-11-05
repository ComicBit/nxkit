//! Wire format of the translator host (`dmsh_xlate_host`): one JSON object per line. Only the fields the bridge needs
//! are parsed, by a small reader (this crate has no JSON dependency).

/// A reply line of the host.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Reply {
    /// The echoed request id.
    pub id: Option<u64>,
    /// `error`: the request itself was bad.
    pub error: Option<String>,
    /// `red`: the `[RED] <key>: <msg>` lines of the request, in job order.
    pub red: Vec<String>,
}

/// A translate request line.
pub(crate) fn request(id: u64, jobs: &str, out: &str) -> String {
    format!(
        "{{\"id\": {id}, \"jobs\": {}, \"out\": {}}}\n",
        quote(jobs),
        quote(out)
    )
}

/// The ping request line.
pub(crate) fn ping(id: u64) -> String {
    format!("{{\"id\": {id}, \"op\": \"ping\"}}\n")
}

fn quote(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

impl P<'_> {
    fn ws(&mut self) {
        while self.b.get(self.i).is_some_and(u8::is_ascii_whitespace) {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: u8) -> Option<()> {
        self.ws();
        (self.b.get(self.i) == Some(&c)).then(|| self.i += 1)
    }
    fn string(&mut self) -> Option<String> {
        self.eat(b'"')?;
        let mut out = Vec::new();
        loop {
            let c = *self.b.get(self.i)?;
            self.i += 1;
            match c {
                b'"' => return String::from_utf8(out).ok(),
                b'\\' => {
                    let e = *self.b.get(self.i)?;
                    self.i += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b't' => out.push(b'\t'),
                        b'r' => out.push(b'\r'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'u' => {
                            let mut v = self.hex4()?;
                            if (0xD800..0xDC00).contains(&v)
                                && self.b.get(self.i..self.i + 2) == Some(b"\\u")
                            {
                                self.i += 2;
                                let lo = self.hex4()?;
                                v = 0x10000 + ((v - 0xD800) << 10) + lo.wrapping_sub(0xDC00);
                            }
                            let ch = char::from_u32(v).unwrap_or('\u{fffd}');
                            out.extend_from_slice(ch.to_string().as_bytes());
                        }
                        o => out.push(o),
                    }
                }
                c => out.push(c),
            }
        }
    }
    fn hex4(&mut self) -> Option<u32> {
        let s = std::str::from_utf8(self.b.get(self.i..self.i + 4)?).ok()?;
        self.i += 4;
        u32::from_str_radix(s, 16).ok()
    }
    /// Skip any value, returning its text when it is a scalar token.
    fn value(&mut self) -> Option<Option<String>> {
        self.ws();
        match self.b.get(self.i)? {
            b'"' => Some(Some(self.string()?)),
            b'[' | b'{' => {
                let mut depth = 0;
                loop {
                    match *self.b.get(self.i)? {
                        b'"' => {
                            self.string()?;
                            continue;
                        }
                        b'[' | b'{' => depth += 1,
                        b']' | b'}' => depth -= 1,
                        _ => {}
                    }
                    self.i += 1;
                    if depth == 0 {
                        return Some(None);
                    }
                }
            }
            _ => {
                let s = self.i;
                while self
                    .b
                    .get(self.i)
                    .is_some_and(|c| !b",}]".contains(c) && !c.is_ascii_whitespace())
                {
                    self.i += 1;
                }
                Some(Some(
                    String::from_utf8_lossy(&self.b[s..self.i]).into_owned(),
                ))
            }
        }
    }
}

/// Parse one reply line.`None` when it is not a JSON object.
pub(crate) fn parse_reply(line: &str) -> Option<Reply> {
    let mut p = P {
        b: line.as_bytes(),
        i: 0,
    };
    p.eat(b'{')?;
    let mut r = Reply::default();
    p.ws();
    if p.eat(b'}').is_some() {
        return Some(r);
    }
    loop {
        let k = p.string()?;
        p.eat(b':')?;
        match k.as_str() {
            "red" => {
                p.eat(b'[')?;
                if p.eat(b']').is_none() {
                    loop {
                        r.red.push(p.string()?);
                        if p.eat(b',').is_none() {
                            p.eat(b']')?;
                            break;
                        }
                    }
                }
            }
            "id" => r.id = p.value()?.and_then(|s| s.parse().ok()),
            "error" => r.error = p.value()?,
            _ => {
                p.value()?;
            }
        }
        if p.eat(b',').is_some() {
            continue;
        }
        p.eat(b'}')?;
        return Some(r);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies() {
        let r = parse_reply(
            r#"{"id": 17, "exit": 1, "ok": 127, "failed": 1, "red": ["[RED] 0a: Inv: \"x\" é\\"], "ms": 812.4}"#,
        )
        .unwrap();
        assert_eq!(r.id, Some(17));
        assert_eq!(r.red, ["[RED] 0a: Inv: \"x\" \u{e9}\\"]);
        let e = parse_reply(r#"{"id": null, "error": "bad request: x"}"#).unwrap();
        assert_eq!((e.id, e.error.as_deref()), (None, Some("bad request: x")));
        assert!(parse_reply("nonsense").is_none());
        assert_eq!(
            parse_reply(r#"{"id": 3, "ok": true, "pid": 4, "threads": 12}"#)
                .unwrap()
                .id,
            Some(3)
        );
        assert_eq!(
            request(1, "/a\"b", "/o"),
            "{\"id\": 1, \"jobs\": \"/a\\\"b\", \"out\": \"/o\"}\n"
        );
    }
}
