//! Small text matchers that reproduce the Python regular expressions of the side-entry port. All positions are byte
//! indices, every delimiter is ASCII, so byte and code-point processing agree.

/// Python `str.isspace` for the ASCII range plus Unicode white space (what `\s` and `strip()` match).
pub(crate) fn is_ws(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// Python `\w` (Unicode word characters, MSL is ASCII in practice).
pub(crate) fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Python `str.strip()`.
pub(crate) fn strip(s: &str) -> &str {
    s.trim_matches(is_ws)
}

/// Index after the whitespace run starting at `i`.
pub(crate) fn skip_ws(s: &str, i: usize) -> usize {
    let mut j = i;
    for c in s[i..].chars() {
        if !is_ws(c) {
            break;
        }
        j += c.len_utf8();
    }
    j
}

/// Index after the word run starting at `i` (equal to `i` when there is none).
pub(crate) fn skip_word(s: &str, i: usize) -> usize {
    let mut j = i;
    for c in s[i..].chars() {
        if !is_word(c) {
            break;
        }
        j += c.len_utf8();
    }
    j
}

/// Index after the ASCII digit run starting at `i`.
pub(crate) fn skip_digits(s: &str, i: usize) -> usize {
    i + s[i..].bytes().take_while(u8::is_ascii_digit).count()
}

/// Python `re.search(r'struct NAME\s*\{(.*?)\};', msl, re.S)`: the first struct body.
pub(crate) fn struct_body<'a>(msl: &'a str, name: &str) -> Option<&'a str> {
    let pat = format!("struct {name}");
    let mut from = 0;
    while let Some(off) = msl[from..].find(&pat) {
        let at = from + off;
        let j = skip_ws(msl, at + pat.len());
        if msl[j..].starts_with('{') {
            let body_start = j + 1;
            if let Some(end) = msl[body_start..].find("};") {
                return Some(&msl[body_start..body_start + end]);
            }
            // No `};` after this one means none after any later one either.
            return None;
        }
        from = at + 1;
    }
    None
}

/// The starts of the candidate lines of a `re.M` pattern beginning with `^\s*`: each line start, with the leading white
/// space skipped (which may cross blank lines, the leftmost match is the same).
pub(crate) fn line_starts(s: &str) -> impl Iterator<Item = usize> + '_ {
    std::iter::once(0)
        .chain(s.match_indices('\n').map(|(i, _)| i + 1))
        .map(move |i| skip_ws(s, i))
}
