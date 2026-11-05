//! `raster_facts('vs', src)`.

use super::is_word;
use super::screen::{screen_varyings, word_end};
use nxb_pyjson::Value;
use std::collections::BTreeMap;

/// An fma of an attribute component: (attribute member and component, scale, offset uniforms).
type Affine = ((String, char), [i64; 3], [i64; 3]);

/// `raster_facts('vs', src)`.
pub fn raster_vs(src: &str) -> Value {
    let scr = Value::List(screen_varyings(src).into_iter().map(Value::Int).collect());
    let unprovable = || Value::obj([("pos_attr", Value::Int(-1)), ("screen", scr.clone())]);
    // \bvertex\s+\w+\s+\w+\s*\(.*?\)\s*\{(.*)\}\s*$: the first vertex function's body to the last brace.
    let Some(body) = first_vertex_body(src) else {
        return unprovable();
    };
    let kw = ["if", "for", "while", "switch", "do"].iter().any(|k| {
        body.match_indices(k).any(|(i, _)| {
            !body[..i].chars().next_back().is_some_and(is_word)
                && !body[i + k.len()..].chars().next().is_some_and(is_word)
        })
    });
    if kw || body.contains("clip_distance") || body.contains("ClipDistance") {
        return unprovable();
    }
    let b = body;
    let attr: Vec<(String, i64)> = attrs(src);
    let attr_of = |n: &str| attr.iter().rev().find(|(a, _)| a == n).map(|x| x.1);
    // var: float\s+(_\w+)\s*=\s*in\.(\w+)\.([xyzw]);
    let mut var: BTreeMap<String, (String, char)> = BTreeMap::new();
    for (i, _) in b.match_indices("float") {
        let r = &b[i + 5..];
        let t = r.trim_start();
        if t.len() == r.len() || !t.starts_with('_') {
            continue;
        }
        let e = word_end(t, 0);
        let Some(x) = t[e..].trim_start().strip_prefix('=') else {
            continue;
        };
        let Some(x) = x.trim_start().strip_prefix("in.") else {
            continue;
        };
        let me = word_end(x, 0);
        let Some(y) = x[me..].strip_prefix('.') else {
            continue;
        };
        let Some(c) = y.chars().next() else { continue };
        if me > 0 && "xyzw".contains(c) && y[1..].starts_with(';') {
            var.insert(t[..e].to_string(), (x[..me].to_string(), c));
        }
    }
    let assigns = |prefix: &str| -> Vec<(String, char, String)> {
        let mut out = Vec::new();
        for (i, m) in b.match_indices(prefix) {
            let r = &b[i + m.len()..];
            let e = word_end(r, 0);
            let Some(x) = r[e..].strip_prefix('.') else {
                continue;
            };
            let Some(c) = x.chars().next() else { continue };
            if !"xyzw".contains(c) {
                continue;
            }
            let Some(v) = x[1..].trim_start().strip_prefix('=') else {
                continue;
            };
            if v.starts_with('=') {
                continue;
            }
            let Some(k) = v.find(';') else { continue };
            let v = v[..k].trim();
            if k > 0 && !v.is_empty() {
                out.push((r[..e].to_string(), c, v.to_string()));
            }
        }
        out
    };
    let mut last: BTreeMap<char, String> = BTreeMap::new();
    for (mem, c, v) in assigns("out.") {
        if mem == "gl_Position" {
            last.insert(c, v);
        }
    }
    let src_of = |c: char| last.get(&c).and_then(|v| var.get(v)).cloned();
    let Some(m0) = src_of('x') else {
        return unprovable();
    };
    if attr_of(&m0.0).is_none()
        || ['x', 'y', 'z']
            .iter()
            .any(|c| src_of(*c) != Some((m0.0.clone(), *c)))
    {
        return unprovable();
    }
    let w = if last.get(&'w').map(String::as_str) == Some("1.0") {
        "one"
    } else if src_of('w') == Some((m0.0.clone(), 'w')) {
        "attr"
    } else {
        return unprovable();
    };
    let ost = crate::msl::out_group(src).map_or("", |(a, b)| &src[a..b]);
    let mut locn: Vec<(String, i64)> = Vec::new();
    for (i, _) in ost.match_indices("m_") {
        if ost[..i].chars().next_back().is_some_and(is_word) {
            continue;
        }
        let e = word_end(ost, i);
        if !ost[i + 2..e].bytes().all(|c| c.is_ascii_digit()) || e == i + 2 {
            continue;
        }
        if let Some(x) = ost[e..].strip_prefix(" [[user(locn") {
            let d = x.bytes().take_while(u8::is_ascii_digit).count();
            if d > 0 && x[d..].starts_with(")]]") {
                let n = ost[i..e].to_string();
                let l = x[..d].parse().unwrap_or(0);
                match locn.iter_mut().find(|(a, _)| *a == n) {
                    Some(s) => s.1 = l,
                    None => locn.push((n, l)),
                }
            }
        }
    }
    let mut olast: BTreeMap<(String, char), String> = BTreeMap::new();
    for (mem, c, v) in assigns("out.") {
        if mem.starts_with("m_") && mem.len() > 2 && mem[2..].bytes().all(|x| x.is_ascii_digit()) {
            olast.insert((mem, c), v);
        }
    }
    let fm = |v: &str| -> Option<Affine> {
        let pat = format!("float {v} = fma(");
        let r = b
            .match_indices(&pat)
            .map(|(i, _)| &b[i + pat.len()..])
            .next()?;
        let e = word_end(r, 0);
        let a = &r[..e];
        if !a.starts_with('_') {
            return None;
        }
        let mut rest = &r[e..];
        let uni = |rest: &mut &str| -> Option<[i64; 3]> {
            let x = rest.strip_prefix(", (*spvDescriptorSet0.vp_c")?;
            let d = x.bytes().take_while(u8::is_ascii_digit).count();
            let slot: i64 = x[..d].parse().ok()?;
            let x = x[d..].strip_prefix(")._m0[")?;
            let d2 = x.bytes().take_while(u8::is_ascii_digit).count();
            let vec: i64 = x[..d2].parse().ok()?;
            let x = x[d2..].strip_prefix("].")?;
            let c = "xyzw".find(x.chars().next()?)? as i64;
            *rest = &x[1..];
            Some([slot - 3, vec, c])
        };
        let s1 = uni(&mut rest)?;
        let s2 = uni(&mut rest)?;
        if !rest.starts_with(");") {
            return None;
        }
        Some((var.get(a)?.clone(), s1, s2))
    };
    let mut uv: Vec<(String, Value)> = Vec::new();
    let mut aff: Vec<(String, Value)> = Vec::new();
    for (mem, k) in &locn {
        let ox = olast.get(&(mem.clone(), 'x')).cloned().unwrap_or_default();
        let oy = olast.get(&(mem.clone(), 'y')).cloned().unwrap_or_default();
        let (sx, sy) = (var.get(&ox), var.get(&oy));
        if let (Some(sx), Some(sy)) = (sx, sy)
            && sx.0 == sy.0
            && let Some(a) = attr_of(&sx.0)
            && sx.1 == 'x'
            && sy.1 == 'y'
        {
            uv.push((k.to_string(), Value::Int(a)));
            continue;
        }
        if let (Some(fx), Some(fy)) = (fm(&ox), fm(&oy))
            && fx.0.0 == fy.0.0
            && let Some(a) = attr_of(&fx.0.0)
            && fx.0.1 == 'x'
            && fy.0.1 == 'y'
            && fx.1[0].min(fx.2[0]).min(fy.1[0]).min(fy.2[0]) >= 0
        {
            let l = |v: [i64; 3]| Value::List(v.iter().map(|x| Value::Int(*x)).collect());
            aff.push((
                k.to_string(),
                Value::obj([
                    ("attr", Value::Int(a)),
                    ("sx", l(fx.1)),
                    ("ox", l(fx.2)),
                    ("sy", l(fy.1)),
                    ("oy", l(fy.2)),
                ]),
            ));
        }
    }
    Value::obj([
        ("pos_attr", Value::Int(attr_of(&m0.0).unwrap_or(-1))),
        ("pos_w", Value::str(w)),
        ("uv_from_attr", Value::Object(uv)),
        ("uv_affine", Value::Object(aff)),
        ("screen", scr),
    ])
}

