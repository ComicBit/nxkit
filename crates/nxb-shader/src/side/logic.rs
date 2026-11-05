//! Logic operation entry points (port of `dmsh_logic.py`).

use super::text::{line_starts, skip_digits, skip_word, skip_ws, struct_body};
use super::unit::Entry;
use super::{LOGIC_BUFFER, LOGIC_FC};
use std::collections::BTreeSet;

/// The helper text, byte-equal to `dmsh_logic.HELPER`.
pub(crate) const HELPER: &str = "
struct dm_logic_state { uint op; uint pad0, pad1, pad2; uint4 mask[8]; };   // NVN_LOGIC_OP_* and each colour target's channel masks (dmsh_logic.py)
static inline uint4 dm_logic(uint4 s, uint4 d, constant dm_logic_state& l, uint n)
{
    uint4 r;
    switch (l.op)
    {
        case 0u: r = uint4(0u); break;
        case 1u: r = s & d; break;
        case 2u: r = s & ~d; break;
        case 4u: r = ~s & d; break;
        case 5u: r = d; break;
        case 6u: r = s ^ d; break;
        case 7u: r = s | d; break;
        case 8u: r = ~(s | d); break;
        case 9u: r = ~(s ^ d); break;
        case 10u: r = ~d; break;
        case 11u: r = s | ~d; break;
        case 12u: r = ~s; break;
        case 13u: r = ~s | d; break;
        case 14u: r = ~(s & d); break;
        case 15u: r = uint4(~0u); break;
        default: r = s; break;
    }
    return r & l.mask[n];
}
static inline float4 dm_logic_norm(float4 s, float4 d, constant dm_logic_state& l, uint n)
{
    if (all(l.mask[n] == uint4(0u))) return s;   // not a normalized target: the operation does not apply
    float4 scale = float4(l.mask[n]);
    return float4(dm_logic(uint4(floor(saturate(s) * scale + 0.5f)), uint4(floor(saturate(d) * scale + 0.5f)), l, n)) / scale;
}
";

/// `refused(msl)`.
pub(crate) fn refused(msl: &str) -> Option<String> {
    if msl.contains(&format!("[[buffer({LOGIC_BUFFER})]]")) {
        return Some(format!("fragment buffer {LOGIC_BUFFER} is taken"));
    }
    if reads_attachment(msl) {
        return Some("an entry point already reads an attachment".into());
    }
    None
}

/// `re.search(r'\[\[color\(\d+\)\]\]\s*[,)]', msl)`.
fn reads_attachment(msl: &str) -> bool {
    msl.match_indices("[[color(").any(|(i, m)| {
        let a = i + m.len();
        let b = skip_digits(msl, a);
        b > a && msl[b..].starts_with(")]]") && msl[skip_ws(msl, b + 3)..].starts_with([',', ')'])
    })
}

/// `constants(targets)`.
pub(crate) fn constants(targets: &BTreeSet<u32>) -> String {
    targets
        .iter()
        .map(|n| {
            format!(
                "constant bool dm_lfb{n} [[function_constant({})]];\n",
                LOGIC_FC + n
            )
        })
        .collect()
}

/// The colour outputs: `^\s*((?:uint|float)[234]?)\s+(\w+)\s*\[\[color\((\d+)\)\]\];` (re.M, findall).
fn outputs(body: &str) -> Vec<(String, String, u32)> {
    let mut out = Vec::new();
    let mut taken_until = 0;
    for p in line_starts(body) {
        if p < taken_until {
            continue;
        }
        let rest = &body[p..];
        let Some(base) = ["uint", "float"].into_iter().find(|t| rest.starts_with(t)) else {
            continue;
        };
        let mut a = p + base.len();
        if body[a..].starts_with(['2', '3', '4']) {
            // `[234]?` is greedy but may give the digit back, a digit followed by white space is the only way on.
            let a2 = a + 1;
            if skip_ws(body, a2) > a2 {
                a = a2;
            }
        }
        let b = skip_ws(body, a);
        if b == a {
            continue;
        }
        let c = skip_word(body, b);
        if c == b {
            continue;
        }
        let d = skip_ws(body, c);
        let Some(t) = body[d..].strip_prefix("[[color(") else {
            continue;
        };
        let e = d + "[[color(".len();
        let f = skip_digits(body, e);
        if f == e || !body[f..].starts_with(")]];") || t.is_empty() {
            continue;
        }
        let Ok(n) = body[e..f].parse::<u32>() else {
            continue;
        };
        out.push((body[p..a].to_string(), body[b..c].to_string(), n));
        taken_until = f + 4;
    }
    out
}

/// `entry(msl, e)`: the logic operation entry point and the colour outputs it reads.
pub(crate) fn entry(msl: &str, e: &Entry) -> Option<(String, BTreeSet<u32>)> {
    if !["main0", "main0_u", "main0_m"].contains(&e.name.as_str()) {
        return None;
    }
    let outs = struct_body(msl, &e.ret).map(outputs).unwrap_or_default();
    if outs.is_empty() {
        return None;
    }
    let scalar = |ty: &str| {
        if ty.starts_with("uint") {
            "uint"
        } else {
            "float"
        }
    };
    let mut params = e.params.clone();
    for (ty, _, n) in &outs {
        params.push(format!(
            "{}4 dm_fb{n} [[color({n}), function_constant(dm_lfb{n})]]",
            scalar(ty)
        ));
    }
    let mut apply = Vec::new();
    for (ty, nm, n) in &outs {
        let s = scalar(ty);
        let width: usize = ty[s.len()..].parse().unwrap_or(1);
        let zero = if s == "uint" { "0u" } else { "0.0f" };
        let pad = format!(", {zero}").repeat(4usize.saturating_sub(width));
        let sw = if width < 4 {
            format!(".{}", &"xyzw"[..width])
        } else {
            String::new()
        };
        let f = if s == "uint" {
            "dm_logic"
        } else {
            "dm_logic_norm"
        };
        apply.push(format!(
            "if (dm_lfb{n}) dm_r.{nm} = {ty}({f}({s}4(dm_r.{nm}{pad}), dm_fb{n}, dm_lop, {n}u){sw});"
        ));
    }
    params.push(format!(
        "constant dm_logic_state& dm_lop [[buffer({LOGIC_BUFFER})]]"
    ));
    let text = format!(
        "{attr}fragment {ret} {n}_l({p})\n{{\n    {ret} dm_r = dm_body_{n}({args});\n    {apply}\n    return dm_r;\n}}\n",
        attr = e.attr,
        ret = e.ret,
        n = e.name,
        p = params.join(", "),
        args = e.args,
        apply = apply.join(" ")
    );
    Some((text, outs.iter().map(|o| o.2).collect()))
}
