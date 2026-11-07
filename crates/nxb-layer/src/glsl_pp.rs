//! A GLSL preprocessor, the counterpart of `glslangValidator -E`. glslang's own preprocessor only runs inside a parse, and a
//! parse fails on the source text before the fix-ups in `crate::glsl_fix` (`textureCube` is a texture type under Vulkan
//! rules). So the macros are expanded here, the fix-ups run on the result, and glslang reads the fixed text.
//!
//! It handles comments, line continuations, `#define` with and without parameters, `##`, `#undef`, the conditionals with
//! `defined`, `#error` and `#line`. `#version`, `#extension` and `#pragma` stay in the text. The macros glslang defines for
//! GLSL ES (`GL_ES`, `GL_FRAGMENT_PRECISION_HIGH`, the extension macros) are defined for the conditionals, as are
//! `__VERSION__` and `__LINE__`.

use std::collections::BTreeMap;

#[derive(Clone, Debug)]
struct Macro {
    params: Option<Vec<String>>,
    body: Vec<String>,
}

/// The extension macros glslang defines for GLSL ES (`MachineIndependent/Versions.cpp`).
const ES_EXT: &[&str] = &[
    "GL_OES_texture_3D",
    "GL_OES_standard_derivatives",
    "GL_EXT_frag_depth",
    "GL_OES_EGL_image_external",
    "GL_OES_EGL_image_external_essl3",
    "GL_EXT_YUV_target",
    "GL_EXT_shader_texture_lod",
    "GL_EXT_shadow_samplers",
    "GL_EXT_fragment_shading_rate",
    "GL_EXT_conservative_depth",
    "GL_ANDROID_extension_pack_es31a",
    "GL_OES_sample_variables",
    "GL_OES_shader_image_atomic",
    "GL_OES_shader_multisample_interpolation",
    "GL_OES_texture_storage_multisample_2d_array",
    "GL_EXT_geometry_shader",
    "GL_EXT_geometry_point_size",
    "GL_EXT_gpu_shader5",
    "GL_EXT_primitive_bounding_box",
    "GL_EXT_shader_io_blocks",
    "GL_EXT_tessellation_shader",
    "GL_EXT_tessellation_point_size",
    "GL_EXT_texture_buffer",
    "GL_EXT_texture_cube_map_array",
    "GL_EXT_shader_implicit_conversions",
    "GL_EXT_shader_integer_mix",
    "GL_EXT_blend_func_extended",
    "GL_OES_geometry_shader",
    "GL_OES_geometry_point_size",
    "GL_OES_gpu_shader5",
    "GL_OES_primitive_bounding_box",
    "GL_OES_shader_io_blocks",
    "GL_OES_tessellation_shader",
    "GL_OES_tessellation_point_size",
    "GL_OES_texture_buffer",
    "GL_OES_texture_cube_map_array",
    "GL_EXT_shader_non_constant_global_initializers",
];

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident(t: &str) -> bool {
    t.starts_with(is_ident_start)
}

fn is_space(t: &str) -> bool {
    t.starts_with(|c: char| c.is_whitespace())
}

/// Splits text into identifiers, numbers, runs of white space and single other characters, with `##` kept as one token.
fn tokens(s: &str) -> Vec<String> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let start = i;
        if is_ident_start(c[i]) {
            while i < c.len() && (c[i].is_ascii_alphanumeric() || c[i] == '_') {
                i += 1;
            }
        } else if c[i].is_ascii_digit()
            || (c[i] == '.' && c.get(i + 1).is_some_and(char::is_ascii_digit))
        {
            i += 1;
            while i < c.len() {
                let sign = (c[i] == '+' || c[i] == '-') && matches!(c[i - 1], 'e' | 'E');
                if !(c[i].is_ascii_alphanumeric() || c[i] == '_' || c[i] == '.' || sign) {
                    break;
                }
                i += 1;
            }
        } else if c[i].is_whitespace() {
            while i < c.len() && c[i].is_whitespace() {
                i += 1;
            }
        } else if c[i] == '#' && c.get(i + 1) == Some(&'#') {
            i += 2;
        } else {
            i += 1;
        }
        out.push(c[start..i].iter().collect());
    }
    out
}

/// Removes comments and joins continued lines. A comment becomes a space, the newlines of a block comment stay.
fn strip(src: &str) -> String {
    let s = src.replace("\r\n", "\n").replace("\\\n", "");
    let c: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < c.len() {
        if c[i] == '/' && c.get(i + 1) == Some(&'/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
            out.push(' ');
        } else if c[i] == '/' && c.get(i + 1) == Some(&'*') {
            i += 2;
            while i < c.len() && !(c[i] == '*' && c.get(i + 1) == Some(&'/')) {
                if c[i] == '\n' {
                    out.push('\n');
                }
                i += 1;
            }
            i += 2;
            out.push(' ');
        } else {
            out.push(c[i]);
            i += 1;
        }
    }
    out
}

