//! The part of Python's `xml.etree.ElementTree` the workspace file needs: elements with text and tails, the parser's
//! handling (comments, processing instructions and the DOCTYPE dropped, entities decoded, attribute values
//! normalised), `ET.indent` and the serialiser with its XML declaration.

/// An element as ElementTree keeps it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Element {
    /// The tag.
    pub tag: String,
    /// Attributes in document order.
    pub attrs: Vec<(String, String)>,
    /// Text before the first child (`None`: no text).
    pub text: Option<String>,
    /// Text after the element.
    pub tail: Option<String>,
    /// Children.
    pub children: Vec<Element>,
}

impl Element {
    /// `ET.Element(tag, **attrs)`.
    pub fn new(tag: &str, attrs: &[(&str, &str)]) -> Self {
        Element {
            tag: tag.into(),
            attrs: attrs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            ..Element::default()
        }
    }

    /// `elem.get(k)`.
    pub fn get(&self, k: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.as_str())
    }

    /// `elem.set(k, v)`.
    pub fn set(&mut self, k: &str, v: &str) {
        match self.attrs.iter_mut().find(|(n, _)| n == k) {
            Some(slot) => slot.1 = v.into(),
            None => self.attrs.push((k.into(), v.into())),
        }
    }

    /// `elem.iter(tag)`: this element and its descendants, document order.
    pub fn iter<'a>(&'a self, tag: &str, out: &mut Vec<&'a Element>) {
        if self.tag == tag {
            out.push(self);
        }
        for c in &self.children {
            c.iter(tag, out);
        }
    }
}

/// A parse error (`ET.ParseError`).
#[derive(Debug)]
pub struct ParseError(pub String);

struct Parser<'a> {
    s: &'a str,
    i: usize,
}

fn decode(raw: &str, attr: bool) -> Result<String, ParseError> {
    let mut out = String::new();
    let mut rest = raw;
    while let Some(c) = rest.chars().next() {
        if c == '&' {
            let end = rest
                .find(';')
                .ok_or_else(|| ParseError("unterminated entity".into()))?;
            let name = &rest[1..end];
            let ch = match name {
                "lt" => '<',
                "gt" => '>',
                "amp" => '&',
                "quot" => '"',
                "apos" => '\'',
                _ => {
                    let v = if let Some(h) = name.strip_prefix("#x") {
                        u32::from_str_radix(h, 16).ok()
                    } else if let Some(d) = name.strip_prefix('#') {
                        d.parse().ok()
                    } else {
                        None
                    };
                    v.and_then(char::from_u32)
                        .ok_or_else(|| ParseError(format!("undefined entity &{name};")))?
                }
            };
            out.push(ch);
            rest = &rest[end + 1..];
        } else {
            // attribute-value normalisation: literal whitespace characters become spaces
            out.push(if attr && matches!(c, '\t' | '\n' | '\r') {
                ' '
            } else {
                c
            });
            rest = &rest[c.len_utf8()..];
        }
    }
    Ok(out)
}

