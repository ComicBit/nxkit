//! Synthetic emit tests of single instructions (no corpus needed).
#![cfg(test)]

use super::emit_test_util::*;

/// `FSET.GT R1, R2, R3` with PT as the predicate input, optionally `.FTZ`.
fn fset_r(ftz: bool) -> u64 {
    let w = 0x5800u64 << 48 | 1 | 2 << 8 | 3 << 20 | PRED7 | 7 << 39 | 4 << 48;
    if ftz { w | 1 << 55 } else { w }
}

/// Number of lines of the dump that carry the operation `op` (the hex code of the dump).
fn count(text: &str, op: &str) -> usize {
    let pat = format!("  op {op} ");
    text.lines().filter(|l| l.starts_with(&pat)).count()
}

#[test]
fn fset_ftz_flushes_both_operands() {
    let plain = maxwell_cs_text(&[fset_r(false)]).expect("emit");
    let ftz = maxwell_cs_text(&[fset_r(true)]).expect("emit");
    // FPCompareLess FP32 (0x10025) against the smallest normal, once per operand
    assert_eq!(count(&plain, "10025"), 0);
    assert_eq!(count(&ftz, "10025"), 2);
    assert!(ftz.contains("#800000"));
}
