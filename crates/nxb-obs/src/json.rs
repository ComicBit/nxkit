//! Minimal JSON string escaping with Python `json.dumps(ensure_ascii=False)` rules.

/// Append `s` as a JSON string literal (with quotes) to `out`.
///
/// ```
/// let mut out = String::new();
/// nxb_obs::escape_into(&mut out, "a\"b\n\u{1}é");
/// assert_eq!(out, "\"a\\\"b\\n\\u0001é\"");
/// ```
pub fn escape_into(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use nxb_testkit::check;

    /// Test-only decoder for the escapes `escape_into` produces.
    pub(crate) fn unescape(lit: &str) -> Option<String> {
        let inner = lit.strip_prefix('"')?.strip_suffix('"')?;
        let mut out = String::new();
        let mut it = inner.chars();
        while let Some(c) = it.next() {
            if c != '\\' {
                if c == '"' || (c as u32) < 0x20 {
                    return None;
                }
                out.push(c);
                continue;
            }
            match it.next()? {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                'b' => out.push('\u{08}'),
                'f' => out.push('\u{0c}'),
                'u' => {
                    let hex: String = it.by_ref().take(4).collect();
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                }
                _ => return None,
            }
        }
        Some(out)
    }

    #[test]
    fn python_examples() {
        // json.dumps("tab\there", ensure_ascii=False) == '"tab\\there"'
        let mut s = String::new();
        escape_into(&mut s, "tab\there \u{7f} ü \u{2028}");
        assert_eq!(s, "\"tab\\there \u{7f} ü \u{2028}\"");
    }

    #[test]
    fn round_trip_property() {
        let alphabet: Vec<char> = "ab\"\\\n\r\t\u{0}\u{1f}\u{8}\u{c}é漢😀 /".chars().collect();
        check(41, 500, |r| {
            let n = r.below(30) as usize;
            let s = r.string_from(&alphabet, n);
            let mut lit = String::new();
            escape_into(&mut lit, &s);
            assert!(!lit.contains('\n'), "a literal never spans lines");
            assert_eq!(unescape(&lit).as_deref(), Some(s.as_str()));
        });
    }
}
