//! A small XML property list reader and a writer with CoreFoundation's exact layout.
//!
//! The reader accepts what Info.plist and entitlement files contain in practice: `dict`,
//! `array`, `string`, `integer`, `real`, `true`, `false`, `data`, `date`, comments, the XML
//! declaration and the DOCTYPE. Binary plists are not read (an [`crate::Error::Unsupported`]).
//!
//! The writer produces the layout of `CFPropertyListCreateData(..., XML)`: tab indentation,
//! dictionary keys sorted by UTF-16 code units, `<data>` on three lines, `<true/>`, and
//! `&`, `<`, `>` escaped. `CodeResources` is written with it.

use crate::error::{Error, Result, format_err};

mod binary;
pub use binary::parse_binary;

/// A property list value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// `<dict>`: entries in file order (the writer sorts them).
    Dict(Vec<(String, Value)>),
    /// `<array>`.
    Array(Vec<Value>),
    /// `<string>`.
    String(String),
    /// `<integer>`.
    Integer(i64),
    /// `<real>`.
    Real(f64),
    /// `<true/>` or `<false/>`.
    Bool(bool),
    /// `<data>` (decoded).
    Data(Vec<u8>),
    /// `<date>` (kept as text).
    Date(String),
}

impl Value {
    /// The value of `key` when `self` is a dictionary.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Dict(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The string, when `self` is a `<string>`.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }
}

/// Parses an XML property list.
pub fn parse(bytes: &[u8]) -> Result<Value> {
    if bytes.starts_with(b"bplist00") {
        return parse_binary(bytes);
    }
    if bytes.starts_with(b"bplist") {
        return Err(Error::Unsupported("binary property list version".into()));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| format_err("plist is not UTF-8"))?;
    let mut p = Parser { s: text, pos: 0 };
    p.skip_misc()?;
    let tag = p.open_tag()?;
    let value = if tag.name == "plist" {
        if tag.empty {
            return Err(format_err("empty <plist/>"));
        }
        p.skip_misc()?;
        let v = p.value()?;
        p.skip_misc()?;
        p.close_tag("plist")?;
        v
    } else {
        p.value_from(tag)?
    };
    p.skip_misc()?;
    if p.pos != p.s.len() {
        return Err(format_err("trailing content after the plist"));
    }
    Ok(value)
}

struct Tag<'a> {
    name: &'a str,
    empty: bool,
}

