//! The assembly of the base library: the text of `main0`, the helper functions, and the reflection.

use super::args::{self, SET_NAME, SET_TYPE};
use super::emit::{Em, Options, R, Shader, Shared};
use super::refl::Reflection;
use super::types::{msl_type, scalar};
use super::usage::{self, Usage};
use super::{Context, Library, Translated, facts, io, prelude, variant};
use crate::codegen::props::MemoryDef;
use crate::translation::msl_entry::Structured;
use std::fmt::Write;

/// What to write for the main entry: its name and the stage keyword. Variants write further entries with other names.
pub struct EntryName<'a> {
    /// The function name.
    pub name: &'a str,
    /// The return type instead of `main0_out` or `void`.
    pub ret: Option<&'a str>,
    /// The stage input parameter instead of `main0_in in [[stage_in]]`. Empty for none.
    pub in_param: Option<&'a str>,
    /// Parameters after the standard ones.
    pub params: &'a [String],
    /// Statements before the output is declared.
    pub pro: &'a [String],
    /// The statements that end the entry where the body falls through, instead of the default tail.
    pub exit: Option<&'a [String]>,
}

impl<'a> EntryName<'a> {
    /// An entry with the standard signature and tail.
    pub fn new(name: &'a str) -> EntryName<'a> {
        EntryName {
            name,
            ret: None,
            in_param: None,
            params: &[],
            pro: &[],
            exit: None,
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
    let ret = e.ret.unwrap_or(if has_out { "main0_out" } else { "void" });
    let mut params = Vec::new();
    match e.in_param {
        Some("") => {}
        Some(p) => params.push(p.to_string()),
        None if !plan.in_members.is_empty() => params.push("main0_in in [[stage_in]]".to_string()),
        None => {}
    }
    if res.has_args {
        params.push(format!("constant {SET_TYPE}& {SET_NAME} [[buffer(0)]]"));
    }
    for b in &plan.builtins {
        params.push(format!("{} {} [[{}]]", b.ty, b.name, b.attr));
    }
    params.extend(e.params.iter().cloned());
    let mut t = String::new();
    let attr = if early {
        "[[early_fragment_tests]] "
    } else {
        ""
    };
    let _ = writeln!(
        t,
        "{attr}{} {ret} {}({})\n{{",
        kind(shader),
        e.name,
        params.join(", ")
    );
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
    for m in &s.props.properties.shared_memories {
        let (ty, n) = memory(m)?;
        let _ = writeln!(t, "    threadgroup {ty} sm{}[{n}];", m.id);
    }
    t.push_str(body);
    let last = body.trim_end().lines().last().map(str::trim);
    if last.is_some_and(|l| l.starts_with("return")) {
        t.push_str("}\n");
        return Ok(t);
    }
    if let (Shader::Vertex, Some(exit)) = (shader, e.exit) {
        for l in exit {
            let _ = writeln!(t, "    {l}");
        }
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
    build_with(s, ctx, opt, &[])
}

/// Build with the vertex inputs at `ints` declared `uint`.
fn build_with(s: &Structured, ctx: &Context<'_>, opt: &Options, ints: &[u32]) -> R<Translated> {
    let job = &s.job;
    let patched = (!ints.is_empty()).then(|| variant::int_inputs::declare(&s.props, ints));
    let props = patched.as_ref().unwrap_or(&s.props);
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
    let res = args::build(props, &s.program, &usage, &job.stage, &s.notes, &s.fetch1d)?;
    let base_instance = shader == Shader::Vertex
        && res
            .cbs
            .values()
            .any(|b| b.name == "vp_c0" && usage.base_inst.contains(&b.def.binding));
    let lane_zero = shader == Shader::Vertex && opt.visibility_counter;
    let mut plan = io::build(
        &s.info,
        props,
        shader,
        usage.lane_id,
        base_instance,
        lane_zero,
    )?;
    let vs_pad = if shader == Shader::Vertex {
        variant::vertex_pad::apply(&mut plan, ctx.partners)
    } else {
        Vec::new()
    };
    let shared = shared_params(s, &res, &plan, shader)?;
    let mut em = Em::new(&s.info, props, &res, opt, &shared, &plan, shader);
    em.probe = (shader == Shader::Vertex && ints.is_empty()).then(Vec::new);
    let mut bodies = Vec::new();
    for i in 0..s.info.functions.len() {
        bodies.push(em.body(i)?);
    }
    if let Some(uses) = em.probe.take() {
        let declared = s
            .job
            .attr
            .iter()
            .filter_map(|(k, _)| u32::try_from(*k).ok())
            .collect();
        let float = |l: u32| props.definitions.attribute_types.get(l as usize) == Some(&0);
        let found = variant::int_inputs::find(&s.info, &uses, &declared, &float);
        if !found.is_empty() {
            return build_with(s, ctx, opt, &found);
        }
    }
    if opt.visibility_counter && shader == Shader::Vertex {
        em.notes.push(
            "visibility counter lowered to singleton groups (one atomic add per passing vertex)"
                .into(),
        );
    }
    let main = bodies.first().ok_or("program without a main function")?;
    let vx = if shader == Shader::Vertex {
        let w = res.bindings.iter().any(facts::writes);
        Some(variant::vertex::bodies(&mut em, &plan, main.clone(), w)?)
    } else {
        None
    };
    let mut msl = prelude::header(s.props.host.reduced_precision);
    msl.push_str(&prelude::helpers(&em.helpers, res.border));
    msl.push_str(&res.text);
    msl.push_str(&plan.in_struct());
    msl.push_str(&plan.out_struct());
    if let Some(vx) = &vx {
        msl.push_str(&variant::vertex::decls(&plan, vx)?);
    }
    for i in 1..bodies.len() {
        let _ = writeln!(msl, "{};", em.signature(i)?);
    }
    for (i, b) in bodies.iter().enumerate().skip(1) {
        let _ = write!(msl, "{}\n{{\n{b}}}\n", em.signature(i)?);
    }
    match &vx {
        Some(vx) => msl.push_str(&variant::vertex::entries(
            s,
            &res,
            &plan,
            vx,
            &variant::vertex::Frame::default(),
        )?),
        None => msl.push_str(&entry_text(
            s,
            &EntryName::new("main0"),
            shader,
            &res,
            &plan,
            main,
        )?),
    }

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
        vs_pad,
        ..Reflection::default()
    };
    if let Some(vx) = &vx {
        r.entries = variant::vertex::fields(vx);
    }
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
        side: None,
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
