//! The in-process Rust translator (`nxb-xlate`) behind the [`Translator`] trait, with a per-key fall back to another
//! bridge (the C# program) when the Rust core refuses or fails a key. Every fall back is counted with its reason.

use crate::tool::ToolId;
use crate::translator::{Translator, XlateFailure, XlateJob, XlateOutput, XlateResult};
use nxb_error::{Reason, ToolError};
use nxb_xlate::driver::run_job;
use nxb_xlate::job::parse_jobs;
use nxb_xlate::translation::core::Translator as Core;
use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;

/// Environment switch of the stage layer: `NXB_XLATE_RUST=1` turns this bridge on (it is off by default).
pub const ENV_SWITCH: &str = "NXB_XLATE_RUST";

/// The Rust translator with an optional fall back for the keys it cannot do.
pub struct RustTranslator {
    id: ToolId,
    fallback: Option<Box<dyn Translator>>,
    reasons: Mutex<BTreeMap<String, u64>>,
    rust_keys: std::sync::atomic::AtomicU64,
}

/// The coarse reason of a Rust failure: `unported` for the fail-closed gaps, else `error`.
fn reason_of(msg: &str) -> &'static str {
    if msg.starts_with("unported") {
        "unported"
    } else {
        "error"
    }
}

impl RustTranslator {
    /// A bridge named `id`.`fallback` takes the keys the Rust core refuses (without it they are red).
    pub fn new(id: ToolId, fallback: Option<Box<dyn Translator>>) -> Self {
        RustTranslator {
            id,
            fallback,
            reasons: Mutex::new(BTreeMap::new()),
            rust_keys: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Keys the Rust core translated.
    pub fn rust_keys(&self) -> u64 {
        self.rust_keys.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Keys sent to the fall back, by reason (`unported`, `error`, `panic`), plus the fall back's own counters.
    pub fn fallbacks(&self) -> Vec<(String, u64)> {
        self.reasons
            .lock()
            .map(|m| m.iter().map(|(k, v)| (format!("rust {k}"), *v)).collect())
            .unwrap_or_default()
    }

    fn one(job: &XlateJob) -> Result<XlateOutput, (&'static str, String)> {
        let text = format!("[{}]", job.job_json);
        let jobs = parse_jobs(&text).map_err(|e| ("error", e))?;
        let j = jobs.first().ok_or(("error", "empty job".to_string()))?;
        let code = std::fs::read(&j.code_path).map_err(|e| ("error", format!("code: {e}")))?;
        let r = catch_unwind(AssertUnwindSafe(|| run_job(&Core, j, &code)));
        match r {
            Err(_) => Err(("panic", "panic in the Rust translator".into())),
            Ok(Err(e)) => Err((reason_of(&e), e.lines().next().unwrap_or("").to_string())),
            Ok(Ok(o)) => Ok(XlateOutput {
                spirv: o.spv,
                resources_json: o.res.to_text().into_bytes(),
            }),
        }
    }
}

impl Translator for RustTranslator {
    fn identity(&self) -> &ToolId {
        &self.id
    }

    fn translate(&self, batch: &[XlateJob]) -> Result<Vec<XlateResult>, ToolError> {
        let mut out: Vec<Option<XlateResult>> = Vec::with_capacity(batch.len());
        let mut rest: Vec<(usize, &XlateJob, String)> = Vec::new();
        for (i, job) in batch.iter().enumerate() {
            match Self::one(job) {
                Ok(o) => {
                    self.rust_keys
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    out.push(Some(XlateResult {
                        key: job.key.clone(),
                        outcome: Ok(o),
                    }));
                }
                Err((reason, msg)) => {
                    if let Ok(mut m) = self.reasons.lock() {
                        *m.entry(reason.to_string()).or_default() += 1;
                    }
                    out.push(None);
                    rest.push((i, job, msg));
                }
            }
        }
        if rest.is_empty() {
            return Ok(out.into_iter().flatten().collect());
        }
        match &self.fallback {
            Some(fb) => {
                let jobs: Vec<XlateJob> = rest.iter().map(|(_, j, _)| (*j).clone()).collect();
                let res = fb.translate(&jobs)?;
                for ((i, _, _), r) in rest.iter().zip(res) {
                    out[*i] = Some(r);
                }
            }
            None => {
                for (i, job, msg) in &rest {
                    out[*i] = Some(XlateResult {
                        key: job.key.clone(),
                        outcome: Err(XlateFailure {
                            reason: Reason::TranslationFailed,
                            detail: msg.clone(),
                        }),
                    });
                }
            }
        }
        Ok(out.into_iter().flatten().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(ToolId);
    impl Translator for Fake {
        fn identity(&self) -> &ToolId {
            &self.0
        }
        fn translate(&self, b: &[XlateJob]) -> Result<Vec<XlateResult>, ToolError> {
            Ok(b.iter()
                .map(|j| XlateResult {
                    key: j.key.clone(),
                    outcome: Ok(XlateOutput {
                        spirv: b"fallback".to_vec(),
                        resources_json: b"{}".to_vec(),
                    }),
                })
                .collect())
        }
    }

    fn id(n: &str) -> ToolId {
        ToolId {
            name: n.into(),
            path: "/x".into(),
            version: String::new(),
        }
    }

    #[test]
    fn refused_key_goes_to_the_fallback_and_is_counted() {
        let t = RustTranslator::new(id("r"), Some(Box::new(Fake(id("f")))));
        let job = XlateJob {
            key: "0101010101010101".into(),
            job_json: r#"{"key": "0101010101010101", "stage": "ps", "code": "/nonexistent/x.bin"}"#
                .into(),
        };
        let out = t.translate(std::slice::from_ref(&job)).unwrap();
        assert_eq!(out[0].outcome.as_ref().unwrap().spirv, b"fallback");
        assert_eq!(t.fallbacks(), vec![("rust error".to_string(), 1)]);
        let alone = RustTranslator::new(id("r"), None);
        let out = alone.translate(&[job]).unwrap();
        assert!(out[0].outcome.is_err());
    }
}