struct Parser<'a> {
    s: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn rest(&self) -> &'a str {
        &self.s[self.pos..]
    }

    fn skip_ws(&mut self) {
        let r = self.rest();
        self.pos += r.len() - r.trim_start().len();
    }

    /// Skips white space, comments, `<?...?>` and `<!DOCTYPE ...>`.
    fn skip_misc(&mut self) -> Result<()> {
        loop {
            self.skip_ws();
            let r = self.rest();
            let end = if r.starts_with("<?") {
                r.find("?>").map(|i| i + 2)
            } else if r.starts_with("<!--") {
                r.find("-->").map(|i| i + 3)
            } else if r.starts_with("<!") {
                r.find('>').map(|i| i + 1)
            } else {
                return Ok(());
            };
            self.pos += end.ok_or_else(|| format_err("unterminated markup"))?;
        }
    }

    fn open_tag(&mut self) -> Result<Tag<'a>> {
        let r = self.rest();
        if !r.starts_with('<') || r.starts_with("</") {
            return Err(format_err(format!("expected a tag at byte {}", self.pos)));
        }
        let end = r.find('>').ok_or_else(|| format_err("unterminated tag"))?;
        let inner = &r[1..end];
        let empty = inner.ends_with('/');
        let inner = inner.trim_end_matches('/');
        let name = inner
            .split(|c: char| c.is_whitespace())
            .next()
            .unwrap_or("");
        self.pos += end + 1;
        Ok(Tag { name, empty })
    }

    fn close_tag(&mut self, name: &str) -> Result<()> {
        self.skip_ws();
        let r = self.rest();
        let want = format!("</{name}");
        if !r.starts_with(&want) {
            return Err(format_err(format!(
                "expected </{name}> at byte {}",
                self.pos
            )));
        }
        let end = r
            .find('>')
            .ok_or_else(|| format_err("unterminated close tag"))?;
        self.pos += end + 1;
        Ok(())
    }

    /// Raw text up to the next `<`, entity-decoded.
    fn text(&mut self) -> Result<String> {
        let r = self.rest();
        let end = r.find('<').ok_or_else(|| format_err("unterminated text"))?;
        self.pos += end;
        unescape(&r[..end])
    }

    fn value(&mut self) -> Result<Value> {
        self.skip_misc()?;
        let tag = self.open_tag()?;
        self.value_from(tag)
    }

    fn value_from(&mut self, tag: Tag<'a>) -> Result<Value> {
        let name = tag.name;
        if tag.empty {
            return Ok(match name {
                "true" => Value::Bool(true),
                "false" => Value::Bool(false),
                "dict" => Value::Dict(Vec::new()),
                "array" => Value::Array(Vec::new()),
                "string" => Value::String(String::new()),
                "data" => Value::Data(Vec::new()),
                _ => return Err(format_err(format!("unexpected empty <{name}/>"))),
            });
        }
        let v = match name {
            "dict" => {
                let mut entries = Vec::new();
                loop {
                    self.skip_misc()?;
                    if self.rest().starts_with("</") {
                        break;
                    }
                    let k = self.open_tag()?;
                    if k.name != "key" {
                        return Err(format_err(format!("expected <key>, found <{}>", k.name)));
                    }
                    let key = if k.empty {
                        String::new()
                    } else {
                        let t = self.text()?;
                        self.close_tag("key")?;
                        t
                    };
                    let v = self.value()?;
                    entries.push((key, v));
                }
                Value::Dict(entries)
            }
            "array" => {
                let mut items = Vec::new();
                loop {
                    self.skip_misc()?;
                    if self.rest().starts_with("</") {
                        break;
                    }
                    items.push(self.value()?);
                }
                Value::Array(items)
            }
            "string" => Value::String(self.text()?),
            "integer" => {
                let t = self.text()?;
                let t = t.trim();
                let n = if let Some(h) = t.strip_prefix("0x") {
                    i64::from_str_radix(h, 16).ok()
                } else {
                    t.parse::<i64>().ok()
                };
                Value::Integer(n.ok_or_else(|| format_err(format!("bad integer {t:?}")))?)
            }
            "real" => {
                let t = self.text()?;
                Value::Real(
                    t.trim()
                        .parse()
                        .map_err(|_| format_err(format!("bad real {t:?}")))?,
                )
            }
            "data" => Value::Data(base64_decode(&self.text()?)?),
            "date" => Value::Date(self.text()?.trim().to_string()),
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            _ => return Err(format_err(format!("unknown plist element <{name}>"))),
        };
        self.close_tag(name)?;
        Ok(v)
    }
}