struct Pp {
    macros: BTreeMap<String, Macro>,
}

impl Pp {
    fn define(&mut self, name: &str, value: &str) {
        self.macros.insert(
            name.to_string(),
            Macro {
                params: None,
                body: tokens(value),
            },
        );
    }

    /// Expands the macros in `t`. A macro is not expanded again inside its own expansion.
    fn expand(
        &self,
        t: &[String],
        hide: &mut Vec<String>,
        depth: usize,
    ) -> Result<Vec<String>, String> {
        if depth > 64 {
            return Err("macro expansion too deep".into());
        }
        let mut out = Vec::new();
        let mut i = 0;
        while i < t.len() {
            let name = &t[i];
            let Some(m) = self
                .macros
                .get(name)
                .filter(|_| is_ident(name) && !hide.contains(name))
            else {
                out.push(name.clone());
                i += 1;
                continue;
            };
            let body = match &m.params {
                None => {
                    i += 1;
                    paste(m.body.clone())
                }
                Some(params) => {
                    let mut j = i + 1;
                    while j < t.len() && is_space(&t[j]) {
                        j += 1;
                    }
                    if t.get(j).map(String::as_str) != Some("(") {
                        out.push(name.clone());
                        i += 1;
                        continue;
                    }
                    let (args, end) = call_args(t, j)?;
                    i = end;
                    let args: Vec<Vec<String>> = if params.is_empty()
                        && args.len() == 1
                        && args[0].iter().all(|x| is_space(x))
                    {
                        Vec::new()
                    } else {
                        args
                    };
                    if args.len() != params.len() {
                        return Err(format!(
                            "macro {name} takes {} arguments, not {}",
                            params.len(),
                            args.len()
                        ));
                    }
                    let mut expanded = Vec::new();
                    for a in &args {
                        expanded.push(self.expand(a, hide, depth + 1)?);
                    }
                    let mut body = Vec::new();
                    for (k, b) in m.body.iter().enumerate() {
                        let Some(p) = params.iter().position(|p| p == b) else {
                            body.push(b.clone());
                            continue;
                        };
                        let near = |d: isize| {
                            let mut x = k as isize + d;
                            while x >= 0
                                && (x as usize) < m.body.len()
                                && is_space(&m.body[x as usize])
                            {
                                x += d;
                            }
                            x >= 0 && m.body.get(x as usize).is_some_and(|y| y == "##")
                        };
                        let arg = if near(-1) || near(1) {
                            &args[p]
                        } else {
                            &expanded[p]
                        };
                        body.extend(trim(arg).iter().cloned());
                    }
                    paste(body)
                }
            };
            hide.push(name.clone());
            let r = self.expand(&body, hide, depth + 1);
            hide.pop();
            out.extend(r?);
        }
        Ok(out)
    }
}

fn trim(t: &[String]) -> &[String] {
    let a = t.iter().position(|x| !is_space(x)).unwrap_or(t.len());
    let b = t.iter().rposition(|x| !is_space(x)).map_or(a, |b| b + 1);
    &t[a..b]
}

/// Joins the tokens around each `##`.
fn paste(t: Vec<String>) -> Vec<String> {
    if !t.iter().any(|x| x == "##") {
        return t;
    }
    let mut out: Vec<String> = Vec::new();
    let mut glue = false;
    for x in t {
        if x == "##" {
            while out.last().is_some_and(|y| is_space(y)) {
                out.pop();
            }
            glue = true;
        } else if glue && is_space(&x) {
        } else if glue {
            match out.last_mut() {
                Some(l) => l.push_str(&x),
                None => out.push(x),
            }
            glue = false;
        } else {
            out.push(x);
        }
    }
    tokens(&out.concat())
}

/// The arguments of the call whose `(` is token `open`, and the index after its `)`.
fn call_args(t: &[String], open: usize) -> Result<(Vec<Vec<String>>, usize), String> {
    let mut depth = 0;
    let mut args = vec![Vec::new()];
    for (k, x) in t.iter().enumerate().skip(open) {
        match x.as_str() {
            "(" => {
                depth += 1;
                if depth == 1 {
                    continue;
                }
            }
            ")" => {
                depth -= 1;
                if depth == 0 {
                    return Ok((args, k + 1));
                }
            }
            "," if depth == 1 => {
                args.push(Vec::new());
                continue;
            }
            _ => {}
        }
        if let Some(a) = args.last_mut() {
            a.push(x.clone());
        }
    }
    Err("a macro call without its closing parenthesis".into())
}

