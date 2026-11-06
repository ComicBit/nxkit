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

/// `BMSK R1, R2, R3`, with `.W` when `w`.
fn bmsk(w: bool) -> (u64, u64) {
    let lo = 0x01b | 1 << 9 | APT | 1 << 16 | 2 << 24 | 3 << 32;
    (lo, if w { 1 << (75 - 64) } else { 0 })
}

#[test]
fn bmsk_clamps_unless_w() {
    let count_geu = |t: &str| count(t, "23");
    let plain = ampere_cs_text(&[bmsk(false)]).expect("emit");
    let wrap = ampere_cs_text(&[bmsk(true)]).expect("emit");
    // the field reaching bit 32, and with the clamping form a and b at least 32 as well
    assert_eq!(count_geu(&plain), 3);
    assert_eq!(count_geu(&wrap), 1);
}

/// `PIXLD.COVERED R1` (0x925 with 78..81 = 1 and no predicate result).
fn pixld() -> (u64, u64) {
    (0x925 | APT | 1 << 16, 1 << (78 - 64) | 7 << (81 - 64))
}

#[test]
fn pixld_reads_the_helper_invocation() {
    let t = ampere_ps_text(&[pixld()]).expect("emit");
    // Load Input, IoVariable 43, and a select of 0 and 1
    assert!(t.contains("op 4a 1 0 0 d=l0, s=#2b,"), "{t}");
}

#[test]
fn pixld_is_refused_outside_a_pixel_shader() {
    let r = ampere_cs_text(&[pixld()]);
    assert!(r.is_err_and(|e| e.contains("0x925 form 1")));
}

/// `UP2UR UR4, URZ, mask`.
fn up2ur(mask: u64) -> (u64, u64) {
    (0x883 | APT | 4 << 16 | 63 << 24 | mask << 32, 0)
}

#[test]
fn up2ur_packs_the_masked_uniform_predicates() {
    let t = ampere_cs_text(&[up2ur(0x41)]).expect("emit");
    assert_eq!(count(&t, "2a"), 2, "{t}");
    assert!(t.contains("R4:0") && t.contains("#40"), "{t}");
}

#[test]
fn up2ur_refuses_a_mask_past_the_predicates() {
    let r = ampere_cs_text(&[up2ur(0x100)]);
    assert!(r.is_err_and(|e| e.contains("mask past the predicates")));
}

/// A 2D texture instruction of opcode `op` (Rd R4, Ra R8, Rb R12, a uniform handle in UR2 for the 0xf.. forms), all four components.
fn tex2d(op: u64) -> (u64, u64) {
    let lo = op | APT | 4 << 16 | 8 << 24 | 12 << 32 | 2 << 40 | 1 << 61;
    (lo, 0xf << 8 | 7 << 17)
}

#[test]
fn tld4_takes_a_register_handle_from_the_second_block() {
    let plain = ampere_cs_text(&[tex2d(0xb63)]).expect("emit");
    let t = ampere_cs_text(&[tex2d(0x364)]).expect("emit");
    assert!(plain.contains("s=l0,l1,#0,"), "{plain}");
    // the handle R12 comes first
    assert!(t.contains("d=l2, s=R1:12,"), "{t}");
    assert!(t.contains("s=l2,l0,l1,#0,"), "{t}");
}

#[test]
fn tld4_and_txd_take_a_uniform_register_handle() {
    let tld4 = ampere_cs_text(&[tex2d(0xf63)]).expect("emit");
    assert!(tld4.contains("s=l0,l1,l2,#0,"), "{tld4}");
    assert!(tld4.contains("d=l0, s=R3:2,"), "{tld4}");
    let txd = ampere_cs_text(&[tex2d(0xf6c)]).expect("emit");
    // the handle, then the two coordinates, then the derivatives
    assert!(txd.contains("s=l0,l1,l2,l3,l4,l5,l6,"), "{txd}");
    assert!(txd.contains("d=l0, s=R3:2,"), "{txd}");
}

#[test]
fn tld_takes_a_register_handle() {
    let (lo, hi) = tex2d(0x367);
    let t = ampere_cs_text(&[(lo, hi | 1 << (87 - 64))]).expect("emit");
    assert!(t.contains("d=l0, s=R1:8,"), "{t}");
    assert!(t.contains("R1:12"), "{t}");
}
