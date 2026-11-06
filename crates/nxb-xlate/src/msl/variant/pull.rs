//! The vertex pulling library `_bp`: the entries read their inputs from the streams themselves.
//!
//! The host fills a `dm_vlayout` block per draw and binds it at buffer 16. `dm_fetch` (`dm_fetch.metal`) turns the
//! attribute state of Maxwell and the stream into the four register words, which the input takes with `as_type`.

use super::super::io::Plan;
use nxb_pyjson::Value;
use std::fmt::Write;

/// The buffer index of the layout block.
pub const SLOT: u32 = 16;

/// The fetch helpers, placed after `using namespace metal;`.
pub const FETCH: &str = include_str!("dm_fetch.metal");

/// What the library `_bp` adds to the base library.
pub struct Pull {
    /// The struct `main0_in` as plain members.
    pub in_struct: String,
    /// The parameters after the standard ones.
    pub params: Vec<String>,
    /// The statements that fill `in`.
    pub pro: Vec<String>,
    /// The locations pulled.
    pub inputs: Vec<u32>,
}

/// The library `_bp` of a vertex shader, or `None` when the host cannot pull its inputs.
pub fn build(plan: &Plan) -> Option<Pull> {
    let attrs = &plan.attributes;
    if attrs.is_empty()
        || attrs.iter().any(|a| a.0 >= SLOT)
        || attrs
            .iter()
            .any(|a| !matches!(a.1.as_str(), "float4" | "uint4" | "int4"))
    {
        return None;
    }
    let mut in_struct = String::from("struct main0_in\n{\n");
    for (l, t) in attrs {
        let _ = writeln!(in_struct, "    {t} loc{l};");
    }
    in_struct.push_str("};\n");
    let has = |n: &str| plan.builtins.iter().any(|b| b.name == n);
    let mut params = vec![format!("constant dm_vlayout& dm_lay [[buffer({SLOT})]]")];
    let mut names = Vec::new();
    for (name, own, attr) in [
        ("gl_VertexIndex", "dm_vid", "vertex_id"),
        ("gl_InstanceIndex", "dm_iid", "instance_id"),
        ("gl_BaseInstance", "dm_bi", "base_instance"),
    ] {
        if has(name) {
            names.push(name);
        } else {
            params.push(format!("uint {own} [[{attr}]]"));
            names.push(own);
        }
    }
    let mut pro = vec!["main0_in in;".to_string()];
    for (l, t) in attrs {
        pro.push(format!(
            "in.loc{l} = as_type<{t}>(dm_fetch(dm_lay, {l}u, {}, {}, {}));",
            names[0], names[1], names[2]
        ));
    }
    let mut inputs: Vec<u32> = attrs.iter().map(|a| a.0).collect();
    inputs.sort_unstable();
    Some(Pull {
        in_struct,
        params,
        pro,
        inputs,
    })
}

/// The `pull` field of the reflection.
pub fn field(p: &Pull) -> Value {
    Value::obj([
        ("entry", Value::str("main0")),
        ("entry_z", Value::str("main0_z")),
        ("slot", Value::Int(i64::from(SLOT))),
        (
            "inputs",
            Value::List(p.inputs.iter().map(|l| Value::Int(i64::from(*l))).collect()),
        ),
    ])
}