fn unescape(s: &str) -> Result<String> {
    if !s.contains('&') {
        return Ok(s.to_string());
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let end = rest
            .find(';')
            .ok_or_else(|| format_err("unterminated entity"))?;
        let ent = &rest[1..end];
        let c = match ent {
            "lt" => '<',
            "gt" => '>',
            "amp" => '&',
            "quot" => '"',
            "apos" => '\'',
            _ => {
                let code = if let Some(h) = ent.strip_prefix("#x") {
                    u32::from_str_radix(h, 16).ok()
                } else if let Some(d) = ent.strip_prefix('#') {
                    d.parse().ok()
                } else {
                    None
                };
                code.and_then(char::from_u32)
                    .ok_or_else(|| format_err(format!("unknown entity &{ent};")))?
            }
        };
        out.push(c);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding, no line breaks.
pub fn base64_encode(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        s.push(B64[(n >> 18) as usize & 63] as char);
        s.push(B64[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 {
            B64[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        s.push(if c.len() > 2 {
            B64[n as usize & 63] as char
        } else {
            '='
        });
    }
    s
}

/// Decodes base64, ignoring white space.
pub fn base64_decode(s: &str) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            c if c.is_ascii_whitespace() => continue,
            _ => return Err(format_err("bad base64")),
        };
        acc = acc << 6 | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
}

/// Escapes text the way CoreFoundation's XML writer does (`&`, `<`, `>`).
pub fn escape(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
}

/// Compares two strings by UTF-16 code units, the order CoreFoundation sorts keys in.
pub fn cf_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    if a.is_ascii() && b.is_ascii() {
        return a.as_bytes().cmp(b.as_bytes());
    }
    a.encode_utf16().cmp(b.encode_utf16())
}

/// The XML header CoreFoundation writes.
pub const XML_HEADER: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n";

/// Serialises `v` as CoreFoundation does (`CFPropertyListCreateData`, XML format).
///
/// Limits (checked): `<data>` values longer than 57 bytes and non-integral reals are rejected,
/// because their exact CF layout (line wrapping, float formatting) is not reproduced.
pub fn to_cf_xml(v: &Value) -> Result<String> {
    let mut out = String::from(XML_HEADER);
    write_value(v, 0, &mut out)?;
    out.push_str("</plist>\n");
    Ok(out)
}

/// Appends `v` at `indent` tabs (the first line's indentation is written by the caller).
pub fn write_value(v: &Value, indent: usize, out: &mut String) -> Result<()> {
    let tabs = |out: &mut String, n: usize| out.extend(std::iter::repeat_n('\t', n));
    match v {
        Value::Dict(entries) if entries.is_empty() => out.push_str("<dict/>\n"),
        Value::Dict(entries) => {
            out.push_str("<dict>\n");
            let mut sorted: Vec<&(String, Value)> = entries.iter().collect();
            sorted.sort_by(|a, b| cf_cmp(&a.0, &b.0));
            for (k, v) in sorted {
                tabs(out, indent + 1);
                out.push_str("<key>");
                escape(k, out);
                out.push_str("</key>\n");
                tabs(out, indent + 1);
                write_value(v, indent + 1, out)?;
            }
            tabs(out, indent);
            out.push_str("</dict>\n");
        }
        Value::Array(items) if items.is_empty() => out.push_str("<array/>\n"),
        Value::Array(items) => {
            out.push_str("<array>\n");
            for v in items {
                tabs(out, indent + 1);
                write_value(v, indent + 1, out)?;
            }
            tabs(out, indent);
            out.push_str("</array>\n");
        }
        Value::String(s) => {
            out.push_str("<string>");
            escape(s, out);
            out.push_str("</string>\n");
        }
        Value::Integer(n) => {
            out.push_str("<integer>");
            out.push_str(&n.to_string());
            out.push_str("</integer>\n");
        }
        Value::Real(r) => {
            if r.fract() != 0.0 || r.abs() >= 1e15 {
                return Err(Error::Unsupported(format!("CF layout of real {r}")));
            }
            out.push_str("<real>");
            out.push_str(&(*r as i64).to_string());
            out.push_str("</real>\n");
        }
        Value::Bool(true) => out.push_str("<true/>\n"),
        Value::Bool(false) => out.push_str("<false/>\n"),
        Value::Data(d) => {
            if d.len() > 57 {
                return Err(Error::Unsupported(
                    "CF layout of <data> longer than 57 bytes".into(),
                ));
            }
            out.push_str("<data>\n");
            tabs(out, indent);
            out.push_str(&base64_encode(d));
            out.push('\n');
            tabs(out, indent);
            out.push_str("</data>\n");
        }
        Value::Date(d) => {
            out.push_str("<date>");
            out.push_str(d);
            out.push_str("</date>\n");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_info_plist_shapes() {
        let x = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<!-- c -->
<dict>
	<key>CFBundleIdentifier</key>
	<string>org.a&amp;b</string>
	<key>n</key><integer>-5</integer>
	<key>r</key><real>2.5</real>
	<key>t</key><true/>
	<key>e</key><array/>
	<key>d</key><data>aGk=</data>
	<key>s</key><string/>
	<key>a</key><array><string>x</string><false/></array>
</dict>
</plist>
"#;
        let v = parse(x).unwrap();
        assert_eq!(
            v.get("CFBundleIdentifier").unwrap().as_str(),
            Some("org.a&b")
        );
        assert_eq!(v.get("n"), Some(&Value::Integer(-5)));
        assert_eq!(v.get("r"), Some(&Value::Real(2.5)));
        assert_eq!(v.get("t"), Some(&Value::Bool(true)));
        assert_eq!(v.get("e"), Some(&Value::Array(vec![])));
        assert_eq!(v.get("d"), Some(&Value::Data(b"hi".to_vec())));
        assert_eq!(v.get("s"), Some(&Value::String(String::new())));
        assert_eq!(
            v.get("a"),
            Some(&Value::Array(vec![
                Value::String("x".into()),
                Value::Bool(false)
            ]))
        );
    }

    const BINARY_SAMPLE: &str = "62706c6973743030d3010203040508516e51615f1012434642756e646c6545786563757461626c6511012ca2060710016100e95178080f1113282b2e30330000000000000101000000000000000900000000000000000000000000000035";

    #[test]
    fn binary_plist() {
        // plutil -convert binary1 of {CFBundleExecutable = "x"; n = 300; a = (1, "\u{e9}")}.
        let b = crate::hash::unhex(BINARY_SAMPLE).unwrap();
        let v = parse(&b).unwrap();
        assert_eq!(v.get("CFBundleExecutable").unwrap().as_str(), Some("x"));
        assert_eq!(v.get("n"), Some(&Value::Integer(300)));
        assert_eq!(
            v.get("a"),
            Some(&Value::Array(vec![
                Value::Integer(1),
                Value::String("\u{e9}".into())
            ]))
        );
    }

    #[test]
    fn rejects_binary_and_garbage() {
        assert!(parse(b"bplist00").is_err());
        assert!(matches!(
            parse(b"bplist15xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"),
            Err(Error::Unsupported(_))
        ));
        assert!(parse(b"<plist><dict><key>a</key></dict></plist>").is_err());
        assert!(parse(b"<plist><foo/></plist>").is_err());
    }

    #[test]
    fn writer_layout() {
        let v = Value::Dict(vec![
            ("b".into(), Value::Data(vec![1, 2, 3])),
            ("a<&>".into(), Value::Bool(true)),
            (
                "c".into(),
                Value::Dict(vec![("w".into(), Value::Real(1000.0))]),
            ),
            ("d".into(), Value::Dict(vec![])),
        ]);
        let s = to_cf_xml(&v).unwrap();
        let body = s.strip_prefix(XML_HEADER).unwrap();
        assert_eq!(
            body,
            "<dict>\n\t<key>a&lt;&amp;&gt;</key>\n\t<true/>\n\t<key>b</key>\n\t<data>\n\tAQID\n\t</data>\n\t<key>c</key>\n\t<dict>\n\t\t<key>w</key>\n\t\t<real>1000</real>\n\t</dict>\n\t<key>d</key>\n\t<dict/>\n</dict>\n</plist>\n"
        );
    }

    #[test]
    fn base64_round_trip() {
        for n in 0..70 {
            let d: Vec<u8> = (0..n).map(|i| (i * 37 + 5) as u8).collect();
            assert_eq!(base64_decode(&base64_encode(&d)).unwrap(), d);
        }
        assert_eq!(base64_encode(b"hi"), "aGk=");
    }

    #[test]
    fn utf16_order() {
        use std::cmp::Ordering::*;
        assert_eq!(cf_cmp("B", "a"), Less);
        assert_eq!(cf_cmp("_", "a"), Less);
        // U+FF01 sorts after a surrogate pair in UTF-8 byte order, before it in UTF-16.
        assert_eq!(cf_cmp("\u{FF01}", "\u{1F600}"), Greater);
        assert!("\u{FF01}".as_bytes() < "\u{1F600}".as_bytes());
    }
}