/// Evaluates a `#if` expression whose macros are expanded: integers, unary `! ~ - +`, the binary operators of C and
/// parentheses. An identifier left over is 0.
fn eval(t: &[String]) -> Result<i64, String> {
    let t: Vec<&str> = t
        .iter()
        .map(String::as_str)
        .filter(|x| !is_space(x))
        .collect();
    // two-character operators come as two tokens
    let mut v: Vec<String> = Vec::new();
    for x in t {
        let joined = v.last().map(|l| format!("{l}{x}"));
        match joined.as_deref() {
            Some("<<" | ">>" | "<=" | ">=" | "==" | "!=" | "&&" | "||") => {
                if let Some(l) = v.last_mut() {
                    l.push_str(x);
                }
            }
            _ => v.push(x.to_string()),
        }
    }
    let mut p = 0;
    let r = expr(&v, &mut p, 0)?;
    if p != v.len() {
        return Err(format!(
            "an expression with `{}` left over",
            v[p..].join(" ")
        ));
    }
    Ok(r)
}

fn prec(op: &str) -> Option<u8> {
    Some(match op {
        "||" => 1,
        "&&" => 2,
        "|" => 3,
        "^" => 4,
        "&" => 5,
        "==" | "!=" => 6,
        "<" | ">" | "<=" | ">=" => 7,
        "<<" | ">>" => 8,
        "+" | "-" => 9,
        "*" | "/" | "%" => 10,
        _ => return None,
    })
}

fn unary(v: &[String], p: &mut usize) -> Result<i64, String> {
    let x = v.get(*p).ok_or("an expression that ends early")?;
    *p += 1;
    Ok(match x.as_str() {
        "!" => i64::from(unary(v, p)? == 0),
        "~" => !unary(v, p)?,
        "-" => unary(v, p)?.wrapping_neg(),
        "+" => unary(v, p)?,
        "(" => {
            let r = expr(v, p, 0)?;
            if v.get(*p).map(String::as_str) != Some(")") {
                return Err("a missing )".into());
            }
            *p += 1;
            r
        }
        n if is_ident(n) => 0,
        n => {
            let n = n.trim_end_matches(['u', 'U']);
            let r = if let Some(h) = n.strip_prefix("0x").or_else(|| n.strip_prefix("0X")) {
                i64::from_str_radix(h, 16)
            } else if n.len() > 1 && n.starts_with('0') {
                i64::from_str_radix(&n[1..], 8)
            } else {
                n.parse()
            };
            r.map_err(|_| format!("`{n}` is not an integer"))?
        }
    })
}

fn expr(v: &[String], p: &mut usize, min: u8) -> Result<i64, String> {
    let mut l = unary(v, p)?;
    while let Some(op) = v.get(*p).map(String::as_str) {
        let Some(pr) = prec(op).filter(|&pr| pr > min) else {
            break;
        };
        *p += 1;
        let r = expr(v, p, pr)?;
        l = match op {
            "||" => i64::from(l != 0 || r != 0),
            "&&" => i64::from(l != 0 && r != 0),
            "|" => l | r,
            "^" => l ^ r,
            "&" => l & r,
            "==" => i64::from(l == r),
            "!=" => i64::from(l != r),
            "<" => i64::from(l < r),
            ">" => i64::from(l > r),
            "<=" => i64::from(l <= r),
            ">=" => i64::from(l >= r),
            "<<" => l.wrapping_shl(r as u32),
            ">>" => l.wrapping_shr(r as u32),
            "+" => l.wrapping_add(r),
            "-" => l.wrapping_sub(r),
            "*" => l.wrapping_mul(r),
            _ if r == 0 => return Err("a division by zero".into()),
            "/" => l / r,
            _ => l % r,
        };
    }
    Ok(l)
}

/// Replaces `defined X` and `defined(X)` by 1 or 0.
fn resolve_defined(pp: &Pp, t: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < t.len() {
        if t[i] != "defined" {
            out.push(t[i].clone());
            i += 1;
            continue;
        }
        let rest: Vec<(usize, &String)> = t
            .iter()
            .enumerate()
            .skip(i + 1)
            .filter(|(_, x)| !is_space(x))
            .take(3)
            .collect();
        let (name, end) = match rest.as_slice() {
            [(_, o), (_, n), (k, c), ..] if o.as_str() == "(" && c.as_str() == ")" => {
                (n.as_str(), k + 1)
            }
            [(k, n), ..] => (n.as_str(), k + 1),
            [] => ("", t.len()),
        };
        out.push(
            if pp.macros.contains_key(name) {
                "1"
            } else {
                "0"
            }
            .to_string(),
        );
        i = end;
    }
    out
}

