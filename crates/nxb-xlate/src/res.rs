//! `res.json`: the resource summary next to each SPIR-V file, in the `System.Text.Json` indented format.

use nxb_pyjson::Value;

/// A constant buffer binding.
#[derive(Clone, Debug, PartialEq)]
pub struct CBuf {
    pub binding: i64,
    pub slot: i64,
}

/// A storage buffer binding.
#[derive(Clone, Debug, PartialEq)]
pub struct SBuf {
    pub binding: i64,
    pub slot: i64,
    pub cb_slot: i64,
    pub cb_offset: i64,
    pub flags: i64,
}

/// A texture or image binding.
#[derive(Clone, Debug, PartialEq)]
pub struct Tex {
    pub binding: i64,
    pub ty: i64,
    pub format: String,
    pub cbuf_slot: i64,
    pub handle: i64,
    pub array: i64,
    pub separate: bool,
    pub flags: i64,
}

/// The whole file.
#[derive(Clone, Debug, PartialEq)]
pub struct Res {
    pub key: String,
    pub stage: String,
    pub notes: Vec<String>,
    pub cbufs: Vec<CBuf>,
    pub sbufs: Vec<SBuf>,
    pub textures: Vec<Tex>,
    pub images: Vec<Tex>,
    pub geometry_stride: i64,
    pub uses_frag_coord: bool,
    pub frag_out_map: i64,
    pub fetch1d: Vec<i64>,
}

/// A JSON string as `System.Text.Json` writes it with the default encoder.
fn string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\u0022"),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '<' | '>' | '&' | '\'' | '+' | '`' => out.push_str(&format!("\\u{:04X}", c as u32)),
            ' '..='~' => out.push(c),
            _ => {
                let mut b = [0u16; 2];
                for u in c.encode_utf16(&mut b) {
                    out.push_str(&format!("\\u{u:04X}"));
                }
            }
        }
    }
    out.push('"');
}

fn pad(out: &mut String, n: usize) {
    for _ in 0..n {
        out.push_str("  ");
    }
}

fn write(v: &Value, d: usize, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Float(f) => out.push_str(&f.to_string()),
        Value::Str(s) => string(out, s),
        Value::List(l) if l.is_empty() => out.push_str("[]"),
        Value::List(l) => {
            out.push_str("[\n");
            for (i, x) in l.iter().enumerate() {
                if i > 0 {
                    out.push_str(",\n");
                }
                pad(out, d + 1);
                write(x, d + 1, out);
            }
            out.push('\n');
            pad(out, d);
            out.push(']');
        }
        Value::Object(m) if m.is_empty() => out.push_str("{}"),
        Value::Object(m) => {
            out.push_str("{\n");
            for (i, (k, x)) in m.iter().enumerate() {
                if i > 0 {
                    out.push_str(",\n");
                }
                pad(out, d + 1);
                string(out, k);
                out.push_str(": ");
                write(x, d + 1, out);
            }
            out.push('\n');
            pad(out, d);
            out.push('}');
        }
    }
}

/// `JsonSerializerOptions { WriteIndented = true }` output of a value.
pub fn to_json_string(v: &Value) -> String {
    let mut s = String::new();
    write(v, 0, &mut s);
    s
}

fn i(v: i64) -> Value {
    Value::Int(v)
}

fn tex(t: &Tex) -> Value {
    Value::obj([
        ("binding", i(t.binding)),
        ("type", i(t.ty)),
        ("format", Value::str(&t.format)),
        ("cbuf_slot", i(t.cbuf_slot)),
        ("handle", i(t.handle)),
        ("array", i(t.array)),
        ("separate", Value::Bool(t.separate)),
        ("flags", i(t.flags)),
    ])
}

