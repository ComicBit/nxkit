//! The vertex input library `_bv`: each integer input also reads two shadow attributes.
//!
//! A host whose vertex format does not feed an integer register directly binds the same stream a second time as
//! `float4` (and once with the other sign) at an attribute of 16 to 30. The entries OR the shadows into the input.

use super::super::entry::Param;
use super::super::io::Plan;
use nxb_pyjson::Value;
use std::fmt::Write;

/// The first shadow attribute.
const SLOT0: u32 = 16;
/// The end of the shadow attributes.
const SLOT_END: u32 = 31;

/// A shadow: the location, the float slot and the other-sign key with its slot.
type Shadow<'a> = (u32, u32, Option<(&'a str, u32)>);

/// What the library `_bv` adds to the base library.
pub struct Vin {
    /// The structs `main0_in` and `main0_in_v`.
    pub in_struct: String,
    /// The stage input parameter of the entries.
    pub in_param: Param,
    /// The statements that fill `in` from `dm_in`.
    pub pro: Vec<String>,
    /// The `vin` field of the reflection without the entry names.
    pub shadow: Vec<Value>,
}

fn int_type(t: &str) -> bool {
    t == "uint4" || t == "int4"
}

/// The library `_bv` of a vertex shader, or `None` when it has no integer input or one the host cannot shadow.
pub fn build(plan: &Plan) -> Option<Vin> {
    let attrs = &plan.attributes;
    let ints: Vec<&(u32, String)> = attrs.iter().filter(|a| a.1 != "float4").collect();
    if ints.is_empty()
        || attrs.iter().any(|a| a.0 >= SLOT0)
        || attrs.iter().any(|a| a.1 != "float4" && !int_type(&a.1))
    {
        return None;
    }
    let mut slot = SLOT0;
    let mut extra = String::new();
    let mut shadow: Vec<Shadow<'_>> = Vec::new();
    for (l, _) in &ints {
        if slot >= SLOT_END {
            break;
        }
        let _ = writeln!(extra, "    float4 loc{l}_vf [[attribute({slot})]];");
        shadow.push((*l, slot, None));
        slot += 1;
    }
    for (e, (l, t)) in shadow.iter_mut().zip(&ints) {
        if slot >= SLOT_END {
            break;
        }
        let (o, key) = if t == "uint4" {
            ("int4", "sint")
        } else {
            ("uint4", "uint")
        };
        let _ = writeln!(extra, "    {o} loc{l}_vo [[attribute({slot})]];");
        e.2 = Some((key, slot));
        slot += 1;
    }
    let mut in_struct = plan.in_struct();
    in_struct.push_str("struct main0_in_v\n{\n");
    for m in &plan.in_members {
        let _ = writeln!(in_struct, "    {m}");
    }
    in_struct.push_str(&extra);
    in_struct.push_str("};\n");
    let mut pro = vec!["main0_in in = {};".to_string()];
    for (l, t) in attrs {
        let ors = match shadow.iter().find(|s| s.0 == *l) {
            None => String::new(),
            Some(s) => {
                let mut o = format!(" | as_type<{t}>(dm_in.loc{l}_vf)");
                if s.2.is_some() {
                    let _ = write!(o, " | as_type<{t}>(dm_in.loc{l}_vo)");
                }
                o
            }
        };
        pro.push(format!("in.loc{l} = dm_in.loc{l}{ors};"));
    }
    let shadow = shadow
        .iter()
        .map(|(l, f, o)| {
            let mut v = Value::obj([
                ("location", Value::Int(i64::from(*l))),
                ("float", Value::Int(i64::from(*f))),
            ]);
            if let Some((k, s)) = o {
                v.set(k, Value::Int(i64::from(*s)));
            }
            v
        })
        .collect();
    Some(Vin {
        in_struct,
        in_param: Param::new("main0_in_v dm_in", "stage_in", "dm_in"),
        pro,
        shadow,
    })
}

/// The `vin` field of the reflection.
pub fn field(v: &Vin) -> Value {
    Value::obj([
        ("entry", Value::str("main0")),
        ("entry_z", Value::str("main0_z")),
        ("shadow", Value::List(v.shadow.clone())),
    ])
}
