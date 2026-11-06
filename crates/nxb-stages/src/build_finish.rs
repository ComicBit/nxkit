//! Per-shader work of [`crate::build`]: `dmsh_build.finish` and `fin` for one translated shader.

use crate::front::{PsInfo, ps_reflection};
use crate::vertex::vertex;
use nxb_pyjson::Value;
use nxb_sched::{Ctx, Task};
use nxb_shader::job::Job;
use nxb_shader::msl::{common_passes, cross_args, fs_uint};
use nxb_shader::{side, variants::fetch};
use nxb_tool::XlateOutput;
use std::fs;
use std::sync::Arc;
use std::sync::PoisonError;
use std::time::Instant;

pub(crate) use crate::build_shared::Shared;

/// A paired vertex shader's wait: its translation plus each pixel partner.
pub(crate) struct Gate {
    pub(crate) remaining: usize,
    pub(crate) out: Option<(String, Job, XlateOutput)>,
}

type Ready = Option<(String, String, Job, XlateOutput)>;

/// Count one arrival at a vertex shader's gate.`Some` when it may run.
fn arrive(s: &Shared, vs: &str, out: Option<(String, Job, XlateOutput)>) -> Ready {
    let mut g = s.gates.lock().unwrap_or_else(PoisonError::into_inner);
    let gate = g.get_mut(vs)?;
    if out.is_some() {
        gate.out = out;
    }
    gate.remaining = gate.remaining.saturating_sub(1);
    if gate.remaining > 0 {
        return None;
    }
    let gate = g.remove(vs)?;
    gate.out.map(|(pk, j, o)| (vs.to_string(), pk, j, o))
}

fn spawn_finish(s: &Arc<Shared>, ready: Ready, ctx: &Ctx<'_>) {
    if let Some((k, pk, j, o)) = ready {
        let s2 = s.clone();
        ctx.spawn(
            Task::new(move |ctx| finish(&s2, &k, &pk, &j, o, ctx))
                .prio(1)
                .mem(200 << 20),
        );
    }
}

/// A shader's translation arrived: run it, or count it at its gate (paired vertex shader).
pub(crate) fn translated(
    s: &Arc<Shared>,
    k: String,
    pk: String,
    j: Job,
    o: XlateOutput,
    ctx: &Ctx<'_>,
) {
    let gated = s
        .gates
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .contains_key(&k);
    if gated {
        let ready = arrive(s, &k, Some((pk, j, o)));
        spawn_finish(s, ready, ctx);
    } else {
        spawn_finish(s, Some((k, pk, j, o)), ctx);
    }
}

/// A shader is done (finished or failed): release the vertex shaders that wait for it, a failed gated shader
/// releases nothing of its own.
pub(crate) fn done(s: &Arc<Shared>, k: &str, ctx: &Ctx<'_>) {
    let waiting = s
        .dependents
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(k)
        .unwrap_or_default();
    for v in waiting {
        let ready = arrive(s, &v, None);
        spawn_finish(s, ready, ctx);
    }
    // A gated shader whose own translation failed: drop its gate so it is not finished later.
    if s.gates
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(k)
        .is_some_and(|g| g.out.is_none())
    {
        let _ = arrive(s, k, None);
    }
}

/// One translated shader to its outputs.
pub(crate) fn finish(
    s: &Arc<Shared>,
    key: &str,
    pk: &str,
    job: &Job,
    o: XlateOutput,
    ctx: &Ctx<'_>,
) {
    let t = Instant::now();
    let r = finish_inner(s, key, pk, job, o);
    s.span("finish", key, t);
    match r {
        Ok(()) => s.result(key, "ok", ""),
        Err((st, err)) => s.result(key, &st, &err),
    }
    done(s, key, ctx);
}

