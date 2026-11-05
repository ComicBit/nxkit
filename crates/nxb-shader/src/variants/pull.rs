//! `<key>_bp`: the vertex shader reading its inputs itself (port of `dmsh_pull.variant`).

use super::common::{StageIn, member, short, uint_param, vertex_header};
use nxb_pyjson::Value;

/// The layout block's buffer index (`dmsh_pull.SLOT`).
pub const SLOT: u32 = 16;

/// `dmsh_pull.metal`, the fetch helpers inserted after `using namespace metal;`.
pub const FETCH: &str = include_str!("../../data/dmsh_pull.metal");

/// `_drop_entries(msl, keep)`: drop top-level vertex functions not in `keep` (from a line `vertex ...` to the next
/// line `}`).
fn drop_entries(msl: &str, keep: &[&str]) -> String {
    let lines: Vec<&str> = msl.split('\n').collect();
    let mut out: Vec<&str> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        if let Some(name) = vertex_name(lines[i])
            && !keep.contains(&name)
        {
            let mut j = i;
            while j < lines.len() && lines[j] != "}" {
                j += 1;
            }
            i = j + 1;
            continue;
        }
        out.push(lines[i]);
        i += 1;
    }
    out.join("\n")
}

/// `re.match(r'vertex \S+ (\w+)\(', line)`.
fn vertex_name(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("vertex ")?;
    let s_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    if s_end == 0 {
        return None;
    }
    let rest = rest[s_end..].strip_prefix(' ')?;
    let w_end = rest
        .char_indices()
        .find(|(_, c)| !(c.is_alphanumeric() || *c == '_'))
        .map_or(rest.len(), |(k, _)| k);
    (w_end > 0 && rest[w_end..].starts_with('(')).then(|| &rest[..w_end])
}

/// `variant(msl)` → (source, info) or the reason. `info` is the dict Python returns.`dmsh_build.pull_finish` keeps
/// its non-`None` members as reflection `pull`.
pub fn variant(msl: &str) -> Result<(String, Value), String> {
    let Some(m) = StageIn::find(msl) else {
        return Err("no stage input".into());
    };
    if msl.contains(&format!("[[buffer({SLOT})]]")) {
        return Err(format!("buffer {SLOT} is taken"));
    }
    let mut members = Vec::new();
    for ln in m.body.split('\n') {
        let Some(x) = member(ln) else {
            return Err(format!(
                "an input declaration not understood: {}",
                short(ln)
            ));
        };
        members.push(x);
    }
    if members.iter().any(|(_, _, l)| *l >= 16) {
        return Err("an input at attribute 16 or above".into());
    }
    let decls: Vec<String> = members
        .iter()
        .map(|(t, n, _)| format!("    {t} {n};"))
        .collect();
    let mut head = format!(
        "{}\nstruct main0_in\n{{\n{}\n}};",
        &msl[..m.start],
        decls.join("\n")
    );
    let mut body = drop_entries(&msl[m.end..], &["main0", "main0_z"]);
    let mut has_z = false;
    for entry in ["main0", "main0_z"] {
        let Some((hs, he, rest)) = vertex_header(&body, entry, true) else {
            if entry == "main0" {
                return Err("main0 signature not recognized".into());
            }
            continue;
        };
        let vid = uint_param(rest, "vertex_id");
        let iid = uint_param(rest, "instance_id");
        let bi = uint_param(rest, "base_instance");
        let mut params: Vec<String> = if rest.is_empty() {
            Vec::new()
        } else {
            rest[2..].split(", ").map(str::to_string).collect()
        };
        params.push(format!("constant dm_vlayout& dm_lay [[buffer({SLOT})]]"));
        if vid.is_none() {
            params.push("uint dm_vid [[vertex_id]]".into());
        }
        if iid.is_none() {
            params.push("uint dm_iid [[instance_id]]".into());
        }
        if bi.is_none() {
            params.push("uint dm_bi [[base_instance]]".into());
        }
        let (vn, inn, bn) = (
            vid.unwrap_or("dm_vid").to_string(),
            iid.unwrap_or("dm_iid").to_string(),
            bi.unwrap_or("dm_bi").to_string(),
        );
        let mut pro = String::from("    main0_in in;\n");
        for (t, n, l) in &members {
            pro.push_str(&format!(
                "    in.{n} = as_type<{t}>(dm_fetch(dm_lay, {l}u, {vn}, {inn}, {bn}));\n"
            ));
        }
        body = format!(
            "{}\nvertex main0_out {entry}({})\n{{\n{pro}{}",
            &body[..hs],
            params.join(", "),
            &body[he..]
        );
        has_z |= entry == "main0_z";
    }
    const USING: &str = "using namespace metal;";
    let Some(ui) = head.find(USING) else {
        return Err("no \"using namespace metal;\"".into());
    };
    let ui = ui + USING.len();
    head = format!("{}\n\n{FETCH}{}", &head[..ui], &head[ui..]);
    let info = Value::obj([
        ("entry", Value::str("main0")),
        (
            "entry_z",
            if has_z {
                Value::str("main0_z")
            } else {
                Value::Null
            },
        ),
        ("slot", Value::Int(i64::from(SLOT))),
        (
            "inputs",
            Value::List(members.iter().map(|m| Value::Int(i64::from(m.2))).collect()),
        ),
    ]);
    Ok((head + &body, info))
}
