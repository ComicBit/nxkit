//! The few Python semantics the profile readers depend on: `str.splitlines`, `str.split()`, `str.strip()`, `int(s, 0)`,
//! `int(s, 16)` and `'%#x'`.

/// `str.splitlines()`: splits at `\n`, `\r`, `\r\n`, `\x0b`, `\x0c`, `\x1c`, `\x1d`, `\x1e`, `\x85`, ` `, ` `.
pub fn splitlines(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut it = s.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        let brk = matches!(
            c,
            '\n' | '\r'
                | '\u{0b}'
                | '\u{0c}'
                | '\u{1c}'
                | '\u{1d}'
                | '\u{1e}'
                | '\u{85}'
                | '\u{2028}'
                | '\u{2029}'
        );
        if !brk {
            continue;
        }
        out.push(&s[start..i]);
        let mut end = i + c.len_utf8();
        if c == '\r'
            && let Some((j, '\n')) = it.peek().copied()
        {
            it.next();
            end = j + 1;
        }
        start = end;
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

/// Python's whitespace (`str.isspace`) for `strip` and `split()`.
pub fn is_space(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}')
}

/// `str.strip()`.
pub fn strip(s: &str) -> &str {
    s.trim_matches(is_space)
}

/// `str.split()` (runs of whitespace, no empty words).
pub fn words(s: &str) -> Vec<&str> {
    s.split(is_space).filter(|w| !w.is_empty()).collect()
}

fn digits(s: &str, radix: u32) -> Option<i128> {
    // digits with single underscores between them
    if s.is_empty() || s.starts_with('_') || s.ends_with('_') || s.contains("__") {
        return None;
    }
    let mut v: i128 = 0;
    for c in s.chars().filter(|&c| c != '_') {
        let d = c.to_digit(radix)?;
        v = v.checked_mul(radix as i128)?.checked_add(d as i128)?;
    }
    Some(v)
}

fn sign(s: &str) -> (i128, &str) {
    let s = strip(s);
    match s.as_bytes().first() {
        Some(b'-') => (-1, &s[1..]),
        Some(b'+') => (1, &s[1..]),
        _ => (1, s),
    }
}

/// `int(s, 0)`: optional sign, `0x`/`0o`/`0b` prefixes, underscores, no leading zeros in a decimal number but zero.
pub fn int0(s: &str) -> Result<i128, String> {
    let bad = || format!("invalid literal for int() with base 0: {s:?}");
    let (sg, t) = sign(s);
    let lower = t.get(..2).map(str::to_ascii_lowercase);
    let v = match lower.as_deref() {
        Some("0x") => digits(t[2..].trim_start_matches('_'), 16),
        Some("0o") => digits(t[2..].trim_start_matches('_'), 8),
        Some("0b") => digits(t[2..].trim_start_matches('_'), 2),
        _ => {
            let v = digits(t, 10);
            if v.is_some_and(|v| v != 0) && t.starts_with('0') {
                None
            } else {
                v
            }
        }
    };
    v.map(|v| sg * v).ok_or_else(bad)
}

/// `int(s, 16)`: optional sign and `0x` prefix, underscores.
pub fn int16(s: &str) -> Result<i128, String> {
    let (sg, t) = sign(s);
    let t = match t.get(..2).map(str::to_ascii_lowercase).as_deref() {
        Some("0x") => t[2..].trim_start_matches('_'),
        _ => t,
    };
    digits(t, 16)
        .map(|v| sg * v)
        .ok_or_else(|| format!("invalid literal for int() with base 16: {s:?}"))
}

/// `f'{v:#x}'`.
pub fn hex(v: i128) -> String {
    if v < 0 {
        format!("-{:#x}", v.unsigned_abs())
    } else {
        format!("{v:#x}")
    }
}

