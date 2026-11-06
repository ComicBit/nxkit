//! The assembly of the base library: the text of `main0`, the helper functions, and the reflection.

use super::args::{self, SET_NAME, SET_TYPE};
use super::emit::{Em, Options, R, Shader, Shared};
use super::refl::Reflection;
use super::types::{msl_type, scalar};
use super::usage::{self, Usage};
use super::variant::pixel::Built;
use super::{Context, Library, Translated, facts, io, prelude, variant};
use crate::codegen::props::MemoryDef;
use crate::translation::msl_entry::Structured;
use std::fmt::Write;

/// What to write for the main entry: its name and the stage keyword. Variants write further entries with other names.
pub struct EntryName<'a> {
    /// The function name.
    pub name: &'a str,
    /// Write a `static` function `dm_body_<name>` with the plain parameters instead of the entry function.
    pub body: bool,
}

/// One parameter of an entry function.
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
    /// The declaration with its attribute.
    pub fn full(&self) -> String {
        format!("{} [[{}]]", self.decl, self.attr)
    }
}

/// The parameters of the entry functions of a shader.
pub fn params(res: &args::Res, plan: &io::Plan) -> Vec<Param> {
    let p = |decl: String, attr: String, name: &str| Param {
        decl,
        attr,
        name: name.to_string(),
    };
    let mut v = Vec::new();
    if !plan.in_members.is_empty() {
        v.push(p("main0_in in".into(), "stage_in".into(), "in"));
    }
    if res.has_args {
        v.push(p(
            format!("constant {SET_TYPE}& {SET_NAME}"),
            "buffer(0)".into(),
            SET_NAME,
        ));
    }
    for b in &plan.builtins {
        v.push(p(format!("{} {}", b.ty, b.name), b.attr.into(), b.name));
    }
    v
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

fn shared_params(s: &Structured, res: &args::Res, plan: &io::Plan, shader: Shader) -> R<Shared> {
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

fn tg_bytes(s: &Structured) -> i64 {
    s.props
        .properties
        .shared_memories
        .iter()
        .map(|m| i64::from(m.ty.size_in_bytes()) * i64::from(m.array_length.max(1)))
        .sum()
}

/// The text of one entry function around a body.
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
    let ret = if has_out { "main0_out" } else { "void" };
    let ps = params(res, plan);
    let mut t = String::new();
    if e.body {
        let decls: Vec<&str> = ps.iter().map(|p| p.decl.as_str()).collect();
        let _ = writeln!(
            t,
            "static {ret} dm_body_{}({})\n{{",
            e.name,
            decls.join(", ")
        );
    } else {
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
    if has_out {
        t.push_str("    main0_out out = {};\n");
    }
    if shader == Shader::Vertex {
        for m in &plan.vec4_outputs {
            let _ = writeln!(t, "    out.{m} = float4(0.0, 0.0, 0.0, 1.0);");
        }
    }
    for m in &s.props.properties.local_memories {
        let (ty, n) = memory(m)?;
        let _ = writeln!(t, "    {ty} lm{}[{n}];", m.id);
    }
    for m in &s.props.properties.shared_memories {
        let (ty, n) = memory(m)?;
        let _ = writeln!(t, "    threadgroup {ty} sm{}[{n}];", m.id);
    }
    t.push_str(body);
    let last = body.trim_end().lines().last().map(str::trim);
    if matches!(last, Some("return out;" | "return;")) {
        t.push_str("}\n");
        return Ok(t);
    }
    if shader == Shader::Vertex {
        t.push_str(
            "    out.gl_Position.z = fma(out.gl_Position.z, 0.5, out.gl_Position.w * 0.5);\n",
        );
    }
    if has_out {
        t.push_str("    return out;\n");
    }
    t.push_str("}\n");
    Ok(t)
}

/// Build the base library `_b` and its reflection.
pub fn build(s: &Structured, ctx: &Context<'_>, opt: &Options) -> R<Translated> {
    let job = &s.job;
    let shader = match job.stage.as_str() {
        "vs" => Shader::Vertex,
        "ps" => Shader::Fragment,
        "cs" => Shader::Compute,
        other => return Err(format!("unported: {other} stage")),
    };
    if s.props.definitions.transform_feedback_enabled {
        return Err("unported: transform feedback".into());
    }
    if shader == Shader::Compute && tg_bytes(s) > i64::from(super::abi::TG_MAX) {
        return Err("unported: threadgroup memory past 32 KB (tg_spill)".into());
    }
    let usage: Usage = usage::collect(&s.info);
    let res = args::build(
        &s.props, &s.program, &usage, &job.stage, &s.notes, &s.fetch1d,
    )?;
    let base_instance = shader == Shader::Vertex
        && res
            .cbs
            .values()
            .any(|b| b.name == "vp_c0" && usage.base_inst.contains(&b.def.binding));
    let lane_zero = shader == Shader::Vertex && opt.visibility_counter;
    let plan = io::build(
        &s.info,
        &s.props,
        shader,
        usage.lane_id,
        base_instance,
        lane_zero,
    )?;
    let shared = shared_params(s, &res, &plan, shader)?;
    let mut em = Em::new(&s.info, &s.props, &res, opt, &shared, &plan, shader);
    let mut bodies = Vec::new();
    for i in 0..s.info.functions.len() {
        bodies.push(em.body(i)?);
    }
    if opt.visibility_counter && shader == Shader::Vertex {
        em.notes.push(
            "visibility counter lowered to singleton groups (one atomic add per passing vertex)"
                .into(),
        );
    }
    let mut msl = prelude::header(s.props.host.reduced_precision);
    msl.push_str(&prelude::helpers(&em.helpers, res.border));
    msl.push_str(&res.text);
    msl.push_str(&plan.in_struct());
    msl.push_str(&plan.out_struct());
    for i in 1..bodies.len() {
        let _ = writeln!(msl, "{};", em.signature(i)?);
    }
    for (i, b) in bodies.iter().enumerate().skip(1) {
        let _ = write!(msl, "{}\n{{\n{b}}}\n", em.signature(i)?);
    }
    let main = bodies.first().ok_or("program without a main function")?;
    let built = if shader == Shader::Fragment {
        variant::pixel::assemble(s, &res, &plan, main, &[])?
    } else {
        let e = EntryName {
            name: "main0",
            body: false,
        };
        Built {
            text: entry_text(s, &e, shader, &res, &plan, main)?,
            ..Built::default()
        }
    };
    msl.push_str(&built.text);

    let mut notes = s.notes.clone();
    notes.extend(em.notes.iter().cloned());
    if ctx.no_facts {
        notes.push("not in the answer table, queries use defaults".into());
    }
    let reads = facts::drv_reads(&em.drv);
    let mut r = Reflection {
        key: format!("{}_b", job.key),
        chain: ctx.chain.to_string(),
        dm_ctl: job.key.clone(),
        source: ctx.source.clone(),
        pc_paired: ctx.pc_paired.to_string(),
        stage: job.stage.clone(),
        entry: "main0".into(),
        msl_entry_kind: kind(shader).into(),
        threadgroup_size: (shader == Shader::Compute).then(|| local_size(s)),
        c1: facts::hex(&job.c1),
        notes,
        drv_unfillable: super::refl::unfillable(&reads, &job.stage),
        drv_reads: reads,
        bindings: res.bindings.clone(),
        vertex_inputs: facts::vertex_inputs(&plan),
        entries: built.entries,
        extra: built.extra,
        ..Reflection::default()
    };
    r.raster = facts::raster(shader, &plan, usage.discard, &r.bindings);
    let mut varyings = plan.varyings.clone();
    varyings.sort_by_key(|v| v.location);
    Ok(Translated {
        key: job.key.clone(),
        stage: job.stage.clone(),
        libraries: vec![Library {
            suffix: "_b".into(),
            msl,
            reflection: Some(r),
        }],
        inputs: super::StageInputs { varyings },
        side: built.side,
    })
}

fn local_size(s: &Structured) -> [u32; 3] {
    let d = s.props.definitions.compute_local_size;
    let from_job = s.job.local.as_deref().unwrap_or(&[]);
    let at = |i: usize| {
        let j = from_job.get(i).copied().unwrap_or(0);
        u32::try_from(if j > 0 { j } else { i64::from(d[i]) })
            .unwrap_or(1)
            .max(1)
    };
    [at(0), at(1), at(2)]
}
