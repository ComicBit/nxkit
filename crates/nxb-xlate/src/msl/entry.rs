//! The assembly of the base library: the text of `main0`, the helper functions, and the reflection.

use super::args::{self, SET_NAME, SET_TYPE};
use super::emit::{Em, Options, R, Shader, Shared};
use super::refl::Reflection;
use super::types::{msl_type, scalar};
use super::usage::{self, Usage};
use super::{Context, Library, Translated, facts, io, mesh, prelude};
use crate::codegen::props::MemoryDef;
use crate::structured::types::AggType;
use crate::translation::msl_entry::Structured;
use nxb_pyjson::Value;
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

/// The element type of a memory and its length, `None` for a scalar.
fn memory(m: &MemoryDef) -> R<(String, Option<i32>)> {
    let len = m.ty.has(AggType::ARRAY).then(|| m.array_length.max(1));
    Ok((msl_type(scalar(m.ty))?, len))
}

/// The declaration of a memory variable and the parameter that passes it on, without the address space.
fn memory_decl(ty: &str, name: &str, len: Option<i32>) -> (String, String) {
    match len {
        Some(n) => (format!("{ty} {name}[{n}]"), format!("{ty} (&{name})[{n}]")),
        None => (format!("{ty} {name}"), format!("{ty}& {name}")),
    }
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
        let name = format!("lm{}", m.id);
        p.params
            .push(d(&format!("thread {}", memory_decl(&t, &name, n).1), &name));
    }
    for m in &s.props.properties.shared_memories {
        let (t, n) = memory(m)?;
        let name = format!("sm{}", m.id);
        p.params.push(d(
            &format!("threadgroup {}", memory_decl(&t, &name, n).1),
            &name,
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

/// One parameter of an entry function: the declaration with its attribute, without it, and the name.
pub struct Param {
    pub attr: String,
    pub plain: String,
    pub name: String,
}

/// The parameters of an entry function.
pub fn entry_params(res: &args::Res, plan: &io::Plan) -> Vec<Param> {
    let mut v = Vec::new();
    let mut add = |plain: String, attr: Option<String>, name: &str| {
        let attr = attr.map_or_else(|| plain.clone(), |a| format!("{plain} [[{a}]]"));
        v.push(Param {
            attr,
            plain,
            name: name.to_string(),
        });
    };
    if !plan.in_members.is_empty() {
        add("main0_in in".into(), Some("stage_in".into()), "in");
    }
    if res.has_args {
        let plain = format!("constant {SET_TYPE}& {SET_NAME}");
        add(plain, Some("buffer(0)".into()), SET_NAME);
    }
    for b in &plan.builtins {
        add(
            format!("{} {}", b.ty, b.name),
            Some(b.attr.to_string()),
            b.name,
        );
    }
    v
}

/// How an entry function is written.
#[derive(Clone, Copy)]
enum Form {
    /// An entry function, with the depth convention of a vertex shader.
    Entry { depth_fix: bool },
    /// The body of a task or mesh stage: a function that takes the parameters without attributes and the stage memory
    /// the wrapper owns.
    StageBody,
}

/// The text of one entry function around a body.
fn entry_text(
    s: &Structured,
    e: &EntryName<'_>,
    shader: Shader,
    res: &args::Res,
    plan: &io::Plan,
    body: &str,
    form: Form,
) -> R<String> {
    let (stage_body, depth_fix) = match form {
        Form::Entry { depth_fix } => (false, depth_fix),
        Form::StageBody => (true, false),
    };
    let early = shader == Shader::Fragment && s.props.definitions.early_z_force;
    let has_out = shader != Shader::Compute && !plan.out_members.is_empty();
    let ret = if has_out { "main0_out" } else { "void" };
    let mut t = String::new();
    if stage_body {
        let mut params: Vec<String> = entry_params(res, plan)
            .into_iter()
            .map(|p| p.plain)
            .collect();
        for m in &s.props.properties.shared_memories {
            let (ty, n) = memory(m)?;
            let p = memory_decl(&ty, &format!("sm{}", m.id), n).1;
            params.push(format!("threadgroup {p}"));
        }
        let _ = writeln!(
            t,
            "static inline __attribute__((always_inline)) void {}({})\n{{",
            e.name,
            params.join(", ")
        );
    } else {
        let params: Vec<String> = entry_params(res, plan)
            .into_iter()
            .map(|p| p.attr)
            .collect();
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
    }
    if has_out {
        t.push_str("    main0_out out = {};\n");
    }
    if shader == Shader::Vertex {
        for m in &plan.vec4_outputs {
            let _ = writeln!(t, "    out.{m} = float4(0.0, 0.0, 0.0, 1.0);");
        }
        if plan.viewport_mask {
            let _ = writeln!(t, "    uint {} = 1u;", io::VIEWPORT_MASK);
        }
    }
    for m in &s.props.properties.local_memories {
        let (ty, n) = memory(m)?;
        let _ = writeln!(t, "    {};", memory_decl(&ty, &format!("lm{}", m.id), n).0);
    }
    for m in s
        .props
        .properties
        .shared_memories
        .iter()
        .filter(|_| !stage_body)
    {
        let (ty, n) = memory(m)?;
        let _ = writeln!(
            t,
            "    threadgroup {};",
            memory_decl(&ty, &format!("sm{}", m.id), n).0
        );
    }
    t.push_str(body);
    let last = body.trim_end().lines().last().map(str::trim);
    if matches!(last, Some("return out;" | "return;")) {
        t.push_str("}\n");
        return Ok(t);
    }
    if shader == Shader::Vertex {
        for l in plan.vertex_epilogue(depth_fix) {
            let _ = writeln!(t, "    {l}");
        }
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
        "cs" | "gs" => Shader::Compute,
        other => return Err(format!("unported: {other} stage")),
    };
    // A task or mesh shader is a compute job, and its tables and reflection are those of its own stage
    let stage_name = job
        .mesh_stage
        .as_ref()
        .map_or(job.stage.as_str(), |m| m.kind.as_str());
    if job.tf || s.props.definitions.transform_feedback_enabled {
        return Err(format!(
            "unported: transform feedback of a {} shader",
            if job.stage == "vs" {
                "vertex"
            } else {
                stage_name
            }
        ));
    }
    if shader == Shader::Compute && tg_bytes(s) > i64::from(super::abi::TG_MAX) {
        return Err("unported: threadgroup memory past 32 KB (tg_spill)".into());
    }
    let usage: Usage = usage::collect(&s.info);
    let res = args::build(
        &s.props, &s.program, &usage, stage_name, &s.notes, &s.fetch1d,
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
    let opt_z = Options {
        no_depth_fix: true,
        ..opt.clone()
    };
    let body_z = if opt.entry_z && shader == Shader::Vertex {
        let mut z = Em::new(&s.info, &s.props, &res, &opt_z, &shared, &plan, shader);
        let body = z.body(0)?;
        em.helpers.extend(z.helpers.iter().copied());
        Some(body)
    } else {
        None
    };
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
    if let Some(m) = &job.mesh_stage {
        msl.push_str(&entry_text(
            s,
            &EntryName {
                name: mesh::BODY_NAME,
            },
            shader,
            &res,
            &plan,
            main,
            Form::StageBody,
        )?);
        let params = entry_params(&res, &plan);
        let words = (job.shared.max(0) + 3) / 4;
        let text = mesh::wrap(m, &params, s.props.properties.shared_memories.len(), words)?;
        msl.push('\n');
        msl.push_str(&text);
    } else {
        msl.push_str(&entry_text(
            s,
            &EntryName { name: "main0" },
            shader,
            &res,
            &plan,
            main,
            Form::Entry {
                depth_fix: !opt.no_depth_fix,
            },
        )?);
    }
    if let Some(body) = &body_z {
        msl.push('\n');
        msl.push_str(&entry_text(
            s,
            &EntryName { name: "main0_z" },
            shader,
            &res,
            &plan,
            body,
            Form::Entry { depth_fix: false },
        )?);
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
        stage: stage_name.to_string(),
        entry: "main0".into(),
        entries: body_z
            .iter()
            .map(|_| ("entry_z".to_string(), Value::str("main0_z")))
            .collect(),
        msl_entry_kind: kind(shader).into(),
        threadgroup_size: (shader == Shader::Compute).then(|| local_size(s)),
        c1: facts::hex(&job.c1),
        notes,
        drv_unfillable: super::refl::unfillable(&reads, stage_name),
        drv_reads: reads,
        bindings: res.bindings.clone(),
        vertex_inputs: facts::vertex_inputs(&plan),
        ..Reflection::default()
    };
    r.raster = facts::raster(shader, &plan, usage.discard, &r.bindings);
    if let Some(m) = &job.mesh_stage {
        mesh::reflect(m, &mut r);
    }
    let mut varyings = plan.varyings.clone();
    varyings.sort_by_key(|v| v.location);
    Ok(Translated {
        key: job.key.clone(),
        stage: stage_name.to_string(),
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
