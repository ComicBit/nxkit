//! Shared matchers of the vertex variants.

/// The `main0_in` struct: `re.search(r'\nstruct main0_in\n\{\n(.*?)\n\};', msl, re.S)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageIn<'a> {
    /// Index of the leading newline (`m.start()`).
    pub start: usize,
    /// Index after `};` (`m.end()`).
    pub end: usize,
    /// The member lines (`m.group(1)`).
    pub body: &'a str,
}

impl<'a> StageIn<'a> {
    /// Find the struct.
    pub fn find(msl: &'a str) -> Option<StageIn<'a>> {
        const HEAD: &str = "\nstruct main0_in\n{\n";
        let start = msl.find(HEAD)?;
        let b = start + HEAD.len();
        // Lazy `(.*?)\n\};`: the first "\n};" at or after the body start.
        let rel = msl[b..].find("\n};")?;
        let (body, end) = (&msl[b..b + rel], b + rel + 3);
        Some(StageIn { start, end, body })
    }
}

/// One stage input: `    (float4|uint4|int4) (m_\d+(?:_\d+)*) \[\[attribute\((\d+)\)\]\];` (fullmatch).
pub(crate) fn member(line: &str) -> Option<(&str, &str, u32)> {
    let rest = line.strip_prefix("    ")?;
    let ty = ["float4", "uint4", "int4"]
        .into_iter()
        .find(|t| rest.starts_with(&format!("{t} ")))?;
    let rest = &rest[ty.len() + 1..];
    let sp = rest.find(' ')?;
    let name = &rest[..sp];
    let digits_ok = |s: &str| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit());
    let mut parts = name.strip_prefix("m_")?.split('_');
    if !parts.all(digits_ok) {
        return None;
    }
    let attr = rest[sp..]
        .strip_prefix(" [[attribute(")?
        .strip_suffix(")]];")?;
    if !digits_ok(attr) {
        return None;
    }
    Some((ty, name, attr.parse().ok()?))
}

/// Python `ln.strip()[:60]`.
pub(crate) fn short(ln: &str) -> String {
    super::super::side::strip_py(ln).chars().take(60).collect()
}

/// `re.search(r'uint (\w+) \[\[ATTR\]\]', s)`: the parameter name.
pub(crate) fn uint_param<'a>(s: &'a str, attr: &str) -> Option<&'a str> {
    let tail = format!(" [[{attr}]]");
    for (i, _) in s.match_indices("uint ") {
        let a = i + 5;
        let end = a + s[a..]
            .char_indices()
            .find(|(_, c)| !(c.is_alphanumeric() || *c == '_'))
            .map_or(s.len() - a, |(k, _)| k);
        if end > a && s[end..].starts_with(&tail) {
            return Some(&s[a..end]);
        }
    }
    None
}

/// The vertex entry header `\nvertex main0_out ENTRY\(main0_in in \[\[stage_in\]\]G\)\n\{\n` where G is the rest of the
/// line before its final `)`.`comma` requires G to be empty or start with `, ` (`dmsh_pull`). Returns (start, end, G).
pub(crate) fn vertex_header<'a>(
    body: &'a str,
    entry: &str,
    comma: bool,
) -> Option<(usize, usize, &'a str)> {
    let prefix = format!("\nvertex main0_out {entry}(main0_in in [[stage_in]]");
    for (i, _) in body.match_indices(&prefix) {
        let a = i + prefix.len();
        let line_end = body[a..].find('\n').map_or(body.len(), |k| a + k);
        let line = &body[a..line_end];
        let Some(g) = line.strip_suffix(')') else {
            continue;
        };
        if !body[line_end..].starts_with("\n{\n") {
            continue;
        }
        if comma && !(g.is_empty() || g.starts_with(", ")) {
            continue;
        }
        return Some((i, line_end + 3, g));
    }
    None
}
