//! The text of one function around a translated body: an entry function or a body function that entries call.
//!
//! `library` builds the libraries out of these functions. The variants (`variant::*`) write further functions with
//! other names and forms around the same helper functions.

use super::args::{self, SET_NAME, SET_TYPE};
use super::emit::{R, Shader, Shared};
use super::facts_ps::{self, KILL};
use super::io;
use super::types::{msl_type, scalar};
use crate::codegen::props::MemoryDef;
use crate::translation::msl_entry::Structured;
use std::fmt::Write;

/// The statement that ends a vertex entry in the depth convention of GL: the depth range is converted to Metal's.
pub const ZFIX: &str = "out.gl_Position.z = fma(out.gl_Position.z, 0.5, out.gl_Position.w * 0.5);";

/// One parameter of a function.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Param {
    /// The declaration without the attribute (`main0_in in`).
    pub decl: String,
    /// The attribute without the brackets (`stage_in`).
    pub attr: String,
    /// The name the body uses.
    pub name: String,
}

impl Param {
    /// A parameter from its parts.
    pub fn new(decl: impl Into<String>, attr: impl Into<String>, name: impl Into<String>) -> Param {
        Param {
            decl: decl.into(),
            attr: attr.into(),
            name: name.into(),
        }
    }

    /// The declaration with its attribute.
    pub fn full(&self) -> String {
        format!("{} [[{}]]", self.decl, self.attr)
    }
}

/// The parameters and arguments of a list of parameters, each joined by `, `.
pub fn params_decl(ps: &[Param]) -> (String, String) {
    let d: Vec<&str> = ps.iter().map(|p| p.decl.as_str()).collect();
    let a: Vec<&str> = ps.iter().map(|p| p.name.as_str()).collect();
    (d.join(", "), a.join(", "))
}

/// The parameters of the entry functions of a shader: the stage input, the argument buffer and the builtins.
pub fn params(res: &args::Res, plan: &io::Plan) -> Vec<Param> {
    let mut v = Vec::new();
    if !plan.in_members.is_empty() {
        v.push(Param::new("main0_in in", "stage_in", "in"));
    }
    if res.has_args {
        v.push(Param::new(
            format!("constant {SET_TYPE}& {SET_NAME}"),
            "buffer(0)",
            SET_NAME,
        ));
    }
    for b in &plan.builtins {
        v.push(Param::new(format!("{} {}", b.ty, b.name), b.attr, b.name));
    }
    v
}

/// The stage input of an entry function.
#[derive(Clone, Copy, Debug, Default)]
pub enum StageIn<'a> {
    /// `main0_in in [[stage_in]]`, when the plan has inputs.
    #[default]
    Standard,
    /// No stage input. The prologue declares `in`.
    None,
    /// This parameter instead. The prologue fills `in` from it.
    Custom(&'a Param),
}

/// How a function is written.
#[derive(Clone, Copy, Debug, Default)]
pub enum Form {
    /// An entry function with the stage keyword and its attributes.
    #[default]
    Entry,
    /// `static <result> dm_body_<name>(...)`: a function with the parameters without attributes that entries call.
    /// The shared memory is a parameter, because the caller owns it. `inline` forces the inlining.
    Body {
        /// Write `__attribute__((always_inline))`.
        inline: bool,
    },
}

/// What to write for one function. Build it with `EntryName { field: .., ..EntryName::new("main0") }`.
#[derive(Clone, Copy, Debug)]
pub struct EntryName<'a> {
    /// The function name. A body function is `dm_body_<name>`.
    pub name: &'a str,
    /// The form of the function.
    pub form: Form,
    /// The return type instead of `main0_out` or `void`.
    pub ret: Option<&'a str>,
    /// The stage input.
    pub stage_in: StageIn<'a>,
    /// Parameters after the standard ones.
    pub extra: &'a [Param],
    /// Statements before the output is declared.
    pub pro: &'a [String],
}

impl<'a> EntryName<'a> {
    /// An entry function with the standard signature and tail.
    pub fn new(name: &'a str) -> EntryName<'a> {
        EntryName {
            name,
            form: Form::Entry,
            ret: None,
            stage_in: StageIn::Standard,
            extra: &[],
            pro: &[],
        }
    }
}

/// The kind keyword of a shader.
fn kind(s: Shader) -> &'static str {
    match s {
        Shader::Vertex => "vertex",
        Shader::Fragment => "fragment",
        Shader::Compute => "kernel",
    }
}

/// The element type and length of a memory.
fn memory(m: &MemoryDef) -> R<(String, i32)> {
    Ok((msl_type(scalar(m.ty))?, m.array_length.max(1)))
}

