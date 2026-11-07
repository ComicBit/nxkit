//! Ampere ALU results the translator folds, stored to shared memory so they are kept (upstream `b97f0dc6`, encodings
//! checked there with nvdisasm 13.5 for sm_86).
#![cfg(test)]

use super::tensor_tests::msl;

/// `STS.U32 [RZ], R6`.
const STS_R6: (u64, u64) = (0x0000_0006_ff00_7388, 0x000f_e800_0000_0800);
/// `SEL R6, RZ, 0x1, !P0`: R6 = P0 ? 1 : 0.
const SEL_P0: (u64, u64) = (0x0000_0001_ff06_7807, 0x000f_c200_0400_00ff);

/// The MSL of `ops` with R6 stored.
fn kept(ops: &[(u64, u64)]) -> String {
    let mut all = ops.to_vec();
    all.push(STS_R6);
    msl(&all)
}

/// The value the last store writes.
fn stored(ops: &[(u64, u64)]) -> String {
    let m = kept(ops);
    let main = &m[m.find("kernel void main0").expect("entry")..];
    let line = main
        .lines()
        .rfind(|l| l.contains("sm0[") && l.contains("] = "))
        .unwrap_or_else(|| panic!("no store\n{m}"));
    line[line.find("] = ").expect("store") + 4..]
        .trim_end_matches(';')
        .to_string()
}

#[test]
fn popc_immediate_top_bit_is_no_not() {
    // POPC R6, 0x800000ff
    assert_eq!(
        stored(&[(0x8000_00ff_ff06_7909, 0x000f_c200_0000_0000)]),
        "9u"
    );
}

#[test]
fn flo_immediate_top_bit_is_no_not() {
    // FLO.U32 R6, 0x800400ff: 0x800400ff itself, not its bitwise not
    let m = kept(&[(0x8004_00ff_ff06_7900, 0x000f_c200_000e_0000)]);
    assert!(m.contains("dm_find_umsb(2147746047u)"), "{m}");
}

#[test]
fn iadd3_carry_of_two_sets_both_predicates() {
    // IADD3 R7, P0, P1, -RZ, -0x1, -RZ: 2^32 + 0xffffffff + 2^32
    let iadd3 = (0xffff_ffff_ff07_7810, 0x000f_c200_0791_e9ff);
    assert_eq!(stored(&[iadd3, SEL_P0]), "1u");
}

#[test]
fn plop3_second_table_reads_the_old_source() {
    // ISETP.EQ.U32.AND P2, PT, RZ, RZ, PT: P2 = true
    let isetp = (0x0000_00ff_ff00_720c, 0x000f_c200_03f4_2070);
    // PLOP3.LUT P2, P0, P2, PT, PT, 0x0, 0xf0: P2 = false, P0 = the old P2
    let plop3 = (0x0000_0000_00f0_781c, 0x000f_c200_0104_e070);
    assert_eq!(stored(&[isetp, plop3, SEL_P0]), "1u");
}

#[test]
fn lea_hi_sx32_sign_extends() {
    // MOV R5, 0x80000000, then LEA.HI.SX32 R6, R5, RZ, 0x1: (0xffffffff:R5) >> 31
    let mov = (0x8000_0000_ff05_7802, 0x000f_c200_0000_0fff);
    let lea = (0x0000_00ff_0506_7211, 0x000f_c200_078f_0aff);
    assert_eq!(stored(&[mov, lea]), "4294967295u");
}

#[test]
fn lea_negated_zero_b_carries() {
    // LEA R6, P0, RZ, -RZ, 0x0: ~0 + 1 carries out
    let lea = (0x8000_00ff_ff06_7211, 0x000f_c200_0780_00ff);
    assert_eq!(stored(&[lea, SEL_P0]), "1u");
}

#[test]
fn lea_x_not_b() {
    // LEA.X R6, RZ, ~RZ, 0x0, PT: 0 + 0xffffffff + 1
    let lea = (0x8000_00ff_ff06_7211, 0x000f_c200_038e_04ff);
    assert_eq!(stored(&[lea]), "0u");
}
