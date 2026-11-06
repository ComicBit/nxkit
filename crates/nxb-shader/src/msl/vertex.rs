//! Vertex shader steps of `dmsh_build.finish` (ports of `int_inputs`, `vs_init`, `vs_layer`, `vs_base_instance`,
//! `vs_pad`, `vs_depth`, `vs_writes`, `vs_discard`, `vs_layer_entry`.`dmsh_geometry.capture` is in `capture.rs`).

use super::text::{is_word, word_starts};
use nxb_pyjson::Value;
use std::collections::BTreeSet;

pub(super) fn word_end(s: &str, i: usize) -> usize {
    s[i..]
        .char_indices()
        .find(|(_, c)| !is_word(*c))
        .map_or(s.len(), |(k, _)| i + k)
}

pub(super) fn prev_word(s: &str, i: usize) -> bool {
    s[..i].chars().next_back().is_some_and(is_word)
}

/// `(\w+)\s+(\w+)\s*\[\[attribute\((\d+)\)\]\]` → (name, location) in order.
fn attributes(src: &str) -> Vec<(String, i64)> {
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
        if let Ok(l) = r[..d].parse() {
            out.push((before[ns..].to_string(), l));
        }
    }
    out
}

fn line_at(body: &str, a: usize, b: usize) -> &str {
    let ls = body[..a].rfind('\n').map_or(0, |k| k + 1);
    match body[b..].find('\n') {
        Some(k) => &body[ls..b + k],
        // Python's find returns -1: the slice drops the last character.
        None => &body[ls..body.len().saturating_sub(1).max(ls)],
    }
}

const CASTS: [&str; 2] = ["as_type<int>(", "as_type<uint>("];

/// `int_inputs(src)` → `{location: "uint"}` in Python's order.
pub fn int_inputs(src: &str) -> Vec<(String, String)> {
    let mut ins: Vec<(String, i64)> = Vec::new();
    for (n, l) in attributes(src) {
        match ins.iter_mut().find(|(x, _)| *x == n) {
            Some(slot) => slot.1 = l,
            None => ins.push((n, l)),
        }
    }
    let Some(i) = src.find("vertex main0_out main0(") else {
        return Vec::new();
    };
    let body = &src[i..];
    let mut out = Vec::new();
    for (name, loc) in ins {
        let pat = format!("in.{name}");
        let (mut nint, mut bad) = (0, false);
        for (u, _) in body.match_indices(&pat) {
            let ue = u + pat.len();
            if body[ue..].chars().next().is_some_and(is_word) {
                continue;
            }
            let line = line_at(body, u, ue);
            let decl = (|| {
                let t = line.trim_start().strip_prefix("float")?;
                let t2 = t.trim_start();
                if t2.len() == t.len() || !t2.starts_with('_') {
                    return None;
                }
                let e = word_end(t2, 0);
                let v = &t2[..e];
                let r = t2[e..].trim_start().strip_prefix('=')?.trim_start();
                let r = r.strip_prefix(&format!("in.{name}."))?;
                let c = r.chars().next()?;
                ("xyzw".contains(c) && &r[1..] == ";").then(|| v.to_string())
            })();
            let Some(v) = decl else {
                if CASTS.iter().any(|c| body[..u].ends_with(c)) {
                    nint += 1;
                    continue;
                }
                bad = true;
                break;
            };
            let occ: Vec<usize> = body
                .match_indices(v.as_str())
                .map(|(k, _)| k)
                .filter(|k| {
                    !prev_word(body, *k) && !body[k + v.len()..].chars().next().is_some_and(is_word)
                })
                .collect();
            for o in occ.into_iter().skip(1) {
                if CASTS.iter().any(|c| body[..o].ends_with(c)) {
                    nint += 1;
                    continue;
                }
                let l2 = line_at(body, o, o + v.len());
                let copy = (|| {
                    let t = l2.trim_start();
                    let e = t
                        .find(|c: char| !(is_word(c) || ".[]".contains(c)))
                        .unwrap_or(t.len());
                    if e == 0 {
                        return None;
                    }
                    let r = t[e..].trim_start().strip_prefix('=')?.trim_start();
                    (r == format!("{v};")).then_some(())
                })();
                if copy.is_some() {
                    continue;
                }
                bad = true;
                break;
            }
            if bad {
                break;
            }
        }
        if !bad && nint > 0 {
            out.push((loc.to_string(), "uint".to_string()));
        }
    }
    out
}

fn out_struct_span(src: &str) -> Option<(usize, usize, usize)> {
    let i = src.find("struct main0_out\n{\n")?;
    let j = i + src[i..].find("\n};")?;
    let k = src.find("main0_out out = {};")?;
    Some((i, j, k))
}

