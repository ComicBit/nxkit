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

#[test]
fn ldc_is_selects_among_the_runtime_banks() {
    // LDC R1, c[3][R2 + 0x10] with .IS: the bank is 3 + (R2 >> 16), the offset R2 & 0xffff. The banks are c1 and c3.
    let t = ampere_cs_text(&[ampere_ldc(1, 2, 3, 0x10, 2)]).expect("emit");
    assert_eq!(count(&t, "4a"), 2);
    assert_eq!(count(&t, "20"), 2);
    assert!(t.contains("op 14 0 0 0 d=l2, s=R1:2,#ffff"));
}

#[test]
fn ldc_is_reaches_the_banks_other_reads_name() {
    // another LDC reads c5 (and c0, which is left out): the run-time bank can be c1, c3 or c5
    let t = ampere_cs_text(&[
        ampere_ldc(1, 2, 3, 0x10, 2),
        ampere_ldc(4, 255, 5, 0x20, 0),
        ampere_ldc(6, 255, 0, 0x30, 0),
    ])
    .expect("emit");
    // three selects for the .IS read, one plain load for each of the others
    assert_eq!(count(&t, "20"), 3);
    assert_eq!(count(&t, "4a"), 5);
}

#[test]
fn ldc_il_is_refused() {
    let r = ampere_cs_text(&[ampere_ldc(1, 2, 3, 0x10, 1)]);
    assert!(r.is_err_and(|e| e.contains(".IL address mode")));
}

/// `VOTEU` (`uniform`) or `VOTE` with the vote mode `mode` of the lane predicate P0, the result in predicate 2.
fn vote(uniform: bool, mode: u64) -> (u64, u64) {
    let op = if uniform { 0x886 } else { 0x806 };
    let lo = op | APT | 0xff << 16;
    let hi = (mode << (72 - 64)) | (2 << (81 - 64));
    (lo, hi)
}

#[test]
fn voteu_writes_the_uniform_predicate() {
    let t = ampere_cs_text(&[vote(true, 1)]).expect("emit");
    assert!(t.contains("d=R4:2,"));
    assert!(!t.contains("d=R2:2,"));
    let t = ampere_cs_text(&[vote(false, 1)]).expect("emit");
    assert!(t.contains("d=R2:2,"));
}