/// `(\w+)\s+(\w+)\s*\[\[attribute\((\d+)\)\]\]` → (name, location).
fn attrs(src: &str) -> Vec<(String, i64)> {
    let mut out = Vec::new();
    for (i, m) in src.match_indices("[[attribute(") {
        let r = &src[i + m.len()..];
        let d = r.bytes().take_while(u8::is_ascii_digit).count();
        if d == 0 || !r[d..].starts_with(")]]") {
            continue;
        }
        let before = src[..i].trim_end();
        let ns = before
            .char_indices()
            .rev()
            .take_while(|(_, c)| is_word(*c))
            .last()
            .map(|(k, _)| k);
        let Some(ns) = ns else { continue };
        let gap = &before[..ns];
        if gap.trim_end().len() == gap.len()
            || !gap.trim_end().chars().next_back().is_some_and(is_word)
        {
            continue;
        }
        out.push((before[ns..].to_string(), r[..d].parse().unwrap_or(0)));
    }
    out
}

fn first_vertex_body(src: &str) -> Option<&str> {
    for (i, _) in src.match_indices("vertex") {
        if src[..i].chars().next_back().is_some_and(is_word) {
            continue;
        }
        let r = &src[i + 6..];
        let a = r.trim_start();
        if a.len() == r.len() {
            continue;
        }
        let w = word_end(a, 0);
        let b = a[w..].trim_start();
        if w == 0 || b.len() == a[w..].len() {
            continue;
        }
        let w2 = word_end(b, 0);
        let c = b[w2..].trim_start();
        if w2 == 0 || !c.starts_with('(') {
            continue;
        }
        let open = src.len() - c.len() + 1;
        let mut k = open;
        while let Some(off) = src[k..].find(')') {
            let cl = k + off;
            let r2 = src[cl + 1..].trim_start();
            if r2.starts_with('{') {
                let bs = src.len() - r2.len() + 1;
                let end = src.trim_end();
                if end.ends_with('}') && end.len() > bs {
                    return Some(&src[bs..end.len() - 1]);
                }
                return None;
            }
            k = cl + 1;
        }
    }
    None
}