/// `vs_init(src)`.
pub fn vs_init(src: &str) -> String {
    let Some((i, j, k)) = out_struct_span(src) else {
        return src.to_string();
    };
    let mut outs = Vec::new();
    let st = &src[i..j];
    for (p, m) in st.match_indices("\n    float4 ") {
        let r = &st[p + m.len()..];
        let e = word_end(r, 0);
        if e == 0 {
            continue;
        }
        if let Some(x) = r[e..].strip_prefix(" [[user(locn") {
            let d = x.bytes().take_while(u8::is_ascii_digit).count();
            if d > 0 && x[d..].starts_with(")]];") {
                outs.push(r[..e].to_string());
            }
        }
    }
    let k = k + "main0_out out = {};".len();
    let ins: String = outs
        .iter()
        .map(|n| format!("\n    out.{n} = float4(0.0, 0.0, 0.0, 1.0);"))
        .collect();
    format!("{}{ins}{}", &src[..k], &src[k..])
}

fn drop_point_size_writes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut at = 0;
    for (p, m) in s.match_indices("\n    out.gl_PointSize = ") {
        if p < at {
            continue;
        }
        let r = &s[p + m.len()..];
        if let Some(k) = r.find([';', '\n'])
            && r[k..].starts_with(';')
        {
            out.push_str(&s[at..p]);
            at = p + m.len() + k + 1;
        }
    }
    out.push_str(&s[at..]);
    out
}

fn ws(r: &str) -> Option<&str> {
    let t = r.trim_start();
    (t.len() < r.len()).then_some(t)
}

/// The last `\bvertex\s+\w+\s+\w+\s*\(` match that starts before `use_at`: (start, end of the match).
fn vertex_fn_before(s: &str, use_at: usize) -> Option<(usize, usize)> {
    let word = |r: &str| {
        let n = r.len() - r.trim_start_matches(is_word).len();
        (n > 0).then_some(n)
    };
    let (mut best, mut at) = (None, 0);
    for i in word_starts(s, "vertex") {
        if i >= use_at || i < at {
            continue;
        }
        let Some(r) = ws(&s[i + 6..]) else { continue };
        let Some(n) = word(r) else { continue };
        let Some(r) = ws(&r[n..]) else { continue };
        let Some(n) = word(r) else { continue };
        let r = &r[n..];
        let t = r.trim_start();
        if t.starts_with('(') {
            at = s.len() - t.len() + 1;
            best = Some((i, at));
        }
    }
    best
}

/// `drop_point_size(src)`: the writes of the point size go. A function that takes the output by reference still needs
/// something to write: the vertex function gets a local float for it.
fn drop_point_size(src: &str) -> Result<String, String> {
    let mut src = drop_point_size_writes(src);
    while let Some(use_at) = src.find("out.gl_PointSize") {
        let (_, aend) = vertex_fn_before(&src, use_at)
            .ok_or("[RED] a point size output used outside a vertex function")?;
        let b = src[aend..]
            .find('{')
            .map(|k| aend + k)
            .ok_or("[RED] a vertex function without a body")?;
        let (mut depth, mut e) = (0i64, src.len() - 1);
        for (k, ch) in src[b..].char_indices() {
            depth += i64::from(ch == '{') - i64::from(ch == '}');
            if depth == 0 {
                e = b + k;
                break;
            }
        }
        src = format!(
            "{}\n    float dmsh_point_size = 1.0;{}{}",
            &src[..=b],
            src[b + 1..e].replace("out.gl_PointSize", "dmsh_point_size"),
            &src[e..]
        );
    }
    Ok(src)
}

/// `vs_layer(src)`.
pub fn vs_layer(src: &str) -> Result<String, String> {
    let ok = |s: &str| Ok(s.to_string());
    let Some(i) = src.find("struct main0_out\n{\n") else {
        return ok(src);
    };
    let Some(j) = src[i..].find("\n};").map(|k| i + k) else {
        return ok(src);
    };
    let st = &src[i..j];
    if !st.contains("[[render_target_array_index]]") || !st.contains("[[point_size]]") {
        return ok(src);
    }
    let st2 = st.replacen("\n    float gl_PointSize [[point_size]];", "", 1);
    drop_point_size(&format!("{}{st2}{}", &src[..i], &src[j..]))
}

const VP_BASE_INSTANCE: &str = "(*spvDescriptorSet0.vp_c0)._m0[100].y";

