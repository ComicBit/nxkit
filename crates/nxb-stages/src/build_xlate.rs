//! Translation batches of [`crate::build`]: one translator process per batch, then each shader to its finish or gate.

use crate::build_finish::Shared;
use nxb_pyjson::dumps;
use nxb_shader::job::{JobOptions, job_for};
use nxb_tool::XlateJob;
use std::fs;
use std::sync::Arc;
use std::time::Instant;

pub(crate) fn translate_batch(s: &Arc<Shared>, keys: Vec<String>, ctx: &nxb_sched::Ctx<'_>) {
    let t = Instant::now();
    let mut jobs = Vec::new();
    let mut meta = Vec::new();
    for k in &keys {
        let r = s
            .inputs
            .record(k)
            .map_err(|e| e.to_string())
            .and_then(|(pk, rec)| {
                let code = s.scratch.join("x").join(format!("{k}.bin"));
                let j = job_for(
                    k,
                    &pk,
                    &rec,
                    s.inputs.facts.get(k),
                    &code.to_string_lossy(),
                    JobOptions::default(),
                )
                .map_err(|e| e.0)?;
                fs::write(&code, &j.code).map_err(|e| e.to_string())?;
                Ok((pk, j))
            });
        match r {
            Ok((pk, j)) => {
                jobs.push(XlateJob {
                    key: k.clone(),
                    job_json: dumps(&j.json),
                });
                meta.push((k.clone(), pk, j));
            }
            Err(e) => {
                s.result(k, "translation failed", &e);
                crate::build_finish::done(s, k, ctx);
            }
        }
    }
    let res = nxb_tool::Translator::translate(&s.translator, &jobs);
    s.span("translate", &keys[0], t);
    let res = match res {
        Ok(r) => r,
        Err(e) => {
            for (k, _, _) in &meta {
                s.result(k, "translation failed", &e.to_string());
                crate::build_finish::done(s, k, ctx);
            }
            return;
        }
    };
    for ((k, pk, j), r) in meta.into_iter().zip(res) {
        match r.outcome {
            Err(f) => {
                s.result(&k, "translation failed", &f.detail);
                crate::build_finish::done(s, &k, ctx);
            }
            Ok(o) => crate::build_finish::translated(s, k, pk, j, o, ctx),
        }
    }
}