/// Preprocesses GLSL `src` as glslang does for an ES or desktop profile, the profile and version taken from its `#version`.
pub(crate) fn preprocess(src: &str) -> Result<String, String> {
    let text = strip(src);
    let mut pp = Pp {
        macros: BTreeMap::new(),
    };
    let mut version = 100;
    let mut out = String::new();
    // per open conditional: is its current branch on, has one of its branches been taken, is its parent on
    let mut stack: Vec<(bool, bool, bool)> = Vec::new();
    let mut chunk: Vec<String> = Vec::new();
    let on = |s: &Vec<(bool, bool, bool)>| s.last().is_none_or(|c| c.0);
    let lines: Vec<&str> = text.split('\n').collect();
    for (n, line) in lines.iter().enumerate() {
        let nl = if n + 1 < lines.len() { "\n" } else { "" };
        let d = line.trim_start();
        let Some(dir) = d.strip_prefix('#') else {
            if on(&stack) {
                chunk.push(format!("{line}{nl}"));
            } else {
                out.push_str(nl);
            }
            continue;
        };
        if !chunk.is_empty() {
            out.push_str(
                &pp.expand(&tokens(&chunk.concat()), &mut Vec::new(), 0)?
                    .concat(),
            );
            chunk.clear();
        }
        let dir = dir.trim_start();
        let word: String = dir
            .chars()
            .take_while(|&c| c.is_ascii_alphanumeric() || c == '_')
            .collect();
        let rest = dir[word.len()..].trim();
        let active = on(&stack);
        pp.define("__LINE__", &(n + 1).to_string());
        match word.as_str() {
            "if" | "ifdef" | "ifndef" => {
                let v = !active
                    || match word.as_str() {
                        "if" => {
                            let t = resolve_defined(&pp, &tokens(rest));
                            eval(&pp.expand(&t, &mut Vec::new(), 0)?)? != 0
                        }
                        "ifdef" => pp.macros.contains_key(rest),
                        _ => !pp.macros.contains_key(rest),
                    };
                stack.push((active && v, active && v, active));
            }
            "elif" => {
                let c = stack.last().copied().ok_or("#elif without #if")?;
                let v = c.2 && !c.1 && {
                    let t = resolve_defined(&pp, &tokens(rest));
                    eval(&pp.expand(&t, &mut Vec::new(), 0)?)? != 0
                };
                if let Some(c) = stack.last_mut() {
                    *c = (v, c.1 || v, c.2);
                }
            }
            "else" => {
                let c = stack.last_mut().ok_or("#else without #if")?;
                *c = (c.2 && !c.1, true, c.2);
            }
            "endif" => {
                stack.pop().ok_or("#endif without #if")?;
            }
            _ if !active => {}
            "define" => {
                let name: String = rest
                    .chars()
                    .take_while(|&c| c.is_ascii_alphanumeric() || c == '_')
                    .collect();
                if name.is_empty() {
                    return Err(format!("line {}: #define without a name", n + 1));
                }
                let after = &rest[name.len()..];
                let (params, body) = match after.strip_prefix('(') {
                    Some(a) => {
                        let close = a
                            .find(')')
                            .ok_or_else(|| format!("line {}: #define {name}( without )", n + 1))?;
                        let ps: Vec<String> = a[..close]
                            .split(',')
                            .map(|p| p.trim().to_string())
                            .filter(|p| !p.is_empty())
                            .collect();
                        (Some(ps), &a[close + 1..])
                    }
                    None => (None, after),
                };
                pp.macros.insert(
                    name,
                    Macro {
                        params,
                        body: tokens(body.trim()),
                    },
                );
            }
            "undef" => {
                pp.macros.remove(rest);
            }
            "error" => return Err(format!("#error {rest}")),
            "line" | "" => {}
            "version" => {
                let mut w = rest.split_whitespace();
                version = w.next().and_then(|v| v.parse().ok()).unwrap_or(100);
                let es = version == 100
                    || version == 300
                    || version == 310
                    || version == 320
                    || w.next() == Some("es");
                pp.define("__VERSION__", &version.to_string());
                if es {
                    for m in ["GL_ES", "GL_FRAGMENT_PRECISION_HIGH"].iter().chain(ES_EXT) {
                        pp.define(m, "1");
                    }
                }
                out.push_str(d);
            }
            _ => out.push_str(d),
        }
        out.push_str(nl);
    }
    if !stack.is_empty() {
        return Err("#if without #endif".into());
    }
    if !chunk.is_empty() {
        out.push_str(
            &pp.expand(&tokens(&chunk.concat()), &mut Vec::new(), 0)?
                .concat(),
        );
    }
    let _ = version;
    Ok(out)
}

#[cfg(test)]
#[path = "glsl_pp_tests.rs"]
mod tests;