/// `repr(s)` of a Python `str`. Non-ASCII characters are kept as Python keeps printable ones, the C1 controls, the
/// no-break space and the soft hyphen are escaped (other non-printable non-ASCII characters are not told apart: the
/// strings hashed here are paths, compiler arguments and C headers).
pub fn repr_str(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20
                || c as u32 == 0x7f
                || (0x80..=0xa0).contains(&(c as u32))
                || c as u32 == 0xad =>
            {
                out.push_str(&format!("\\x{:02x}", c as u32))
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// `shlex.split(s)` (POSIX mode, no comments).
pub fn shlex_split(s: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut have = false;
    let mut it = s.chars();
    while let Some(c) = it.next() {
        match c {
            c if c.is_whitespace() => {
                if have {
                    out.push(std::mem::take(&mut cur));
                    have = false;
                }
            }
            '\'' => {
                have = true;
                loop {
                    match it.next() {
                        Some('\'') => break,
                        Some(c) => cur.push(c),
                        None => return Err("No closing quotation".into()),
                    }
                }
            }
            '"' => {
                have = true;
                loop {
                    match it.next() {
                        Some('"') => break,
                        Some('\\') => match it.next() {
                            Some(c @ ('\\' | '"')) => cur.push(c),
                            Some(c) => {
                                cur.push('\\');
                                cur.push(c);
                            }
                            None => return Err("No closing quotation".into()),
                        },
                        Some(c) => cur.push(c),
                        None => return Err("No closing quotation".into()),
                    }
                }
            }
            '\\' => {
                have = true;
                match it.next() {
                    Some(c) => cur.push(c),
                    None => return Err("No escaped character".into()),
                }
            }
            c => {
                have = true;
                cur.push(c);
            }
        }
    }
    if have {
        out.push(cur);
    }
    Ok(out)
}

/// `str(PurePosixPath(s))`: empty and `.` parts dropped, repeated slashes joined, a trailing slash dropped (`..` kept).
pub fn posix_path(s: &str) -> String {
    let lead = if s.starts_with("//") && !s.starts_with("///") {
        "//"
    } else if s.starts_with('/') {
        "/"
    } else {
        ""
    };
    let parts: Vec<&str> = s
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();
    let body = parts.join("/");
    if lead.is_empty() && body.is_empty() {
        ".".into()
    } else {
        format!("{lead}{body}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_python() {
        assert_eq!(
            splitlines("a\r\nb\rc\n\nd\u{2028}e\n"),
            ["a", "b", "c", "", "d", "e"]
        );
        assert_eq!(splitlines(""), Vec::<&str>::new());
        assert_eq!(words(" a\tb  c\n"), ["a", "b", "c"]);
        assert_eq!(strip("\u{1f} x \t"), "x");
        for (s, v) in [
            ("0x4000_000", 0x4000000),
            ("0X1f", 31),
            ("12", 12),
            ("0", 0),
            ("00", 0),
            ("-0x10", -16),
            (" 7 ", 7),
            ("0b101", 5),
            ("0o17", 15),
            ("0x_1", 1),
        ] {
            assert_eq!(int0(s), Ok(v), "{s}");
        }
        for s in ["012", "0x", "1__0", "_1", "1_", "x", "", "0xg"] {
            assert!(int0(s).is_err(), "{s}");
        }
        assert_eq!(int16("0100A3D008F2C000"), Ok(0x0100A3D008F2C000));
        assert_eq!(int16("0x1f"), Ok(31));
        assert!(int16("").is_err());
        assert_eq!(hex(0x2000000), "0x2000000");
        assert_eq!(hex(0), "0x0");
        assert_eq!(hex(-5), "-0x5");
        assert_eq!(repr_str("a'b"), "\"a'b\"");
        assert_eq!(repr_str("a'\"b"), "'a\\'\"b'");
        assert_eq!(
            repr_str("x\ty\n\\\u{1}\u{7f}\u{e9}"),
            "'x\\ty\\n\\\\\\x01\\x7f\u{e9}'"
        );
        assert_eq!(
            shlex_split("clang -O1 'a b' \"c\\\"d\" e\\ f -o dmhost").unwrap(),
            ["clang", "-O1", "a b", "c\"d", "e f", "-o", "dmhost"]
        );
        assert!(shlex_split("'open").is_err());
        assert_eq!(posix_path("/a//b/./c/"), "/a/b/c");
        assert_eq!(posix_path("a/../b"), "a/../b");
        assert_eq!(posix_path("//x"), "//x");
    }
}
