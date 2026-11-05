//! Test helper: the real-data corpus (`NXB_XLATE_CORPUS` jobs folder with job files, `NXB_XLATE_DUMP` probe dumps).

use crate::job::{Job, parse_jobs};
use std::path::PathBuf;

/// One corpus entry.
pub struct Entry {
    pub job: Job,
    pub code: Vec<u8>,
    pub dump_dir: PathBuf,
}

/// All jobs of the corpus, or `None` when the environment variables are missing.
pub fn load() -> Option<Vec<Entry>> {
    let jobs = PathBuf::from(std::env::var_os("NXB_XLATE_CORPUS")?).join("jobs");
    let dump = PathBuf::from(std::env::var_os("NXB_XLATE_DUMP")?);
    let mut files: Vec<PathBuf> = std::fs::read_dir(jobs)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .collect();
    files.sort();
    let mut out = Vec::new();
    for f in files {
        let text = std::fs::read_to_string(&f).ok()?;
        for job in parse_jobs(&text).ok()? {
            let code = std::fs::read(&job.code_path).ok()?;
            out.push(Entry {
                job,
                code,
                dump_dir: dump.clone(),
            });
        }
    }
    Some(out)
}

impl Entry {
    /// The probe's dump of a phase: the text of the first attempt (up to the next `== attempt`).
    pub fn first_attempt(&self, phase: &str) -> Option<String> {
        let t =
            std::fs::read_to_string(self.dump_dir.join(format!("{}.{phase}.txt", self.job.key)))
                .ok()?;
        let mut parts = t.split("== attempt\n").filter(|p| !p.is_empty());
        parts.next().map(|p| format!("== attempt\n{p}"))
    }
}
