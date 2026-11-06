//! Translation of [`crate::build`]: each shader's job is built, then translated in process by `nxb_xlate::msl`.

use crate::build_finish::{Shared, done, gated, prepared};
use crate::xlate_cache::Translation;
use nxb_pyjson::Value;
use nxb_sched::Ctx;
use nxb_shader::job::{Job, JobOptions, job_for};
use nxb_xlate::msl::{Context, Failure};
use std::sync::{Arc, PoisonError};
use std::time::Instant;

/// The job of one shader from the package and the profile tables. The code bytes travel in the job, so the file it
/// names is a label.
fn job(s: &Shared, k: &str) -> Result<Job, String> {
    let (pk, rec) = s.inputs.record(k).map_err(|e| e.to_string())?;
    job_for(
        k,
        &pk,
        &rec,
        s.inputs.facts.get(k),
        &format!("{k}.bin"),
        JobOptions::default(),
    )
    .map_err(|e| e.0)
}

/// Translate one shader. A vertex shader gets the inputs of the pixel shaders that finished before it.
pub(crate) fn translate_one(s: &Shared, key: &str, job: &Job) -> Result<Translation, Failure> {
    let t = Instant::now();
    let (name, index) = s.inputs.loc.get(key).cloned().unwrap_or_default();
    let pc = s.inputs.pc_of.get(key).cloned().unwrap_or_default();
    let partners: Vec<_> = {
        let have = s.inputs_of.lock().unwrap_or_else(PoisonError::into_inner);
        s.pairs
            .get(key)
            .into_iter()
            .flatten()
            .filter_map(|p| have.get(p).cloned())
            .collect()
    };
    let ctx = Context {
        chain: &s.chain,
        source: format!("{name}#{index}"),
        pc_paired: &pc,
        partners: &partners,
        no_facts: job.json.get("no_facts") == Some(&Value::Bool(true)),
    };
    let r = s.xlate.translate(job, &ctx);
    s.span("translate", key, t);
    r
}

/// Translate a batch of shaders. A paired vertex shader only has its job built here: it is translated when its pixel
/// shaders are done.
pub(crate) fn translate_batch(s: &Arc<Shared>, keys: Vec<String>, ctx: &Ctx<'_>) {
    for k in keys {
        let j = match job(s, &k) {
            Ok(j) => j,
            Err(e) => {
                s.result(&k, "translation failed", &e);
                done(s, &k, ctx);
                continue;
            }
        };
        if gated(s, &k) {
            prepared(s, k, j, None, ctx);
            continue;
        }
        match translate_one(s, &k, &j) {
            Ok(t) => prepared(s, k, j, Some(t), ctx),
            Err(f) => {
                s.result(&k, &f.reason, &f.detail);
                done(s, &k, ctx);
            }
        }
    }
}