impl Res {
    /// The value in `dmsh_xlate`'s member order.
    pub fn to_value(&self) -> Value {
        let mut v = Value::obj([
            ("key", Value::str(&self.key)),
            ("stage", Value::str(&self.stage)),
            (
                "notes",
                Value::List(self.notes.iter().map(Value::str).collect()),
            ),
            (
                "cbufs",
                Value::List(
                    self.cbufs
                        .iter()
                        .map(|b| Value::obj([("binding", i(b.binding)), ("slot", i(b.slot))]))
                        .collect(),
                ),
            ),
            (
                "sbufs",
                Value::List(
                    self.sbufs
                        .iter()
                        .map(|b| {
                            Value::obj([
                                ("binding", i(b.binding)),
                                ("slot", i(b.slot)),
                                ("cb_slot", i(b.cb_slot)),
                                ("cb_offset", i(b.cb_offset)),
                                ("flags", i(b.flags)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "textures",
                Value::List(self.textures.iter().map(tex).collect()),
            ),
            ("images", Value::List(self.images.iter().map(tex).collect())),
            ("geometry_stride", i(self.geometry_stride)),
            ("uses_frag_coord", Value::Bool(self.uses_frag_coord)),
            ("frag_out_map", i(self.frag_out_map)),
        ]);
        if !self.fetch1d.is_empty() {
            v.set(
                "fetch1d",
                Value::List(self.fetch1d.iter().map(|&h| i(h)).collect()),
            );
        }
        v
    }

    /// The file text.
    pub fn to_text(&self) -> String {
        to_json_string(&self.to_value())
    }

    /// Read a `res.json` value back (tests and the differ).
    pub fn from_value(v: &Value) -> Option<Res> {
        let n = |x: &Value, k: &str| x.get(k).and_then(Value::as_i64);
        let list = |k: &str| v.get(k).and_then(Value::as_list);
        let tex = |l: &[Value]| -> Option<Vec<Tex>> {
            l.iter()
                .map(|t| {
                    Some(Tex {
                        binding: n(t, "binding")?,
                        ty: n(t, "type")?,
                        format: t.get("format")?.as_str()?.to_string(),
                        cbuf_slot: n(t, "cbuf_slot")?,
                        handle: n(t, "handle")?,
                        array: n(t, "array")?,
                        separate: matches!(t.get("separate")?, Value::Bool(true)),
                        flags: n(t, "flags")?,
                    })
                })
                .collect()
        };
        Some(Res {
            key: v.get("key")?.as_str()?.to_string(),
            stage: v.get("stage")?.as_str()?.to_string(),
            notes: list("notes")?
                .iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect(),
            cbufs: list("cbufs")?
                .iter()
                .map(|b| {
                    Some(CBuf {
                        binding: n(b, "binding")?,
                        slot: n(b, "slot")?,
                    })
                })
                .collect::<Option<_>>()?,
            sbufs: list("sbufs")?
                .iter()
                .map(|b| {
                    Some(SBuf {
                        binding: n(b, "binding")?,
                        slot: n(b, "slot")?,
                        cb_slot: n(b, "cb_slot")?,
                        cb_offset: n(b, "cb_offset")?,
                        flags: n(b, "flags")?,
                    })
                })
                .collect::<Option<_>>()?,
            textures: tex(list("textures")?)?,
            images: tex(list("images")?)?,
            geometry_stride: n(v, "geometry_stride")?,
            uses_frag_coord: matches!(v.get("uses_frag_coord")?, Value::Bool(true)),
            frag_out_map: n(v, "frag_out_map")?,
            fetch1d: v
                .get("fetch1d")
                .and_then(Value::as_list)
                .map(|l| l.iter().filter_map(Value::as_i64).collect())
                .unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes() {
        let mut s = String::new();
        string(&mut s, "a<b>'\"é");
        assert_eq!(s, "\"a\\u003Cb\\u003E\\u0027\\u0022\\u00E9\"");
    }

    /// Round trip of every `*.res.json` of a reference output folder (`NXB_XLATE_REF`).
    #[test]
    fn oracle_round_trip() {
        let Some(root) = std::env::var_os("NXB_XLATE_REF") else {
            return;
        };
        let (mut n, mut same) = (0, 0);
        let mut dirs = vec![std::path::PathBuf::from(root)];
        while let Some(d) = dirs.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    dirs.push(p);
                } else if p.to_string_lossy().ends_with(".res.json") {
                    let text = std::fs::read_to_string(&p).unwrap();
                    let v = nxb_pyjson::parse(&text).unwrap();
                    let r = Res::from_value(&v).expect("res shape");
                    n += 1;
                    if r.to_text() == text {
                        same += 1;
                    } else {
                        eprintln!("differs: {}", p.display());
                    }
                }
            }
        }
        eprintln!("res.json writer identical: {same} of {n}");
        assert_eq!(same, n);
    }
}
