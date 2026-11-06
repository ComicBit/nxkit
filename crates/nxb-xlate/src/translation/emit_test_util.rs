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

/// Guard of an Ampere instruction: `@PT`.
pub const APT: u64 = 0x7 << 12;

/// `EXIT` of Ampere.
pub const AEXIT: (u64, u64) = (0x94d | APT | 1 << 23, 0);

/// The `emit` dump of an Ampere compute program (`lo`, `hi` of every instruction), `EXIT` appended.
pub fn ampere_cs_text(ops: &[(u64, u64)]) -> Result<String, String> {
    let jobs = parse_jobs(r#"[{"key": "t", "stage": "cs", "local": [1, 1, 1], "arch": "sm86"}]"#)?;
    let mut code = Vec::new();
    for (lo, hi) in ops.iter().chain([AEXIT].iter()) {
        code.extend(lo.to_le_bytes());
        code.extend(hi.to_le_bytes());
    }
    emit_text(&jobs[0], &code)
}

/// `LDC Rd, c[bank][offset]` with the address register `ra` and the address mode `mode` (bits 78..80), 32 bits wide.
pub fn ampere_ldc(rd: u64, ra: u64, bank: u64, offset: u64, mode: u64) -> (u64, u64) {
    let lo = 0xb82 | APT | rd << 16 | ra << 24 | (offset & 0xffff) << 38 | bank << 54;
    let hi = 4u64 << (73 - 64) | mode << (78 - 64);
    (lo, hi)
}