impl Parser<'_> {
    fn err(&self, m: &str) -> ParseError {
        ParseError(format!("{m} at byte {}", self.i))
    }

    fn rest(&self) -> &str {
        &self.s[self.i..]
    }

    fn skip_ws(&mut self) {
        let r = self.rest();
        self.i += r.len() - r.trim_start_matches([' ', '\t', '\n', '\r']).len();
    }

    fn skip_past(&mut self, end: &str) -> Result<(), ParseError> {
        let k = self
            .rest()
            .find(end)
            .ok_or_else(|| self.err("unterminated markup"))?;
        self.i += k + end.len();
        Ok(())
    }

    fn name(&mut self) -> Result<String, ParseError> {
        let r = self.rest();
        let n = r
            .find(|c: char| c.is_whitespace() || matches!(c, '=' | '>' | '/'))
            .unwrap_or(r.len());
        if n == 0 {
            return Err(self.err("expected a name"));
        }
        let name = r[..n].to_string();
        self.i += n;
        Ok(name)
    }

    /// Markup that holds no element (comments, processing instructions, the DOCTYPE): skipped as the tree builder does.
    fn skip_misc(&mut self) -> Result<bool, ParseError> {
        if self.rest().starts_with("<!--") {
            self.skip_past("-->")?;
        } else if self.rest().starts_with("<?") {
            self.skip_past("?>")?;
        } else if self.rest().starts_with("<!DOCTYPE") {
            self.skip_past(">")?;
        } else {
            return Ok(false);
        }
        Ok(true)
    }

    fn element(&mut self) -> Result<Element, ParseError> {
        self.i += 1; // '<'
        let mut e = Element {
            tag: self.name()?,
            ..Element::default()
        };
        loop {
            self.skip_ws();
            if self.rest().starts_with("/>") {
                self.i += 2;
                return Ok(e);
            }
            if self.rest().starts_with('>') {
                self.i += 1;
                break;
            }
            let k = self.name()?;
            self.skip_ws();
            if !self.rest().starts_with('=') {
                return Err(self.err("expected ="));
            }
            self.i += 1;
            self.skip_ws();
            let q = self
                .rest()
                .chars()
                .next()
                .filter(|c| *c == '"' || *c == '\'')
                .ok_or_else(|| self.err("expected a quoted value"))?;
            self.i += 1;
            let end = self
                .rest()
                .find(q)
                .ok_or_else(|| self.err("unterminated value"))?;
            let v = decode(&self.rest()[..end], true)?;
            self.i += end + 1;
            if e.get(&k).is_some() {
                return Err(self.err("duplicate attribute"));
            }
            e.attrs.push((k, v));
        }
        let mut text = String::new();
        loop {
            let r = self.rest();
            if r.is_empty() {
                return Err(self.err("no element found"));
            }
            if r.starts_with("</") {
                self.i += 2;
                let n = self.name()?;
                if n != e.tag {
                    return Err(self.err("mismatched tag"));
                }
                self.skip_ws();
                if !self.rest().starts_with('>') {
                    return Err(self.err("expected >"));
                }
                self.i += 1;
                Self::put(&mut e, text);
                return Ok(e);
            }
            if r.starts_with("<![CDATA[") {
                let end = r
                    .find("]]>")
                    .ok_or_else(|| self.err("unterminated CDATA"))?;
                text.push_str(&r[9..end]);
                self.i += end + 3;
                continue;
            }
            if r.starts_with('<') {
                if self.skip_misc()? {
                    continue;
                }
                Self::put(&mut e, std::mem::take(&mut text));
                let c = self.element()?;
                e.children.push(c);
                continue;
            }
            let n = r.find('<').unwrap_or(r.len());
            text.push_str(&decode(&r[..n], false)?);
            self.i += n;
        }
    }

    /// Text gathered so far: the element's text before its first child, else the last child's tail.
    fn put(e: &mut Element, text: String) {
        if text.is_empty() {
            return;
        }
        match e.children.last_mut() {
            Some(c) => c.tail = Some(text),
            None => e.text = Some(text),
        }
    }
}

/// `ET.parse(path).getroot()` of the file's text.
pub fn parse(xml: &str) -> Result<Element, ParseError> {
    let s = xml.replace("\r\n", "\n").replace('\r', "\n");
    let mut p = Parser { s: &s, i: 0 };
    loop {
        p.skip_ws();
        if p.rest().is_empty() {
            return Err(p.err("no element found"));
        }
        if !p.skip_misc()? {
            break;
        }
    }
    if !p.rest().starts_with('<') {
        return Err(p.err("syntax error"));
    }
    let root = p.element()?;
    loop {
        p.skip_ws();
        if p.rest().is_empty() {
            return Ok(root);
        }
        if !p.skip_misc()? {
            return Err(p.err("junk after document element"));
        }
    }
}

fn esc_text(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
}

