//! Text fix-ups that make OpenGL games' GLSL readable for glslang, as the reference's `gl_shaders.py` applies them.
//!
//! `es100` rewrites GLSL ES 1.00 (with `#version 100` or without a version, as GameMaker writes it) to ES 3.10, which SPIR-V
//! needs. `fixups` runs on the preprocessed text and covers what NVIDIA's compiler accepts and glslang does not: the GLSL ES
//! 1.00 texture functions, a redeclaration of `gl_Position` as a plain output and `textureOffset` with an offset that is not
//! a constant (Metal's sample offsets are constants too).

/// The byte ranges of the identifiers in `t`.
fn idents(t: &str) -> Vec<(usize, usize)> {
    let b = t.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_alphabetic() || c == b'_' {
            let s = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push((s, i));
        } else if c.is_ascii_digit() {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'.') {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    out
}

/// Replaces each identifier for which `f` gives a replacement. `f` gets the identifier and the text after it.
fn replace_idents(t: &str, mut f: impl FnMut(&str, &str) -> Option<(String, usize)>) -> String {
    let mut out = String::with_capacity(t.len());
    let mut at = 0;
    for (s, e) in idents(t) {
        if s < at {
            continue;
        }
        if let Some((new, skip)) = f(&t[s..e], &t[e..]) {
            out.push_str(&t[at..s]);
            out.push_str(&new);
            at = e + skip;
        }
    }
    out.push_str(&t[at..]);
    out
}

fn replace_word(t: &str, word: &str, new: &str) -> String {
    replace_idents(t, |w, _| (w == word).then(|| (new.to_string(), 0)))
}

/// The `#version` line: its number and its byte range with the newline.
pub(crate) fn version_line(t: &str) -> Option<(u32, std::ops::Range<usize>)> {
    let mut at = 0;
    for line in t.split_inclusive('\n') {
        let s = line.trim_start_matches([' ', '\t']);
        if let Some(r) = s.strip_prefix("#version") {
            if r.starts_with([' ', '\t']) {
                let digits: String = r.trim_start_matches([' ', '\t']).chars().take_while(char::is_ascii_digit).collect();
                if let Ok(v) = digits.parse() {
                    return Some((v, at..at + line.len()));
                }
            }
        }
        at += line.len();
    }
    None
}

/// GLSL ES 1.00 to 3.10: drops precision statements and qualifiers, `attribute` becomes `in`, `varying` `out` or `in`,
/// `gl_FragColor` an output `gm_FragColor` and `sample` (a reserved word since 3.00) `gm_sample`. `t` has no `#version` line.
pub(crate) fn es100(t: &str, stage: &str) -> String {
    let mut s = String::new();
    for line in t.split_inclusive('\n') {
        let w: Vec<&str> = line.split([' ', '\t', '\r', '\n']).filter(|x| !x.is_empty()).collect();
        let word = |x: &str| !x.is_empty() && x.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        let precision = match w.as_slice() {
            ["precision", q, last] => word(q) && last.strip_suffix(';').is_some_and(word),
            ["precision", q, t, ";"] => word(q) && word(t),
            _ => false,
        };
        if !(precision && line.ends_with('\n')) {
            s.push_str(line);
        }
    }
    let s = replace_idents(&s, |w, rest| {
        matches!(w, "lowp" | "mediump" | "highp").then(|| (String::new(), rest.len() - rest.trim_start_matches([' ', '\t']).len()))
    });
    let s = replace_word(&s, "attribute", "in");
    let mut s = replace_word(&s, "varying", if stage == "vert" { "out" } else { "in" });
    if stage == "frag" && s.contains("gl_FragColor") {
        s = format!("out vec4 gm_FragColor;\n{}", s.replace("gl_FragColor", "gm_FragColor"));
    }
    let s = replace_word(&s, "sample", "gm_sample");
    format!("#version 310 es\nprecision highp float;\nprecision highp int;\n{s}")
}

/// The arguments of the call whose `(` is at byte `open`, and the byte after its `)`.
fn call_args(t: &str, open: usize) -> Option<(Vec<&str>, usize)> {
    let mut depth = 0;
    let mut args = Vec::new();
    let mut start = open + 1;
    for (k, c) in t[open..].char_indices().map(|(k, c)| (k + open, c)) {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    args.push(&t[start..k]);
                    return Some((args, k + 1));
                }
            }
            ',' if depth == 1 => {
                args.push(&t[start..k]);
                start = k + 1;
            }
            _ => {}
        }
    }
    None
}

