//! Jump table bounds on synthetic code: the `GuardedCheck` cases of `dm/tools/tests/test_jump_copies.py`.

use std::collections::HashMap;

use nxb_a64::{Insn, decode, encode};

use crate::jumps::An;

const TABLE: i64 = 0x4040;
const RET: u32 = 0xd65f_03c0;
const NOP: u32 = 0xd503_201f;
/// `ldrsw x18, [x4, x18, lsl #2]; add x18, x18, x4; br x18`
const LOAD: [u32; 3] = [
    0xb8a0_7800 | 18 << 16 | 4 << 5 | 18,
    0x8b04_0252,
    0xd61f_0240,
];

/// `b.<cond>`: eq 0, hs 2, lo 3, hi 8, ls 9, ge 10.
fn bcond(cond: u32, frm: i64, to: i64) -> u32 {
    0x5400_0000 | ((((to - frm) >> 2) as u32) & 0x7ffff) << 5 | cond
}

fn bhi(frm: i64, to: i64) -> u32 {
    bcond(8, frm, to)
}

fn cmp_w(reg: u32, n: u32) -> u32 {
    0x7100_001f | n << 10 | reg << 5
}

fn cbz_w18(frm: i64, to: i64) -> u32 {
    0x3400_0000 | ((((to - frm) >> 2) as u32) & 0x7ffff) << 5 | 18
}

fn b(frm: i64, to: i64) -> u32 {
    encode::b(frm, to).unwrap()
}

/// A `bl` 0x100 ahead.
const BL: u32 = 0x9400_0040;

fn decode_all(words: &[u32]) -> Vec<Insn> {
    let insl: Vec<Insn> = words
        .iter()
        .enumerate()
        .map(|(i, w)| decode(*w, 4 * i as u64).unwrap())
        .collect();
    assert_eq!(insl.len(), words.len());
    insl
}

/// Run a recognizer on `adrp/add x4` + `words` + 8 `ret`s. The table's entries point at the `ret`s.
/// Returns what it found and the first `entries` targets.
fn site(words: &[u32], entries: usize, adr: bool) -> (Option<Vec<i64>>, Vec<i64>) {
    let mut words: Vec<u32> = [
        encode::adrp(4, 0, TABLE).unwrap(),
        encode::addi(4, 4, (TABLE & 0xfff) as u32, 0),
    ]
    .into_iter()
    .chain(words.iter().copied())
    .collect();
    let j = words.iter().position(|w| *w == encode::br(18)).unwrap();
    // with an adr, the entries are relative to the adr, which is 3 before the br
    let rel = if adr { 4 * (j as i64 - 3) } else { TABLE };
    words.extend([RET; 8]);
    let targets: Vec<i64> = (0..8).map(|i| 4 * (words.len() as i64 - 8 + i)).collect();
    let mut img = vec![0u8; 0x5000];
    for (i, t) in targets.iter().enumerate() {
        let at = TABLE as usize + 4 * i;
        img[at..at + 4].copy_from_slice(&((t - rel) as i32).to_le_bytes());
    }
    let insl = decode_all(&words);
    let (jumps, tabjumps) = (HashMap::new(), HashMap::new());
    let an = An {
        img: &img,
        jumps: &jumps,
        tabjumps: &tabjumps,
    };
    let got = if adr {
        an.jump_adr_offsets(&insl, j).unwrap()
    } else {
        an.jump_offsets(&insl, j, false, false, false).unwrap()
    };
    (got, targets[..entries].to_vec())
}

fn check(words: &[u32], entries: usize) {
    let (got, want) = site(words, entries, false);
    assert_eq!(got, Some(want));
}

fn refused(words: &[u32]) {
    assert_eq!(site(words, 0, false).0, None);
}

fn with_load(head: &[u32]) -> Vec<u32> {
    head.iter().chain(LOAD.iter()).copied().collect()
}

#[test]
fn a_nearer_compare_that_guards_nothing_is_not_the_bound() {
    // cmp w18,#4; b.hi exit; cmp w18,#1; ldrsw ...: 5 entries, where the nearest compare gave 2
    check(&with_load(&[cmp_w(18, 4), bhi(12, 80), cmp_w(18, 1)]), 5);
    // the nearer compare feeds a branch that is no bounds check
    check(
        &with_load(&[cmp_w(18, 4), bhi(12, 80), cmp_w(18, 1), bcond(0, 20, 80)]),
        5,
    );
}

