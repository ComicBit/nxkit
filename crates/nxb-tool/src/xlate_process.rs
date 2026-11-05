//! The translator as one child process per batch, with crash and hang recovery (design-v2 9.4).
//!
//! A normal end (any exit code, as in Python) gives per-key results from `[RED]` lines and output files. A crash
//! (signal) or a hang (watchdog: `base + per_key × keys`) keeps every key that finished, re-runs the rest **once** as
//! one batch, then **bisects** what still fails until each failing key is alone. A key that fails alone gets
//! `translation failed` with `crashed: signal N` or `timed out`. Cost: at most about `2·log2(batch)` extra runs per
//! poison key. A machine problem, a missing translator or a cancel fails the whole call.
//!
//! A key counts as finished after an abnormal end only when both `<key>.spv` and `<key>.res.json` exist (assumed:
//! the translator writes `res.json` after the `.spv`), stale outputs of the batch's keys are removed before each run.

use crate::env::EnvPolicy;
use crate::invocation::Invocation;
use crate::process::{ProcessRegistry, run_process};
use crate::tool::ToolId;
use crate::translator::{Translator, XlateFailure, XlateJob, XlateOutput, XlateResult};
use nxb_error::{Reason, ToolError, ToolFailure};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Settings of the process bridge.
#[derive(Clone, Debug)]
pub struct XlateConfig {
    /// Folder for job files and outputs (the build's private work folder).
    pub workdir: PathBuf,
    /// The translator's environment (`EnvPolicy::translator`).
    pub env: EnvPolicy,
    /// Watchdog base (default 60 s).
    pub base_timeout: Duration,
    /// Watchdog per key (default 10 s).
    pub per_key: Duration,
}

impl XlateConfig {
    /// Defaults from design-v2 9.4 (estimates until history exists).
    pub fn new(workdir: PathBuf, env: EnvPolicy) -> Self {
        XlateConfig {
            workdir,
            env,
            base_timeout: Duration::from_secs(60),
            per_key: Duration::from_secs(10),
        }
    }
}

/// `dmsh_xlate` run as a child per batch.
#[derive(Debug)]
pub struct ProcessTranslator {
    id: ToolId,
    registry: Arc<ProcessRegistry>,
    cfg: XlateConfig,
    runs: AtomicU64,
    job_files: AtomicU64,
}

/// A key's state after one run.
enum KeyState {
    Done(XlateOutput),
    Red(String),
    Missing,
}

type Outcome = Result<XlateOutput, XlateFailure>;

fn fail(detail: impl Into<String>) -> XlateFailure {
    XlateFailure {
        reason: Reason::TranslationFailed,
        detail: detail.into(),
    }
}

