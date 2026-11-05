//! Alpha test entry points (port of `dmsh_alpha.py`).

use super::ALPHA_BUFFER;
use super::text::{line_starts, skip_word, skip_ws, struct_body};
use super::unit::Entry;

/// The helper text, byte-equal to `dmsh_alpha.HELPER`.
pub(crate) const HELPER: &str = "
struct dm_alpha_state { uint func; float ref; };   // NVN_ALPHA_FUNC_* and the reference (dmsh_alpha.py)
static inline bool dm_alpha_pass(float a, constant dm_alpha_state& s)
{
    switch (s.func)
    {
        case 1u: return false;
        case 2u: return a < s.ref;
        case 3u: return a == s.ref;
        case 4u: return a <= s.ref;
        case 5u: return a > s.ref;
        case 6u: return a != s.ref;
        case 7u: return a >= s.ref;
        default: return true;
    }
}
";

/// `_colour0`: `^\s*(float4|half4|uint4)\s+(\w+)\s*\[\[color\(0\)(?:, index\(0\))?\]\];` (re.M) in the struct body.
fn colour0<'a>(msl: &'a str, name: &str) -> Option<(&'a str, &'a str)> {
    let body = struct_body(msl, name)?;
    for p in line_starts(body) {
        let rest = &body[p..];
        let Some(ty) = ["float4", "half4", "uint4"]
            .into_iter()
            .find(|t| rest.starts_with(t))
        else {
            continue;
        };
        let a = p + ty.len();
        let b = skip_ws(body, a);
        if b == a {
            continue;
        }
        let c = skip_word(body, b);
        if c == b {
            continue;
        }
        let d = skip_ws(body, c);
        let tail = &body[d..];
        let ok = tail.starts_with("[[color(0)]];") || tail.starts_with("[[color(0), index(0)]];");
        if ok {
            return Some((&body[p..a], &body[b..c]));
        }
    }
    None
}

/// `refused(msl)`: why the shader gets no alpha test entry points.
pub(crate) fn refused(msl: &str) -> Option<String> {
    let Some(body) = struct_body(msl, "main0_out") else {
        return Some("no colour output".into());
    };
    if matches!(colour0(msl, "main0_out"), None | Some(("uint4", _))) {
        return Some(if body.contains("[[color(0)]]") {
            "colour 0 has no float alpha".into()
        } else {
            "no colour 0 output".into()
        });
    }
    if msl.contains(&format!("[[buffer({ALPHA_BUFFER})]]")) {
        return Some(format!("fragment buffer {ALPHA_BUFFER} is taken"));
    }
    None
}

/// `entry(msl, e)`: the alpha test entry point's text.
pub(crate) fn entry(msl: &str, e: &Entry) -> Option<String> {
    let (ty, name) = colour0(msl, &e.ret)?;
    let alpha = if ty == "uint4" {
        format!("as_type<float>(dm_r.{name}.w)")
    } else {
        format!("float(dm_r.{name}.w)")
    };
    let mut params = e.params.clone();
    params.push(format!(
        "constant dm_alpha_state& dm_alpha [[buffer({ALPHA_BUFFER})]]"
    ));
    Some(format!(
        "{attr}fragment {ret} {n}_a({p})\n{{\n    {ret} dm_r = dm_body_{n}({args});\n    if (!dm_alpha_pass({alpha}, dm_alpha)) discard_fragment();\n    return dm_r;\n}}\n",
        attr = e.attr,
        ret = e.ret,
        n = e.name,
        p = params.join(", "),
        args = e.args
    ))
}
