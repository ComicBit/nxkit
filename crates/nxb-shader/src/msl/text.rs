//! Shared text matchers of the MSL passes.

/// Python `\w`.
pub(crate) fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `re.search(PREFIX + r'\s*\{(.*?)\};', s, re.S)` → the group's (start, end).
pub(crate) fn struct_group(s: &str, prefix: &str) -> Option<(usize, usize)> {
    let mut from = 0;
    while let Some(off) = s[from..].find(prefix) {
        let at = from + off;
        let j = at + prefix.len();
        let k = j + (s[j..].len() - s[j..].trim_start().len());
        if s[k..].starts_with('{') {
            let gs = k + 1;
            let ge = gs + s[gs..].find("};")?;
            return Some((gs, ge));
        }
        from = at + 1;
    }
    None
}

/// From just after an opening `(`, the index of its closing `)` (depth counting on every paren, as the Python loops do).
pub(crate) fn find_matching(s: &str, mut i: usize) -> Option<usize> {
    let b = s.as_bytes();
    let mut depth = 1i64;
    while depth != 0 {
        let c = *b.get(i)?;
        if c == b'(' {
            depth += 1;
        } else if c == b')' {
            depth -= 1;
        }
        i += 1;
    }
    Some(i - 1)
}

/// `\bWORD` occurrences that start a word (previous char not a word char).
pub(crate) fn word_starts<'a>(s: &'a str, w: &'a str) -> impl Iterator<Item = usize> + 'a {
    s.match_indices(w)
        .map(|(i, _)| i)
        .filter(move |i| !s[..*i].chars().next_back().is_some_and(is_word))
}