/// Keys become file names and are matched in `[RED] (\w+):` lines.
fn valid_key(k: &str) -> bool {
    !k.is_empty() && k.len() <= 128 && k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// `[RED] <key>: <message>` lines, first 160 characters of the message, as Python keeps them.
fn red_lines(stderr: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in stderr.lines() {
        let Some(rest) = line.strip_prefix("[RED] ") else {
            continue;
        };
        if let Some((key, msg)) = rest.split_once(": ")
            && valid_key(key)
        {
            out.insert(key.to_string(), msg.chars().take(160).collect());
        }
    }
    out
}

impl ProcessTranslator {
    /// A bridge for the translator at `id.path`.
    pub fn new(id: ToolId, registry: Arc<ProcessRegistry>, cfg: XlateConfig) -> Self {
        ProcessTranslator {
            id,
            registry,
            cfg,
            runs: AtomicU64::new(0),
            job_files: AtomicU64::new(0),
        }
    }

    /// Translator processes started so far (a metric, recovery shows up here).
    pub fn runs(&self) -> u64 {
        self.runs.load(Ordering::Relaxed)
    }

    fn out_path(&self, key: &str, ext: &str) -> PathBuf {
        self.cfg.workdir.join(format!("{key}.{ext}"))
    }

    fn read_output(&self, key: &str) -> Option<XlateOutput> {
        let spirv = fs::read(self.out_path(key, "spv")).ok()?;
        let resources_json = fs::read(self.out_path(key, "res.json")).ok()?;
        Some(XlateOutput {
            spirv,
            resources_json,
        })
    }

    /// One translator run over `jobs`. `Err` only for failures of the whole bridge.
    fn run_batch(
        &self,
        jobs: &[&XlateJob],
    ) -> Result<(Vec<KeyState>, Option<ToolFailure>), ToolError> {
        for j in jobs {
            let _ = fs::remove_file(self.out_path(&j.key, "spv"));
            let _ = fs::remove_file(self.out_path(&j.key, "res.json"));
        }
        let n = self.job_files.fetch_add(1, Ordering::Relaxed);
        let job_path = self
            .cfg
            .workdir
            .join(format!("job_{}_{n}.json", std::process::id()));
        let body: Vec<&str> = jobs.iter().map(|j| j.job_json.as_str()).collect();
        let text = format!("[\n{}\n]\n", body.join(",\n"));
        fs::write(&job_path, text).map_err(|e| {
            ToolError::new(&self.id.name, ToolFailure::Io(e.kind())).with_env(
                nxb_error::EnvError::classify(
                    &e,
                    nxb_error::IoOp::Write,
                    &job_path.display().to_string(),
                ),
            )
        })?;
        let keys = u32::try_from(jobs.len()).unwrap_or(u32::MAX);
        let inv = Invocation::new(&self.id.path)
            .arg(&job_path)
            .arg(&self.cfg.workdir)
            .env(self.cfg.env.clone())
            .timeout(self.cfg.base_timeout + self.cfg.per_key * keys);
        self.runs.fetch_add(1, Ordering::Relaxed);
        let res = run_process(&self.id.name, &inv, &self.registry);
        let _ = fs::remove_file(&job_path);
        let (stderr, abnormal) = match res {
            Ok(c) => (String::from_utf8_lossy(&c.stderr).into_owned(), None),
            Err(e) if e.env.is_some() => return Err(e),
            Err(e) => match e.failure {
                ToolFailure::Exit(_) => (e.stderr_head, None),
                ToolFailure::Signal(_) | ToolFailure::Timeout(_) => {
                    let f = e.failure.clone();
                    (e.stderr_head, Some(f))
                }
                _ => return Err(e),
            },
        };
        let red = red_lines(&stderr);
        let states = jobs
            .iter()
            .map(|j| {
                if let Some(msg) = red.get(&j.key) {
                    KeyState::Red(msg.clone())
                } else if let Some(out) = self.read_output(&j.key) {
                    KeyState::Done(out)
                } else {
                    KeyState::Missing
                }
            })
            .collect();
        Ok((states, abnormal))
    }

    /// Translate `jobs` (with their indices into the caller's batch), recovering from crashes and hangs.
    fn solve(
        &self,
        jobs: &[(usize, &XlateJob)],
        out: &mut [Option<Outcome>],
        retried: bool,
    ) -> Result<(), ToolError> {
        if jobs.is_empty() {
            return Ok(());
        }
        let refs: Vec<&XlateJob> = jobs.iter().map(|(_, j)| *j).collect();
        let (states, abnormal) = self.run_batch(&refs)?;
        let mut rest = Vec::new();
        for (&(i, j), st) in jobs.iter().zip(states) {
            match (st, &abnormal) {
                (KeyState::Done(o), _) => out[i] = Some(Ok(o)),
                (KeyState::Red(m), _) => out[i] = Some(Err(fail(m))),
                (KeyState::Missing, None) => out[i] = Some(Err(fail("no spv"))),
                (KeyState::Missing, Some(_)) => rest.push((i, j)),
            }
        }
        let Some(failure) = abnormal else {
            return Ok(());
        };
        if rest.len() == 1 && jobs.len() == 1 {
            let detail = match failure {
                ToolFailure::Timeout(_) => "timed out".to_string(),
                ToolFailure::Signal(s) => format!("crashed: signal {s}"),
                other => other.label(),
            };
            out[rest[0].0] = Some(Err(fail(detail)));
            return Ok(());
        }
        if !retried {
            return self.solve(&rest, out, true);
        }
        let (left, right) = rest.split_at(rest.len().div_ceil(2));
        self.solve(left, out, true)?;
        self.solve(right, out, true)
    }
}

impl Translator for ProcessTranslator {
    fn identity(&self) -> &ToolId {
        &self.id
    }

    fn translate(&self, batch: &[XlateJob]) -> Result<Vec<XlateResult>, ToolError> {
        let mut out: Vec<Option<Outcome>> = vec![None; batch.len()];
        let mut jobs = Vec::new();
        for (i, j) in batch.iter().enumerate() {
            if valid_key(&j.key) {
                jobs.push((i, j));
            } else {
                out[i] = Some(Err(fail("invalid key")));
            }
        }
        self.solve(&jobs, &mut out, false)?;
        Ok(batch
            .iter()
            .zip(out)
            .map(|(j, o)| XlateResult {
                key: j.key.clone(),
                outcome: o.unwrap_or_else(|| Err(fail("not translated"))),
            })
            .collect())
    }
}

#[cfg(all(test, unix))]
mod tests;