/// `vs_base_instance(src)`.
pub fn vs_base_instance(src: &str) -> String {
    if !src.contains(VP_BASE_INSTANCE) {
        return src.to_string();
    }
    let mut s = src.replace(VP_BASE_INSTANCE, "as_type<float>(gl_BaseInstance)");
    // \bvertex\s+\w+\s+main0\s*\((?:[^()]|\([^()]*\))*\): first match.
    let mut from = 0;
    while let Some(off) = s[from..].find("vertex") {
        let i = from + off;
        from = i + 1;
        if prev_word(&s, i) {
            continue;
        }
        let r = &s[i + 6..];
        let a = r.trim_start();
        if a.len() == r.len() {
            continue;
        }
        let w = word_end(a, 0);
        if w == 0 {
            continue;
        }
        let b = a[w..].trim_start();
        if b.len() == a[w..].len() {
            continue;
        }
        let Some(c) = b.strip_prefix("main0") else {
            continue;
        };
        let c = c.trim_start();
        if !c.starts_with('(') {
            continue;
        }
        let open = s.len() - c.len();
        // One level of nested parentheses only.
        let (mut j, mut depth) = (open + 1, 0);
        let bytes = s.as_bytes();
        let mut close = None;
        while j < bytes.len() {
            match bytes[j] {
                b'(' if depth == 0 => depth = 1,
                b'(' => break,
                b')' if depth == 1 => depth = 0,
                b')' => {
                    close = Some(j);
                    break;
                }
                _ => {}
            }
            j += 1;
        }
        let Some(close) = close else { continue };
        if !s[i..=close].contains("[[base_instance]]") {
            s.insert_str(close, ", uint gl_BaseInstance [[base_instance]]");
        }
        break;
    }
    s
}

/// `[[user(locnN)` numbers in a source.
fn locns(s: &str) -> BTreeSet<i64> {
    s.match_indices("[[user(locn")
        .filter_map(|(i, m)| {
            let r = &s[i + m.len()..];
            let d = r.bytes().take_while(u8::is_ascii_digit).count();
            (d > 0).then(|| r[..d].parse().ok()).flatten()
        })
        .collect()
}

/// `vs_pad(key, src)` with the paired pixel shaders' final sources → (source, padded locations).
pub fn vs_pad(src: &str, pixel_sources: &[String]) -> (String, Vec<i64>) {
    let mut want = BTreeSet::new();
    for p in pixel_sources {
        want.extend(locns(p));
    }
    let have = locns(src);
    let miss: Vec<i64> = want.difference(&have).copied().collect();
    let Some((_, j, _)) = out_struct_span(src) else {
        return (src.to_string(), vec![]);
    };
    if miss.is_empty() {
        return (src.to_string(), vec![]);
    }
    let decls: String = miss
        .iter()
        .map(|l| format!("\n    float4 dm_pad{l} [[user(locn{l})]];"))
        .collect();
    let s = format!("{}{decls}{}", &src[..j], &src[j..]);
    let k = s
        .find("main0_out out = {};")
        .map_or(0, |k| k + "main0_out out = {};".len());
    let inits: String = miss
        .iter()
        .map(|l| format!("\n    out.dm_pad{l} = float4(0.0, 0.0, 0.0, 1.0);"))
        .collect();
    (format!("{}{inits}{}", &s[..k], &s[k..]), miss)
}

/// `\bvertex\s+main0_out\s+NAME\s*\(` → (start, end after the paren).
fn vertex_fn(s: &str, name: &str) -> Option<(usize, usize)> {
    for (i, _) in s.match_indices("vertex") {
        if prev_word(s, i) {
            continue;
        }
        let r = &s[i + 6..];
        let a = r.trim_start();
        if a.len() == r.len() {
            continue;
        }
        let Some(b) = a.strip_prefix("main0_out") else {
            continue;
        };
        let c = b.trim_start();
        if c.len() == b.len() {
            continue;
        }
        let Some(d) = c.strip_prefix(name) else {
            continue;
        };
        let e = d.trim_start();
        if e.starts_with('(') {
            return Some((i, s.len() - e.len() + 1));
        }
    }
    None
}

/// `vs_depth(src)` → (source with only main0, the full source with main0_z).
pub fn vs_depth(src: &str) -> Result<(String, String), String> {
    let (start, _) = vertex_fn(src, "main0").ok_or(
        "[RED] vertex shader without a main0 main function, cannot add the depth entry points",
    )?;
    let body = &src[start..];
    let gl = body.replace(
        "return out;",
        "out.gl_Position.z = fma(out.gl_Position.z, 0.5, out.gl_Position.w * 0.5);\n    return out;",
    );
    let single = format!("{}{gl}", &src[..start]);
    // re.sub(r'\bmain0\s*\(', 'main0_z(', body, count=1)
    let mut z = body.to_string();
    for (i, _) in body.match_indices("main0") {
        if prev_word(body, i) {
            continue;
        }
        let r = &body[i + 5..];
        let t = r.trim_start();
        if let Some(after) = t.strip_prefix('(') {
            z = format!("{}main0_z({after}", &body[..i]);
            break;
        }
    }
    Ok((single.clone(), format!("{single}\n{z}")))
}

