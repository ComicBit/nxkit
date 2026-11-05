//! `json.dumps(v, ensure_ascii=False)` and `json.dumps(v, ensure_ascii=False, indent=n)`.

use crate::float::repr;
use crate::value::Value;

fn string(out: &mut String, s: &str) {
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
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn float(out: &mut String, f: f64) {
    if f.is_nan() {
        out.push_str("NaN");
    } else if f.is_infinite() {
        out.push_str(if f > 0.0 { "Infinity" } else { "-Infinity" });
    } else {
        out.push_str(&repr(f));
    }
}

fn write(out: &mut String, v: &Value, indent: Option<usize>, level: usize) {
    let nl = |out: &mut String, lv: usize| {
        if let Some(n) = indent {
            out.push('\n');
            out.push_str(&" ".repeat(n * lv));
        }
    };
    let sep = if indent.is_some() { "," } else { ", " };
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Float(f) => float(out, *f),
        Value::Str(s) => string(out, s),
        Value::List(l) => {
            if l.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (k, x) in l.iter().enumerate() {
                if k > 0 {
                    out.push_str(sep);
                }
                nl(out, level + 1);
                write(out, x, indent, level + 1);
            }
            nl(out, level);
            out.push(']');
        }
        Value::Object(m) => {
            if m.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (k, (name, x)) in m.iter().enumerate() {
                if k > 0 {
                    out.push_str(sep);
                }
                nl(out, level + 1);
                string(out, name);
                out.push_str(": ");
                write(out, x, indent, level + 1);
            }
            nl(out, level);
            out.push('}');
        }
    }
}

/// `json.dumps(v, ensure_ascii=False)`.
pub fn dumps(v: &Value) -> String {
    let mut s = String::new();
    write(&mut s, v, None, 0);
    s
}

/// `json.dumps(v, ensure_ascii=False, indent=n)` (what `json.dump(..., indent=1)` writes into `_b.json`).
pub fn dumps_indent(v: &Value, n: usize) -> String {
    let mut s = String::new();
    write(&mut s, v, Some(n), 0);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_examples() {
        let v = Value::obj([
            (
                "a",
                Value::List(vec![
                    Value::Int(1),
                    Value::Float(2.5),
                    Value::Float(1e-05),
                    Value::Float(f64::INFINITY),
                ]),
            ),
            (
                "b",
                Value::obj([("c", Value::Null), ("d", Value::Bool(true))]),
            ),
            ("é", Value::str("x\u{2028}\"\n")),
        ]);
        assert_eq!(
            dumps_indent(&v, 1),
            "{\n \"a\": [\n  1,\n  2.5,\n  1e-05,\n  Infinity\n ],\n \"b\": {\n  \"c\": null,\n  \"d\": true\n },\n \"é\": \"x\u{2028}\\\"\\n\"\n}"
        );
        let w = Value::obj([
            ("a", Value::List(vec![Value::Int(1), Value::Object(vec![])])),
            ("e", Value::List(vec![])),
        ]);
        assert_eq!(dumps(&w), "{\"a\": [1, {}], \"e\": []}");
    }
}
