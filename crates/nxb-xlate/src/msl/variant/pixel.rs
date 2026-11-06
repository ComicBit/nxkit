//! The fragment entries of a pixel shader: `main0`, the derived result entries and the side entries.
//!
//! A shader with no extra entry gets the plain `main0`. Otherwise the main body becomes the static function
//! `dm_body_main0`, and each entry (`main0`, `main0_u`, `main0_m`, `main0_d`, `<entry>_a`, `<entry>_l`) calls it.

use super::side;
use crate::msl::Side;
use crate::msl::args::Res;
use crate::msl::emit::{R, Shader};
use crate::msl::entry::{EntryName, Form, Param, entry_text, params, params_decl};
use crate::msl::io::{FragOut, Plan};
use crate::translation::msl_entry::Structured;
use nxb_pyjson::Value;
use std::collections::BTreeSet;
use std::fmt::Write;

/// One fragment entry: its name, result type and result members.
#[derive(Clone, Debug)]
pub struct Ent<'a> {
    /// The entry function name.
    pub name: String,
    /// The result type: a struct name or `void`.
    pub ret: String,
    /// The members of the result struct.
    pub members: &'a [FragOut],
}

/// An entry that returns the result of `main0` converted member by member.
#[derive(Clone, Debug)]
pub struct Derived {
    /// The entry name (`main0_u`, `main0_m`, `main0_d`).
    pub name: String,
    /// The members of the result struct and the expression of each, over `dm_r` (the result of `main0`).
    pub members: Vec<(FragOut, String)>,
    /// The colours converted to integers (`main0_m`).
    pub uint_mask: Option<u32>,
}

/// What the entries add to the base library.
#[derive(Debug, Default)]
pub struct Built {
    /// The text of all fragment entries.
    pub text: String,
    /// The entries for the side library.
    pub side: Option<Side>,
    /// The reflection fields that name the derived entries.
    pub entries: Vec<(String, Value)>,
    /// The reflection fields `alpha` and `logic`.
    pub extra: Vec<(String, Value)>,
}

fn struct_name(entry: &str) -> String {
    format!("main0_out{}", entry.strip_prefix("main0").unwrap_or(""))
}

fn names(v: &[String]) -> Value {
    Value::obj([(
        "entries",
        Value::List(v.iter().map(|n| Value::str(n.clone())).collect()),
    )])
}

fn thin(name: &str, ret: &str, ps: &[Param], early: bool) -> String {
    let full: Vec<String> = ps.iter().map(Param::full).collect();
    format!(
        "{}fragment {ret} {name}({})\n{{\n    return dm_body_{name}({});\n}}\n",
        if early {
            "[[early_fragment_tests]] "
        } else {
            ""
        },
        full.join(", "),
        params_decl(ps).1
    )
}

/// The body function of a derived entry.
fn derived_body(d: &Derived, ps: &[Param]) -> String {
    let st = struct_name(&d.name);
    let mut t = String::new();
    let _ = writeln!(t, "struct {st}\n{{");
    for (m, _) in &d.members {
        let _ = writeln!(t, "    {} {} [[{}]];", m.ty, m.name, m.attr);
    }
    t.push_str("};\n");
    let _ = writeln!(
        t,
        "static {st} dm_body_{}({})\n{{\n    main0_out dm_r = dm_body_main0({});\n    {st} out = {{}};",
        d.name,
        params_decl(ps).0,
        params_decl(ps).1
    );
    for (m, e) in &d.members {
        let _ = writeln!(t, "    out.{} = {e};", m.name);
    }
    t.push_str("    return out;\n}\n");
    t
}

/// Write the fragment entries.
pub fn assemble(
    s: &Structured,
    res: &Res,
    plan: &Plan,
    body: &str,
    derived: &[Derived],
) -> R<Built> {
    let early = s.props.definitions.early_z_force;
    let ps = params(res, plan);
    let owned: Vec<Vec<FragOut>> = derived
        .iter()
        .map(|d| d.members.iter().map(|(m, _)| m.clone()).collect())
        .collect();
    let mut es = vec![Ent {
        name: "main0".into(),
        ret: if plan.out_members.is_empty() {
            "void".into()
        } else {
            "main0_out".into()
        },
        members: &plan.frag_outs,
    }];
    for (d, o) in derived.iter().zip(&owned) {
        es.push(Ent {
            name: d.name.clone(),
            ret: struct_name(&d.name),
            members: o,
        });
    }
    let alpha_ok = side::alpha_refusal(&es, &ps).is_none();
    let alphas: Vec<(String, String)> = if alpha_ok {
        es.iter()
            .filter_map(|e| side::alpha_entry(e, &ps, early).map(|t| (format!("{}_a", e.name), t)))
            .collect()
    } else {
        Vec::new()
    };
    let logics: Vec<(String, (String, BTreeSet<u32>))> = es
        .iter()
        .filter_map(|e| side::logic_entry(e, &ps, early).map(|o| (format!("{}_l", e.name), o)))
        .collect();
    if derived.is_empty() && alphas.is_empty() && logics.is_empty() {
        let text = entry_text(
            s,
            &EntryName::new("main0"),
            Shader::Fragment,
            res,
            plan,
            body,
        )?;
        return Ok(Built {
            text,
            ..Built::default()
        });
    }
    let mut t = entry_text(
        s,
        &EntryName {
            form: Form::Body { inline: false },
            ..EntryName::new("main0")
        },
        Shader::Fragment,
        res,
        plan,
        body,
    )?;
    for d in derived {
        t.push_str(&derived_body(d, &ps));
    }
    let mut targets = BTreeSet::new();
    for (_, (_, ts)) in &logics {
        targets.extend(ts.iter().copied());
    }
    if !alphas.is_empty() {
        t.push_str(side::ALPHA_HELPER);
    }
    if !logics.is_empty() {
        t.push_str(side::LOGIC_HELPER);
        t.push_str(&side::logic_constants(&targets));
    }
    for e in &es {
        t.push_str(&thin(&e.name, &e.ret, &ps, early));
    }
    for (_, text) in &alphas {
        t.push_str(text);
    }
    for (_, (text, _)) in &logics {
        t.push_str(text);
    }
    let mut b = Built {
        text: t,
        ..Built::default()
    };
    for d in derived {
        let key = match d.name.as_str() {
            "main0_u" => "entry_u",
            "main0_m" => "entry_m",
            _ => "entry_d",
        };
        b.entries.push((key.into(), Value::str(d.name.clone())));
        if let Some(m) = d.uint_mask {
            b.entries
                .push(("uint_mask_m".into(), Value::Int(i64::from(m))));
        }
    }
    let alpha: Vec<String> = alphas.into_iter().map(|a| a.0).collect();
    let logic: Vec<String> = logics.into_iter().map(|l| l.0).collect();
    if !alpha.is_empty() {
        b.extra.push(("alpha".into(), names(&alpha)));
    }
    if !logic.is_empty() {
        b.extra.push(("logic".into(), names(&logic)));
    }
    if !alpha.is_empty() || !logic.is_empty() {
        b.side = Some(Side { alpha, logic });
    }
    Ok(b)
}
