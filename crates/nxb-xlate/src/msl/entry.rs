//! The assembly of the base library: the text of `main0`, the helper functions, and the reflection.

use super::args::{self, SET_NAME, SET_TYPE};
use super::emit::{Em, Options, R, Shader, Shared};
use super::refl::Reflection;
use super::spill::{self, Spill};
use super::types::{msl_type, scalar};
use super::usage::{self, Usage};
use super::{Context, Library, Translated, facts, first_store, io, prelude, wide};
use crate::codegen::props::MemoryDef;
use crate::translation::msl_entry::Structured;
use std::fmt::Write;

/// What to write for the main entry: its name and the stage keyword. Variants write further entries with other names.
pub struct EntryName<'a> {
    /// The function name.
    pub name: &'a str,
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

fn shared_params(
    s: &Structured,
    res: &args::Res,
    plan: &io::Plan,
    shader: Shader,
    spill: Option<&Spill>,
) -> R<Shared> {
    let mut p = Shared {
        spill: spill.is_some(),
        ..Shared::default()
    };
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
        let decl = if spill.is_some() {
            Spill::param(m)?
        } else {
            format!("threadgroup {t} (&sm{})[{n}]", m.id)
        };
        p.params.push(d(&decl, &format!("sm{}", m.id)));
    }
    Ok(p)
}

/// The text of one entry function around a body.
fn entry_text(
    s: &Structured,
    e: &EntryName<'_>,
    shader: Shader,
    res: &args::Res,
    plan: &io::Plan,
    body: &str,
    spill: Option<&Spill>,
) -> R<String> {
    let early = shader == Shader::Fragment && (s.props.definitions.early_z_force || res.early);
    let has_out = shader != Shader::Compute && !plan.out_members.is_empty();
    let ret = if has_out { "main0_out" } else { "void" };
    let mut params = Vec::new();
    if !plan.in_members.is_empty() {
        params.push("main0_in in [[stage_in]]".to_string());
    }
    if res.has_args {
        params.push(format!("constant {SET_TYPE}& {SET_NAME} [[buffer(0)]]"));
    }
    for b in &plan.builtins {
        params.push(format!("{} {} [[{}]]", b.ty, b.name, b.attr));
    }
    let builtins: Vec<_> = plan.builtins.iter().copied().collect();
    if let Some(sp) = spill {
        params.extend(sp.entry_params(&builtins));
    }
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
    if let Some(sp) = spill {
        t.push_str(&sp.decls(&s.props.properties.shared_memories, &builtins)?);
    } else {
        for m in &s.props.properties.shared_memories {
            let (ty, n) = memory(m)?;
            let _ = writeln!(t, "    threadgroup {ty} sm{}[{n}];", m.id);
        }
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
    let spill = if shader == Shader::Compute {
        spill::plan(&s.props.properties.shared_memories)
    } else {
        None
    };
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
    let mut shared = shared_params(s, &res, &plan, shader, spill.as_ref())?;
    let first = (shader == Shader::Compute)
        .then(|| first_store::find(&s.info, &res, local_size(s)))
        .flatten();
    if let Some(f) = &first {
        shared.skip = f.skip.clone();
    }
    shared.wide = wide::plan(&s.info, &res, &shared.skip);
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
    msl.push_str(&entry_text(
        s,
        &EntryName { name: "main0" },
        shader,
        &res,
        &plan,
        main,
        spill.as_ref(),
    )?);

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
        ..Reflection::default()
    };
    if let Some(sp) = &spill {
        r.set_extra("tg_spill", nxb_pyjson::Value::Int(sp.total));
    }
    if let Some(f) = first.filter(|f| !f.stores.is_empty()) {
        r.set_extra("first_store", first_store::value(&f));
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
