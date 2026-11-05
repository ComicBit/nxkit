//! The translator bridge contract (design-v2 3, workspace-layout ADR W4).
//!
//! Today's semantics: `dmsh_xlate <job.json> <outdir>` translates a batch of shader records to SPIR-V plus a resources
//! JSON per key. The first implementation is a process per batch with crash and hang recovery (design-v2 9.4: re-run
//! the rest once, then bisect until each failing key is alone), a server implementation follows from the bridge design.
//! Contract of `dmsh_xlate <jobs.json> <outdir>` (read from `dmsh_build.py`): the job file is a JSON array of job
//! objects, for each key that translates it writes `<outdir>/<key>.spv` and `<outdir>/<key>.res.json`, a key it rejects
//! is reported on stderr as `[RED] <key>: <message>`. The exit code is not used. Python's result per key: `[RED]` or no
//! `.spv` means `translation failed` with the message or `no spv` as `err`.

use crate::tool::ToolId;
use nxb_error::{Reason, ToolError};

/// One shader to translate: its key (also the output file stem) and its job object as JSON text, exactly as the
/// shader domain built it (`{"key": …, "stage": …, "code": …, …}`, see `dmsh_build.py` `job_for`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XlateJob {
    /// Output stem: ASCII letters, digits and `_` only (16 hex digits, or with a suffix such as `_g0`).
    pub key: String,
    /// The JSON object for this key.
    pub job_json: String,
}

/// One shader's translation, read from `<key>.spv` and `<key>.res.json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XlateOutput {
    /// SPIR-V words as bytes.
    pub spirv: Vec<u8>,
    /// The resources JSON (`res.json`), as written by the translator.
    pub resources_json: Vec<u8>,
}

/// Why one key has no translation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XlateFailure {
    /// Today's reason string (`translation failed`).
    pub reason: Reason,
    /// The `err` text: the translator's `[RED] <key>: …` message, `no spv`, `timed out`, or `crashed: signal N`.
    pub detail: String,
}

/// The result for one key of a batch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XlateResult {
    /// The key.
    pub key: String,
    /// Its outcome.
    pub outcome: Result<XlateOutput, XlateFailure>,
}

/// The translator, whatever runs it (process per batch, server).
pub trait Translator: Send + Sync {
    /// Identity for TOOL_FP.
    fn identity(&self) -> &ToolId;

    /// Translate a batch. `Ok` holds one result per job, in job order.`Err` is a failure of the whole bridge
    /// (the runtime is missing, the run was cancelled, a machine problem) that recovery cannot reduce to single keys.
    fn translate(&self, batch: &[XlateJob]) -> Result<Vec<XlateResult>, ToolError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use nxb_error::Reason;

    /// A fake bridge shows the trait is object-safe and usable from the stage layer.
    struct Echo(ToolId);

    impl Translator for Echo {
        fn identity(&self) -> &ToolId {
            &self.0
        }
        fn translate(&self, batch: &[XlateJob]) -> Result<Vec<XlateResult>, ToolError> {
            Ok(batch
                .iter()
                .map(|j| XlateResult {
                    key: j.key.clone(),
                    outcome: if j.job_json.is_empty() {
                        Err(XlateFailure {
                            reason: Reason::TranslationFailed,
                            detail: "no spv".into(),
                        })
                    } else {
                        Ok(XlateOutput {
                            spirv: j.job_json.clone().into_bytes(),
                            resources_json: b"{}".to_vec(),
                        })
                    },
                })
                .collect())
        }
    }

    #[test]
    fn object_safe_and_ordered() {
        let t: Box<dyn Translator> = Box::new(Echo(ToolId {
            name: "fake".into(),
            path: "/x".into(),
            version: "0".into(),
        }));
        let (k1, k2) = (
            "0101010101010101".to_string(),
            "0202020202020202".to_string(),
        );
        let out = t
            .translate(&[
                XlateJob {
                    key: k1.clone(),
                    job_json: "{}".into(),
                },
                XlateJob {
                    key: k2,
                    job_json: String::new(),
                },
            ])
            .unwrap();
        assert_eq!(out[0].key, k1);
        assert!(out[0].outcome.is_ok());
        assert_eq!(
            out[1].outcome.as_ref().map_err(|f| f.reason.clone()),
            Err(Reason::TranslationFailed)
        );
    }
}