/// Whether an offset expression names a variable: an identifier other than the `vecN`, `ivecN` and `uvecN` constructors.
fn has_variable(o: &str) -> bool {
    idents(o).iter().any(|&(s, e)| {
        let w = &o[s..e];
        !matches!(w, "vec2" | "vec3" | "vec4" | "ivec2" | "ivec3" | "ivec4" | "uvec2" | "uvec3" | "uvec4")
    })
}

/// The fix-ups on preprocessed GLSL.
pub(crate) fn fixups(t: &str) -> Result<String, String> {
    let t = replace_idents(t, |w, rest| {
        let new = match w {
            "texture2DLod" | "textureCubeLod" => "textureLod",
            "texture2DProj" => "textureProj",
            "texture2D" | "textureCube" => "texture",
            _ => return None,
        };
        let ws = rest.len() - rest.trim_start().len();
        rest[ws..].starts_with('(').then(|| (new.to_string(), ws))
    });
    let mut s = String::with_capacity(t.len());
    for line in t.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        let w: Vec<&str> = body.split([' ', '\t']).filter(|x| !x.is_empty()).collect();
        let decl = match w.as_slice() {
            ["out", "vec4", "gl_Position;"] | ["out", "vec4", "gl_Position", ";"] => true,
            ["out", p, "vec4", "gl_Position;"] | ["out", p, "vec4", "gl_Position", ";"] => {
                matches!(*p, "highp" | "mediump" | "lowp")
            }
            _ => false,
        };
        if decl {
            s.push_str(&line[body.len()..]);
        } else {
            s.push_str(line);
        }
    }
    let mut out = String::with_capacity(s.len());
    let mut at = 0;
    for (st, e) in idents(&s) {
        if st < at || &s[st..e] != "textureOffset" {
            continue;
        }
        let ws = s[e..].len() - s[e..].trim_start().len();
        if !s[e + ws..].starts_with('(') {
            continue;
        }
        let (args, end) = call_args(&s, e + ws).ok_or("textureOffset without its closing parenthesis")?;
        if let [smp, p, o] = args.as_slice() {
            if has_variable(o) {
                let (smp, p, o) = (smp.trim(), p.trim(), o.trim());
                out.push_str(&s[at..st]);
                out.push_str(&format!("texture({smp}, ({p}) + vec2({o}) / vec2(textureSize({smp}, 0)))"));
                at = end;
            }
        }
    }
    out.push_str(&s[at..]);
    Ok(out)
}

/// The names of the uniforms declared `uniform [precision] bool name;`. glslang makes them `uint` members of the default
/// block, the table names them `bool` as the game declared them.
pub(crate) fn bool_uniforms(t: &str) -> Vec<String> {
    let w: Vec<&str> = t.split(|c: char| c.is_whitespace()).filter(|x| !x.is_empty()).flat_map(split_semi).collect();
    let mut out = Vec::new();
    for k in 0..w.len() {
        if w[k] != "uniform" {
            continue;
        }
        let mut j = k + 1;
        if w.get(j).is_some_and(|p| matches!(*p, "lowp" | "mediump" | "highp")) {
            j += 1;
        }
        if let (Some(&"bool"), Some(n), Some(&";")) = (w.get(j), w.get(j + 1), w.get(j + 2)) {
            if n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                out.push((*n).to_string());
            }
        }
    }
    out
}

fn split_semi(w: &str) -> Vec<&str> {
    let mut v = Vec::new();
    let mut rest = w;
    while let Some(p) = rest.find(';') {
        if p > 0 {
            v.push(&rest[..p]);
        }
        v.push(";");
        rest = &rest[p + 1..];
    }
    if !rest.is_empty() {
        v.push(rest);
    }
    v
}

#[cfg(test)]
#[path = "glsl_fix_tests.rs"]
mod tests;
