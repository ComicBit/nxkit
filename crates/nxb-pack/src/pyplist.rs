//! Python's `plistlib.dumps(value)` (XML, `sort_keys=True`) byte for byte: what `pack_app.py` writes as `Info.plist`.

/// A property list value as `plistlib` writes it.
#[derive(Clone, Debug, PartialEq)]
pub enum PyValue {
    /// `<string>`.
    Str(String),
    /// `<true/>` / `<false/>`.
    Bool(bool),
    /// `<integer>`.
    Int(i64),
    /// `<real>` (Python `repr`).
    Real(f64),
    /// `<array>`.
    List(Vec<PyValue>),
    /// `<dict>`, written with sorted keys.
    Dict(Vec<(String, PyValue)>),
}

fn escape(s: &str, out: &mut String) {
    let s = s.replace("\r\n", "\n").replace('\r', "\n");
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
}

fn line(out: &mut String, depth: usize, text: &str) {
    for _ in 0..depth {
        out.push('\t');
    }
    out.push_str(text);
    out.push('\n');
}

fn write(v: &PyValue, depth: usize, out: &mut String) {
    match v {
        PyValue::Str(s) => {
            let mut t = String::from("<string>");
            escape(s, &mut t);
            t.push_str("</string>");
            line(out, depth, &t);
        }
        PyValue::Bool(b) => line(out, depth, if *b { "<true/>" } else { "<false/>" }),
        PyValue::Int(i) => line(out, depth, &format!("<integer>{i}</integer>")),
        PyValue::Real(f) => line(
            out,
            depth,
            &format!("<real>{}</real>", nxb_pyjson::repr(*f)),
        ),
        PyValue::List(l) if l.is_empty() => line(out, depth, "<array/>"),
        PyValue::List(l) => {
            line(out, depth, "<array>");
            for x in l {
                write(x, depth + 1, out);
            }
            line(out, depth, "</array>");
        }
        PyValue::Dict(d) if d.is_empty() => line(out, depth, "<dict/>"),
        PyValue::Dict(d) => {
            line(out, depth, "<dict>");
            let mut keys: Vec<&(String, PyValue)> = d.iter().collect();
            keys.sort_by(|a, b| a.0.cmp(&b.0));
            for (k, x) in keys {
                let mut t = String::from("<key>");
                escape(k, &mut t);
                t.push_str("</key>");
                line(out, depth + 1, &t);
                write(x, depth + 1, out);
            }
            line(out, depth, "</dict>");
        }
    }
}

/// `plistlib.dumps(v)`.
pub fn dumps(v: &PyValue) -> Vec<u8> {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n",
    );
    write(v, 0, &mut out);
    out.push_str("</plist>\n");
    out.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_like_plistlib() {
        let v = PyValue::Dict(vec![
            (
                "b".into(),
                PyValue::List(vec![PyValue::Real(3.0), PyValue::Int(2)]),
            ),
            ("a".into(), PyValue::Str("x & <y>".into())),
            ("c".into(), PyValue::List(vec![])),
            ("d".into(), PyValue::Bool(false)),
        ]);
        let s = String::from_utf8(dumps(&v)).unwrap_or_default();
        assert!(s.ends_with("<dict>\n\t<key>a</key>\n\t<string>x &amp; &lt;y&gt;</string>\n\t<key>b</key>\n\t<array>\n\t\t<real>3.0</real>\n\t\t<integer>2</integer>\n\t</array>\n\t<key>c</key>\n\t<array/>\n\t<key>d</key>\n\t<false/>\n</dict>\n</plist>\n"));
    }
}