fn finish_inner(
    s: &Shared,
    key: &str,
    pk: &str,
    job: &Job,
    o: XlateOutput,
) -> Result<(), (String, String)> {
    let f = |r: &str, d: String| (r.to_string(), d);
    let stage = job
        .json
        .get("stage")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let res = nxb_pyjson::parse(&String::from_utf8_lossy(&o.resources_json))
        .map_err(|e| f("translation failed", e.to_string()))?;
    let (name, k) = s.inputs.loc.get(key).cloned().unwrap_or_default();
    let pc = s.inputs.pc_of.get(key).cloned().unwrap_or_default();
    let c1 = nxb_model::to_hex(&job.c1);
    let _ = pk;
    let finish_json = |refl: Value| -> Result<(), (String, String)> {
        s.write_json(key, refl).map_err(|e| f("internal error", e))
    };
    if stage == "vs" {
        let pixels: Vec<String> = s
            .pairs
            .get(key)
            .map(|ps| {
                ps.iter()
                    .filter_map(|p| fs::read_to_string(s.out.join(format!("{p}_b.metal"))).ok())
                    .collect()
            })
            .unwrap_or_default();
        let cross = |spv: &[u8], a: &[String]| s.cross(spv, a);
        let v = vertex(
            key,
            &job.json,
            &s.translator,
            &s.work,
            &cross,
            &pixels,
            (&name, k.to_string(), &pc, c1),
            &s.chain,
            Some((o.spirv, res)),
        )?;
        let lib = s.library(&v.full).map_err(|e| f("metal failed", e))?;
        s.place(key, "_b.metallib", &lib)
            .map_err(|e| f("internal error", e))?;
        s.write(key, "_b.metal", v.full.as_bytes())
            .map_err(|e| f("internal error", e))?;
        let mut refl = v.refl;
        for (field, suffix, make) in [
            (
                "vin",
                "_bv.metallib",
                nxb_shader::variants::vin::variant as fn(&str) -> Result<(String, Value), String>,
            ),
            ("pull", "_bp.metallib", nxb_shader::variants::pull::variant),
        ] {
            if refl.get(field).is_none() {
                continue;
            }
            match make(&v.full).and_then(|(src, _)| s.library(&src)) {
                Ok(lib) => s
                    .place(key, suffix, &lib)
                    .map_err(|e| f("internal error", e))?,
                Err(_) => {
                    refl.remove(field);
                }
            }
        }
        return finish_json(refl);
    }
    if stage != "ps" && stage != "cs" {
        return Err(f("not ported", format!("stage {stage}")));
    }
    let raw = s
        .cross(
            &o.spirv,
            &cross_args(&o.spirv).map_err(|e| f("MSL failed", e))?,
        )
        .map_err(|e| f("MSL failed", e))?;
    let mut src = common_passes(&raw)?;
    // test hook for the chain-delta benchmark: an output-changing pass on NXB_TEST_PASS_SALT percent of the shaders
    if let Some(pct) = std::env::var("NXB_TEST_PASS_SALT")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        && u64::from_str_radix(key.get(..8).unwrap_or("0"), 16).unwrap_or(0) % 100 < pct
    {
        src.push_str("\n// chain-delta test pass\n");
    }
    // first_store is not ported (it fired on no the reference title kernel): refuse a kernel whose source starts its pattern.
    if stage == "cs" && nxb_shader::msl::first_store_could_apply(&src, job.json.get("local")) {
        return Err(f("not ported", "first_store".into()));
    }
    let fu = if stage == "ps" { fs_uint(&src) } else { None };
    let full = fu.as_ref().map_or_else(|| src.clone(), |x| x.0.clone());
    let mut bx = None;
    if stage == "ps"
        && let Ok(u) = side::unit(&full)
        && s.side(key, &u).map_err(|e| f("internal error", e))?
    {
        bx = Some(u);
    }
    if bx.is_none() {
        let lib = s.library(&full).map_err(|e| f("metal failed", e))?;
        s.place(key, "_b.metallib", &lib)
            .map_err(|e| f("internal error", e))?;
    }
    s.write(key, "_b.metal", full.as_bytes())
        .map_err(|e| f("internal error", e))?;
    let info = PsInfo {
        key,
        chain: &s.chain,
        source: format!("{name}#{k}"),
        pc_paired: &pc,
        stage: &stage,
        src: &src,
        fu: fu.as_ref().map(|x| x.1),
        res: &res,
        job: &job.json,
        c1_hex: c1,
        vs: None,
    };
    let mut refl = ps_reflection(&info).map_err(|e| f("reflection failed", e))?;
    if let Some(u) = bx {
        for (fl, v) in [("alpha", &u.alpha), ("logic", &u.logic)] {
            if !v.is_empty() {
                refl.set(
                    fl,
                    Value::obj([(
                        "entries",
                        Value::List(v.iter().map(|x| Value::str(x.clone())).collect()),
                    )]),
                );
            }
        }
    }
    if stage == "ps"
        && let Ok((fv, meta)) =
            fetch::variant(&src, refl.get("bindings").unwrap_or(&Value::Null), false)
        && let Ok(lib) = s.library(&fv)
    {
        s.place(key, "_bf.metallib", &lib)
            .map_err(|e| f("internal error", e))?;
        refl.set("fetch", meta);
    }
    finish_json(refl)
}