#[test]
fn a_nearer_compare_with_its_own_guard_is_the_bound() {
    check(
        &with_load(&[cmp_w(18, 4), bhi(12, 80), cmp_w(18, 1), bhi(20, 80)]),
        2,
    );
}

#[test]
fn a_compare_with_no_branch_is_refused() {
    refused(&with_load(&[cmp_w(18, 4), NOP]));
    // b.eq
    refused(&with_load(&[cmp_w(18, 4), bcond(0, 12, 80)]));
    // b.ge: a signed check lets a negative index through
    refused(&with_load(&[cmp_w(18, 4), bcond(10, 12, 80)]));
}

#[test]
fn the_flags_must_still_be_the_compares() {
    // another compare set them
    refused(&with_load(&[cmp_w(18, 4), cmp_w(0, 1), bhi(16, 80)]));
    // a call in between: the index, w19, survives it, the flags do not
    let ld19 = 0xb8a0_7800 | 19 << 16 | 4 << 5 | 18;
    let mut w = vec![cmp_w(19, 4), BL, bhi(16, 80), ld19];
    w.extend(&LOAD[1..]);
    refused(&w);
    // the same call after the branch
    let mut w = vec![cmp_w(19, 4), bhi(12, 80), BL, ld19];
    w.extend(&LOAD[1..]);
    check(&w, 5);
    // a store between, as Super Smash Bros. Ultimate 13.0.5 main+0x39be740 has
    check(&with_load(&[cmp_w(18, 4), 0xf900_0be0, bhi(16, 80)]), 5);
    // a cbz reads no flags
    check(&with_load(&[cmp_w(18, 4), cbz_w18(12, 80), bhi(16, 80)]), 5);
}

#[test]
fn b_hs_allows_one_entry_fewer_than_b_hi() {
    // cmp #5; b.hs: 0 to 4
    check(&with_load(&[cmp_w(18, 5), bcond(2, 12, 80)]), 5);
    // cmp #4; b.hi: 0 to 4
    check(&with_load(&[cmp_w(18, 4), bhi(12, 80)]), 5);
}

#[test]
fn a_branch_into_the_dispatch_guards_it() {
    // cmp w18,#4; b.ls L; b default; L: ldrsw ...
    check(&with_load(&[cmp_w(18, 4), bcond(9, 12, 20), b(16, 80)]), 5);
    // b.lo: 0 to 4
    check(&with_load(&[cmp_w(18, 5), bcond(3, 12, 20), b(16, 80)]), 5);
    // the failed check falls into the dispatch
    refused(&with_load(&[cmp_w(18, 4), bcond(9, 12, 20), NOP]));
    // b.ls away: the fall-through is the unchecked path
    refused(&with_load(&[cmp_w(18, 4), bcond(9, 12, 80), NOP]));
}

#[test]
fn a_b_hi_into_the_dispatch_is_no_guard() {
    refused(&with_load(&[cmp_w(18, 4), bhi(12, 20), NOP]));
}

#[test]
fn adr_relative_tables_take_the_same_rule() {
    // adrp/add x4; <check>; adr x18,.; ldrsw x0,[x4,x9,lsl #2]; add x18,x18,x0; br x18
    let tail = [
        0x1000_0012,
        0xb8a0_7800 | 9 << 16 | 4 << 5,
        encode::addr(18, 18, 0),
        encode::br(18),
    ];
    let run = |check: &[u32], entries: usize| {
        let w: Vec<u32> = check.iter().chain(tail.iter()).copied().collect();
        site(&w, entries, true)
    };
    let (got, want) = run(&[cmp_w(9, 4), bhi(12, 80), cmp_w(9, 1)], 5);
    assert_eq!(got, Some(want));
    let (got, want) = run(&[cmp_w(9, 5), bcond(2, 12, 80)], 5);
    assert_eq!(got, Some(want));
    assert_eq!(run(&[cmp_w(9, 4), NOP], 5).0, None);
    assert_eq!(run(&[cmp_w(9, 4), bcond(0, 12, 80)], 5).0, None);
}
