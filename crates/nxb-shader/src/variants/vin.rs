//! `<key>_bv`: shadow integer inputs (port of `dmsh_vin.variant`).

use super::common::{StageIn, member, short, vertex_header};
use nxb_pyjson::Value;

/// A shadow: (location, float slot, other-sign (key, slot)).
type Shadow<'a> = (u32, u32, Option<(&'a str, u32)>);

const SLOT0: u32 = 16;
const SLOT_END: u32 = 31;

/// `variant(msl)` → (source, shadows list) or the reason.
pub fn variant(msl: &str) -> Result<(String, Value), String> {
    let Some(m) = StageIn::find(msl) else {
        return Err(if msl.contains("[[stage_in]]") {
            "no main0_in struct".into()
        } else {
            "no integer input".into()
        });
    };
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
    let ints: Vec<_> = members
        .iter()
        .filter(|x| x.0 != "float4")
        .copied()
        .collect();
    if ints.is_empty() {
        return Err("no integer input".into());
    }
    if members.iter().any(|(_, _, l)| *l >= SLOT0) {
        return Err("an input at attribute 16 or above".into());
    }
    let mut slot = SLOT0;
    // (location, float slot, other-sign (key, slot))
    let mut shadows: Vec<Shadow<'_>> = Vec::new();
    let mut extra = Vec::new();
    for (_, n, l) in &ints {
        if slot >= SLOT_END {
            break;
        }
        extra.push(format!("    float4 {n}_vf [[attribute({slot})]];"));
        shadows.push((*l, slot, None));
        slot += 1;
    }
    for (e, (t, n, _)) in shadows.iter_mut().zip(&ints) {
        if slot >= SLOT_END {
            break;
        }
        let o = if *t == "uint4" { "int4" } else { "uint4" };
        extra.push(format!("    {o} {n}_vo [[attribute({slot})]];"));
        e.2 = Some((if o == "int4" { "sint" } else { "uint" }, slot));
        slot += 1;
    }
    let head = format!(
        "{}\n\nstruct main0_in_v\n{{\n{}\n{}\n}};",
        &msl[..m.end],
        m.body,
        extra.join("\n")
    );
    let mut body = msl[m.end..].to_string();
    if let Some(cut) = body.find("\nvertex void main0_tf(") {
        body.truncate(cut);
        body.push('\n');
    }
    let mut pro = String::from("    main0_in in = {};\n");
    for (t, n, l) in &members {
        let e = shadows.iter().find(|s| s.0 == *l);
        let ors = match e {
            None => String::new(),
            Some(e) => {
                let mut s = format!(" | as_type<{t}>(dm_in.{n}_vf)");
                if e.2.is_some() {
                    s.push_str(&format!(" | as_type<{t}>(dm_in.{n}_vo)"));
                }
                s
            }
        };
        pro.push_str(&format!("    in.{n} = dm_in.{n}{ors};\n"));
    }
    for entry in ["main0", "main0_z"] {
        let Some((hs, he, rest)) = vertex_header(&body, entry, false) else {
            if entry == "main0" {
                return Err("main0 signature not recognized".into());
            }
            continue;
        };
        body = format!(
            "{}\nvertex main0_out {entry}(main0_in_v dm_in [[stage_in]]{rest})\n{{\n{pro}{}",
            &body[..hs],
            &body[he..]
        );
    }
    let list = shadows
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
    Ok((head + &body, Value::List(list)))
}
