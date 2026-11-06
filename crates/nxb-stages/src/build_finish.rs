//! Per-shader work of [`crate::build`]: the libraries, side libraries and JSON of one translated shader, and the
//! gates of the paired vertex shaders.

use crate::xlate_cache::Translation;
use nxb_sched::{Ctx, Task};
use nxb_shader::job::Job;
use std::sync::Arc;
use std::sync::PoisonError;
use std::time::Instant;

pub(crate) use crate::build_shared::Shared;

/// A paired vertex shader's wait: its job, and its own arrival plus each pixel partner.
pub(crate) struct Gate {
    pub(crate) remaining: usize,
    pub(crate) out: Option<Job>,
}

/// A shader that may run: its job and, unless it waited for its partners, its translation.
pub(crate) struct Ready {
    key: String,
    job: Job,
    tr: Option<Translation>,
}

/// Count one arrival at a vertex shader's gate. `Some` when it may run.
fn arrive(s: &Shared, vs: &str, out: Option<Job>) -> Option<Ready> {
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
    gate.out.map(|job| Ready {
        key: vs.to_string(),
        job,
        tr: None,
    })
}

fn spawn_finish(s: &Arc<Shared>, ready: Option<Ready>, ctx: &Ctx<'_>) {
    if let Some(r) = ready {
        let s2 = s.clone();
        ctx.spawn(
            Task::new(move |ctx| finish(&s2, r, ctx))
                .prio(1)
                .mem(200 << 20),
        );
    }
}

/// Whether a vertex shader waits at a gate.
pub(crate) fn gated(s: &Shared, k: &str) -> bool {
    s.gates
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .contains_key(k)
}

/// A shader's job is ready: a paired vertex shader is counted at its gate, any other shader runs with its translation.
pub(crate) fn prepared(
    s: &Arc<Shared>,
    key: String,
    job: Job,
    tr: Option<Translation>,
    ctx: &Ctx<'_>,
) {
    let ready = if gated(s, &key) {
        arrive(s, &key, Some(job))
    } else {
        Some(Ready { key, job, tr })
    };
    spawn_finish(s, ready, ctx);
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
    // A gated shader whose own job failed: drop its gate so it is not finished later.
    if s.gates
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(k)
        .is_some_and(|g| g.out.is_none())
    {
        let _ = arrive(s, k, None);
    }
}

/// One shader to its outputs. A vertex shader that waited is translated now, with its partners' inputs.
fn finish(s: &Arc<Shared>, r: Ready, ctx: &Ctx<'_>) {
    let t = Instant::now();
    let tr = match r.tr {
        Some(t) => Ok(t),
        None => crate::build_xlate::translate_one(s, &r.key, &r.job),
    };
    let res = tr.map_err(|f| (f.reason, f.detail)).and_then(|tr| {
        let inputs = tr.inputs.clone();
        outputs(s, &r.key, tr)?;
        Ok(inputs)
    });
    s.span("finish", &r.key, t);
    match res {
        Ok(inputs) => {
            s.inputs_of
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(r.key.clone(), inputs);
            s.result(&r.key, "ok", "");
        }
        Err((st, err)) => s.result(&r.key, &st, &err),
    }
    done(s, &r.key, ctx);
}

/// The `_b.json` field a variant library stands for (`_bf`, `_bp`, `_bv`). The library is optional: when it does not
/// build, it and its field are left out.
fn variant_field(suffix: &str) -> Option<&'static str> {
    match suffix {
        "_bf" => Some("fetch"),
        "_bp" => Some("pull"),
        "_bv" => Some("vin"),
        _ => None,
    }
}

/// The test hook of the chain-delta benchmark: an output-changing pass on `NXB_TEST_PASS_SALT` percent of the pixel
/// and compute shaders.
fn salted(key: &str, stage: &str, msl: &str) -> String {
    let mut src = msl.to_string();
    if (stage == "ps" || stage == "cs")
        && let Some(pct) = std::env::var("NXB_TEST_PASS_SALT")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
        && u64::from_str_radix(key.get(..8).unwrap_or("0"), 16).unwrap_or(0) % 100 < pct
    {
        src.push_str("\n// chain-delta test pass\n");
    }
    src
}

/// The libraries and JSON files of a translation: `<key><suffix>.metal`, `.metallib` and, when the library has
/// reflection, `.json`. `_b.json` comes last, without the fields of the variants that did not build.
fn outputs(s: &Shared, key: &str, tr: Translation) -> Result<(), (String, String)> {
    let internal = |e: String| ("internal error".to_string(), e);
    if !tr.libs.iter().any(|l| l.suffix == "_b") {
        return Err((
            "translation failed".into(),
            "no _b library in the translation".into(),
        ));
    }
    // A side unit with no `_bx` library of its own is cut out of `_b`.
    let cut_b = tr
        .side
        .as_ref()
        .filter(|_| !tr.libs.iter().any(|l| l.suffix == "_bx"));
    let mut main_json = None;
    let mut dropped = Vec::new();
    for l in &tr.libs {
        let src = salted(key, &tr.stage, &l.msl);
        if l.suffix == "_b"
            && let Some(sd) = cut_b
        {
            let names: Vec<String> = sd.alpha.iter().chain(&sd.logic).cloned().collect();
            s.side(key, &src, &names)?;
        } else {
            match s.library(&src) {
                Ok(libs) => s
                    .place(key, &format!("{}.metallib", l.suffix), &libs)
                    .map_err(internal)?,
                Err(_) if variant_field(&l.suffix).is_some() => {
                    dropped.extend(variant_field(&l.suffix));
                    continue;
                }
                Err(e) => return Err(("metal failed".into(), e)),
            }
        }
        s.write(key, &format!("{}.metal", l.suffix), src.as_bytes())
            .map_err(internal)?;
        match (&l.json, l.suffix.as_str()) {
            (Some(j), "_b") => main_json = Some(j.clone()),
            (Some(j), suffix) => s.write_json(key, suffix, j.clone()).map_err(internal)?,
            (None, _) => {}
        }
    }
    if let Some(mut j) = main_json {
        for f in dropped {
            j.remove(f);
        }
        s.write_json(key, "_b", j).map_err(internal)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_libraries_are_optional_and_name_their_field() {
        assert_eq!(variant_field("_bf"), Some("fetch"));
        assert_eq!(variant_field("_bp"), Some("pull"));
        assert_eq!(variant_field("_bv"), Some("vin"));
        assert_eq!(variant_field("_b"), None);
        assert_eq!(variant_field("_bx"), None);
    }

    #[test]
    fn pass_salt_changes_only_pixel_and_compute_sources() {
        // Without the variable the source is untouched.
        assert_eq!(salted("0123456789abcdef", "ps", "x"), "x");
        assert_eq!(salted("0123456789abcdef", "vs", "x"), "x");
    }
}