fn esc_attr(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\r' => out.push_str("&#13;"),
            '\n' => out.push_str("&#10;"),
            '\t' => out.push_str("&#09;"),
            c => out.push(c),
        }
    }
}

fn serialize(e: &Element, out: &mut String) {
    out.push('<');
    out.push_str(&e.tag);
    for (k, v) in &e.attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        esc_attr(v, out);
        out.push('"');
    }
    let text = e.text.as_deref().unwrap_or("");
    if !text.is_empty() || !e.children.is_empty() {
        out.push('>');
        esc_text(text, out);
        for c in &e.children {
            serialize(c, out);
        }
        out.push_str("</");
        out.push_str(&e.tag);
        out.push('>');
    } else {
        out.push_str(" />");
    }
    if let Some(t) = &e.tail {
        esc_text(t, out);
    }
}

/// `tree.write(path, encoding=..., xml_declaration=True)`: the declaration names `declared` (`utf-8` for
/// `encoding='utf-8'`, the locale's `UTF-8` for `encoding='unicode'`).
pub fn to_xml(root: &Element, declared: &str) -> String {
    let mut out = format!("<?xml version='1.0' encoding='{declared}'?>\n");
    serialize(root, &mut out);
    out
}

/// `ET.indent(tree, space)`.
pub fn indent(root: &mut Element, space: &str) {
    fn go(e: &mut Element, level: usize, ind: &mut Vec<String>, space: &str) {
        let child = level + 1;
        if ind.len() <= child {
            let next = format!("{}{space}", ind[level]);
            ind.push(next);
        }
        let ci = ind[child].clone();
        if e.text.as_deref().is_none_or(|t| t.trim().is_empty()) {
            e.text = Some(ci.clone());
        }
        for c in e.children.iter_mut() {
            if !c.children.is_empty() {
                go(c, child, ind, space);
            }
            if c.tail.as_deref().is_none_or(|t| t.trim().is_empty()) {
                c.tail = Some(ci.clone());
            }
        }
        if let Some(last) = e.children.last_mut()
            && last.tail.as_deref().is_some_and(|t| t.trim().is_empty())
        {
            last.tail = Some(ind[level].clone());
        }
    }
    if root.children.is_empty() {
        return;
    }
    let mut ind = vec!["\n".to_string()];
    go(root, 0, &mut ind, space);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A workspace as Xcode writes it (attributes on their own lines, whitespace text in leaf elements), with a comment,
    /// an entity and another game.
    const XCODE: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Workspace\n   version = \"1.0\">\n   <!-- note -->\n   <FileRef\n      location = \"group:Other &amp; Co.xcodeproj\">\n   </FileRef>\n   <Group\n      location = \"container:\"\n      name = \"Old\">\n      <FileRef\n         location = \"absolute:/nonexistent/old/DmIOS.xcodeproj\">\n      </FileRef>\n   </Group>\n</Workspace>\n";

    #[test]
    fn tree_round_trip_and_indent() {
        let mut root = parse(XCODE).unwrap();
        assert_eq!(root.children.len(), 2);
        assert_eq!(
            root.children[0].get("location"),
            Some("group:Other & Co.xcodeproj")
        );
        indent(&mut root, "   ");
        let x = to_xml(&root, "utf-8");
        assert!(x.starts_with("<?xml version='1.0' encoding='utf-8'?>\n<Workspace version=\"1.0\">\n   <FileRef location=\"group:Other &amp; Co.xcodeproj\">\n   </FileRef>\n"));
        let mut e = Element::new("Workspace", &[("version", "1.0")]);
        e.children.push(Element::new("Group", &[("a", "x\"\n")]));
        indent(&mut e, "   ");
        assert_eq!(
            to_xml(&e, "utf-8"),
            "<?xml version='1.0' encoding='utf-8'?>\n<Workspace version=\"1.0\">\n   <Group a=\"x&quot;&#10;\" />\n</Workspace>"
        );
        assert!(parse("<a><b></a>").is_err());
        assert!(parse("").is_err());
    }
}
