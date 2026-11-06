//! Helpers of the synthetic emit tests: Maxwell and Ampere programs built from words, translated up to the `emit` dump.
#![cfg(test)]

use super::pipeline::emit_text;
use crate::job::parse_jobs;

/// Predicate field of a Maxwell instruction: `@PT`.
pub const PRED7: u64 = 0x7 << 16;

/// The `emit` dump of a Maxwell compute program. `ops` are the instruction words, scheduling words are inserted.
pub fn maxwell_cs_text(ops: &[u64]) -> Result<String, String> {
    let jobs = parse_jobs(r#"[{"key": "t", "stage": "cs", "local": [1, 1, 1]}]"#)?;
    let mut words = vec![0u64];
    for (i, w) in ops.iter().enumerate() {
        words.push(*w);
        if i % 3 == 2 {
            words.push(0);
        }
    }
    words.extend([RET, 0]);
    let code: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    emit_text(&jobs[0], &code)
}

/// `EXIT`.
pub const RET: u64 = 0xe300_0000_0000_0000 | 0x70000 | 0xf;