/// The parameters every function of the program takes besides its own.
pub(super) fn shared_params(
    s: &Structured,
    res: &args::Res,
    plan: &io::Plan,
    shader: Shader,
) -> R<Shared> {
    let mut p = Shared::default();
    let d = |a: &str, b: &str| (a.to_string(), b.to_string());
    if res.has_args {
        p.params
            .push(d(&format!("constant {SET_TYPE}& {SET_NAME}"), SET_NAME));
    }
    if !plan.in_members.is_empty() {
        p.params.push(d("thread main0_in& in", "in"));
    }
    if !plan.out_members.is_empty() && shader != Shader::Compute {
        p.params.push(d("thread main0_out& out", "out"));
    }
    if shader == Shader::Fragment && facts_ps::helper_discards(&s.info) {
        p.params.push(d(&format!("thread bool& {KILL}"), KILL));
    }
    for b in &plan.builtins {
        p.params.push(d(&format!("{} {}", b.ty, b.name), b.name));
    }
    for m in &s.props.properties.local_memories {
        let (t, n) = memory(m)?;
        p.params.push(d(
            &format!("thread {t} (&lm{})[{n}]", m.id),
            &format!("lm{}", m.id),
        ));
    }
    for m in &s.props.properties.shared_memories {
        let (t, n) = memory(m)?;
        p.params.push(d(
            &format!("threadgroup {t} (&sm{})[{n}]", m.id),
            &format!("sm{}", m.id),
        ));
    }
    Ok(p)
}

/// The parameters of a function: the standard ones, with the stage input of the entry, then its own.
fn all_params(e: &EntryName<'_>, res: &args::Res, plan: &io::Plan) -> Vec<Param> {
    let mut ps = params(res, plan);
    match e.stage_in {
        StageIn::Standard => {}
        StageIn::None => ps.retain(|p| p.attr != "stage_in"),
        StageIn::Custom(c) => {
            ps.retain(|p| p.attr != "stage_in");
            ps.insert(0, c.clone());
        }
    }
    ps.extend(e.extra.iter().cloned());
    ps
}

/// The text of one function around a body.
pub(super) fn entry_text(
    s: &Structured,
    e: &EntryName<'_>,
    shader: Shader,
    res: &args::Res,
    plan: &io::Plan,
    body: &str,
) -> R<String> {
    let early = shader == Shader::Fragment && s.props.definitions.early_z_force;
    let has_out = shader != Shader::Compute && !plan.out_members.is_empty();
    let ret = e.ret.unwrap_or(if has_out { "main0_out" } else { "void" });
    let ps = all_params(e, res, plan);
    let mut t = String::new();
    let body_fn = matches!(e.form, Form::Body { .. });
    match e.form {
        Form::Entry => {
            let attr = if early {
                "[[early_fragment_tests]] "
            } else {
                ""
            };
            let full: Vec<String> = ps.iter().map(Param::full).collect();
            let _ = writeln!(
                t,
                "{attr}{} {ret} {}({})\n{{",
                kind(shader),
                e.name,
                full.join(", ")
            );
        }
        Form::Body { inline } => {
            let mut decls: Vec<String> = ps.iter().map(|p| p.decl.clone()).collect();
            for m in &s.props.properties.shared_memories {
                let (ty, n) = memory(m)?;
                decls.push(format!("threadgroup {ty} (&sm{})[{n}]", m.id));
            }
            let inline = if inline {
                " inline __attribute__((always_inline))"
            } else {
                ""
            };
            let _ = writeln!(
                t,
                "static{inline} {ret} dm_body_{}({})\n{{",
                e.name,
                decls.join(", ")
            );
        }
    }
    for l in e.pro {
        let _ = writeln!(t, "    {l}");
    }
    if has_out {
        t.push_str("    main0_out out = {};\n");
    }
    if shader == Shader::Vertex {
        for m in &plan.vec4_outputs {
            let _ = writeln!(t, "    out.{m} = float4(0.0, 0.0, 0.0, 1.0);");
        }
        for p in &plan.pads {
            let _ = writeln!(t, "    {}", p.init());
        }
    }
    for m in &s.props.properties.local_memories {
        let (ty, n) = memory(m)?;
        let _ = writeln!(t, "    {ty} lm{}[{n}];", m.id);
    }
    if !body_fn {
        for m in &s.props.properties.shared_memories {
            let (ty, n) = memory(m)?;
            let _ = writeln!(t, "    threadgroup {ty} sm{}[{n}];", m.id);
        }
    }
    t.push_str(body);
    let last = body.trim_end().lines().last().map(str::trim);
    if last.is_some_and(|l| l.starts_with("return")) {
        t.push_str("}\n");
        return Ok(t);
    }
    if shader == Shader::Vertex {
        let _ = writeln!(t, "    {ZFIX}");
    }
    if has_out {
        t.push_str("    return out;\n");
    }
    t.push_str("}\n");
    Ok(t)
}