/// `vs_writes(src, binds)`.
pub fn vs_writes(src: &str, binds: &Value) -> bool {
    let rw = binds
        .as_list()
        .is_some_and(|l| l.iter().any(|b| b.get("rw") == Some(&Value::Bool(true))));
    rw || ["access::write", "access::read_write"].iter().any(|p| {
        src.match_indices(p)
            .any(|(i, m)| !src[i + m.len()..].chars().next().is_some_and(is_word))
    })
}

/// `re.search(r'\bvertex\s+main0_out\s+main0\s*\((.*?)\)\s*\{', src, re.S)` → (params, index after `{`).
fn main_params(src: &str) -> Option<(&str, usize)> {
    let (_, open) = vertex_fn(src, "main0")?;
    let mut k = open;
    while let Some(off) = src[k..].find(')') {
        let c = k + off;
        let r = src[c + 1..].trim_start();
        if r.starts_with('{') {
            return Some((&src[open..c], src.len() - r.len() + 1));
        }
        k = c + 1;
    }
    None
}

/// `vs_discard(src)`.
pub fn vs_discard(src: &str) -> Result<String, String> {
    let (params, end) =
        main_params(src).ok_or("vertex shader has no main0 entry for rasterizer discard")?;
    Ok(format!(
        "\nvertex void main0_discard({params})\n{{{}",
        src[end..].replace("return out;", "return;")
    ))
}

/// `vs_layer_entry(full)` → (source, `{loc, comp}` or `None`).
pub fn vs_layer_entry(full: &str) -> Result<(String, Option<Value>), String> {
    let none = (full.to_string(), None);
    let Some(i) = full.find("struct main0_out\n{\n") else {
        return Ok(none);
    };
    let Some(j) = full[i..].find("\n};").map(|k| i + k) else {
        return Ok(none);
    };
    if !full.contains("gl_InstanceIndex") || full[i..j].contains("[[render_target_array_index]]") {
        return Ok(none);
    }
    let st = &full[i..j];
    let mut locs: Vec<(String, i64)> = Vec::new();
    for (p, m) in st.match_indices("\n    float4 ") {
        let r = &st[p + m.len()..];
        let e = word_end(r, 0);
        if let Some(x) = r[e..].strip_prefix(" [[user(locn") {
            let d = x.bytes().take_while(u8::is_ascii_digit).count();
            if e > 0 && d > 0 && x[d..].starts_with(")]];") {
                locs.push((r[..e].to_string(), x[..d].parse().unwrap_or(0)));
            }
        }
    }
    let hit = full.match_indices("\n    out.").find_map(|(p, m)| {
        let r = &full[p + m.len()..];
        let e = word_end(r, 0);
        let name = &r[..e];
        let c = r[e..].strip_prefix('.')?.chars().next()?;
        let ok = "xyzw".contains(c)
            && r[e + 2..].starts_with(" = as_type<float>(")
            && locs.iter().any(|(n, _)| n == name);
        ok.then(|| (name.to_string(), c))
    });
    let Some((name, comp)) = hit else {
        return Ok(none);
    };
    let st_full = &full[i..j + 3];
    let st2 = st_full
        .replace("\n    float gl_PointSize [[point_size]];", "")
        .replacen("struct main0_out", "struct main0l_out", 1);
    let st2 = format!(
        "{}\n    uint gl_Layer [[render_target_array_index]];\n}};",
        &st2[..st2.len() - 3]
    );
    let mut add = format!("\n{st2}\n");
    for (ep, new) in [("main0", "main0_l"), ("main0_z", "main0_lz")] {
        let Some((a, aend)) = vertex_fn(full, ep) else {
            return Ok(none);
        };
        let Some(b) = full[aend..].find('{').map(|k| aend + k) else {
            return Ok(none);
        };
        let mut depth = 0i64;
        let mut e = b;
        for (k, ch) in full[b..].char_indices() {
            depth += i64::from(ch == '{') - i64::from(ch == '}');
            if depth == 0 {
                e = b + k;
                break;
            }
        }
        let f = full[a..=e].replace("main0_out", "main0l_out").replacen(
            &format!(" {ep}("),
            &format!(" {new}("),
            1,
        );
        let f = drop_point_size(&f)?;
        add.push_str(&format!(
            "\n{}\n",
            f.replace(
                "return out;",
                &format!("out.gl_Layer = as_type<uint>(out.{name}.{comp});\n    return out;")
            )
        ));
    }
    let loc = locs.iter().find(|(n, _)| *n == name).map_or(0, |x| x.1);
    Ok((
        format!("{full}{add}"),
        Some(Value::obj([
            ("loc", Value::Int(loc)),
            ("comp", Value::str(comp.to_string())),
        ])),
    ))
}
