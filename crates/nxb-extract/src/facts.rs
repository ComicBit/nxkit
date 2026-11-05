//! The profile's hand-written facts (`shader_facts_manual.tsv`) merged over the generated rows, as `shader_split.py`
//! does: `facts[k] = {**facts.get(k, {}), **json.loads(v)}`, written with `json.dumps(sort_keys=True,
//! separators=(',', ':'))` (ASCII escapes).

use std::collections::BTreeMap;

use nxb_pyjson::Value;

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
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                let mut b = [0u16; 2];
                for u in c.encode_utf16(&mut b) {
                    out.push_str(&format!("\\u{u:04x}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// `json.dumps(v, sort_keys=True, separators=(',', ':'))`.
pub fn dumps_sorted(v: &Value) -> String {
    let mut out = String::new();
    write(&mut out, v);
    out
}

fn write(out: &mut String, v: &Value) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Float(f) => out.push_str(&nxb_pyjson::repr(*f)),
        Value::Str(s) => string(out, s),
        Value::List(l) => {
            out.push('[');
            for (i, x) in l.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write(out, x);
            }
            out.push(']');
        }
        Value::Object(m) => {
            let mut keys: Vec<&(String, Value)> = m.iter().collect();
            keys.sort_by(|a, b| a.0.cmp(&b.0));
            out.push('{');
            for (i, (k, x)) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                string(out, k);
                out.push(':');
                write(out, x);
            }
            out.push('}');
        }
    }
}

/// Merges the manual table's rows over `facts` (key → generated JSON text).
pub fn merge_manual(facts: &mut BTreeMap<String, String>, manual: &str) -> Result<(), String> {
    for line in manual.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let (k, v) = line
            .split_once('\t')
            .ok_or_else(|| format!("manual facts: no tab in {line:?}"))?;
        let add = nxb_pyjson::parse(v).map_err(|e| format!("manual facts {k}: {e:?}"))?;
        let mut base = match facts.get(k) {
            Some(old) => nxb_pyjson::parse(old).map_err(|e| format!("facts {k}: {e:?}"))?,
            None => Value::Object(Vec::new()),
        };
        if let (Value::Object(b), Value::Object(a)) = (&mut base, add) {
            for (name, x) in a {
                match b.iter_mut().find(|(n, _)| *n == name) {
                    Some(slot) => slot.1 = x,
                    None => b.push((name, x)),
                }
            }
        } else {
            return Err(format!("manual facts {k}: not an object"));
        }
        facts.insert(k.to_string(), dumps_sorted(&base));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merged_and_sorted() {
        let mut f = BTreeMap::new();
        f.insert(
            "a".to_string(),
            "{\"local\":[1,1,1],\"shared\":0}".to_string(),
        );
        assert!(
            merge_manual(
                &mut f,
                "# c\na\t{\"fout\": {\"0\": \"uint\"}}\nb\t{\"x\": \"\u{e9}\"}\n"
            )
            .is_ok()
        );
        assert_eq!(
            f["a"],
            "{\"fout\":{\"0\":\"uint\"},\"local\":[1,1,1],\"shared\":0}"
        );
        assert_eq!(f["b"], "{\"x\":\"\\u00e9\"}");
    }
}
