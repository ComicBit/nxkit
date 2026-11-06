//! `ue_ring` on synthetic images, no game files: the cases of `dm/tools/tests/test_ue_ring.py`.

use crate::ring::ue_ring_find;

const TEXT: usize = 0x8000;
const RO: usize = 0x8000;
const DATA: usize = 0xa000;
const END: usize = 0xc000;
const NAME: usize = 0x8400;
const VAR: usize = 0xa218;
const REG: usize = 0x1000;
const LOCK: usize = 0x2000;
const CTOR: usize = 0x3000;
/// `stur q0, [x0, #0x18]` and `str xzr, [x24, #0x28]!`
const STUR_Q18: u32 = 0x3c81_8000;
const LOC_XZR: u32 = 0xf802_8c1f | 24 << 5;
const LDR_W8_X8: u32 = 0xb940_0108;
const STR_W8_X20_5C: u32 = 0xb900_5e88;
const TBZ_W21_1: u32 = 0x3608_0015 | 3 << 5;

fn put(img: &mut [u8], at: usize, words: &[u32]) {
    for (k, w) in words.iter().enumerate() {
        img[at + 4 * k..at + 4 * k + 4].copy_from_slice(&w.to_le_bytes());
    }
}

fn adrp(rd: u32, pc: usize, target: usize) -> u32 {
    let imm = ((target as i64 & !0xfff) - (pc as i64 & !0xfff)) >> 12;
    0x9000_0000 | ((imm as u32 & 3) << 29) | (((imm >> 2) as u32 & 0x7_ffff) << 5) | rd
}

fn addi(rd: u32, rn: u32, imm12: usize) -> u32 {
    0x9100_0000 | (imm12 as u32) << 10 | rn << 5 | rd
}

fn ldr_w(rt: u32, rn: u32, addr: usize) -> u32 {
    0xb940_0000 | ((addr as u32 & 0xfff) / 4) << 10 | rn << 5 | rt
}

fn ldr_x(rt: u32, rn: u32, addr: usize) -> u32 {
    0xf940_0000 | ((addr as u32 & 0xfff) / 8) << 10 | rn << 5 | rt
}

#[derive(Clone, Copy)]
struct Parts {
    value: u32,
    name: bool,
    registered: bool,
    rotation: bool,
    layout: bool,
}

const FULL: Parts = Parts {
    value: 3,
    name: true,
    registered: true,
    rotation: true,
    layout: true,
};

fn image(p: Parts) -> Vec<u8> {
    let mut img = vec![0u8; END];
    if p.name {
        let n: Vec<u8> = "nvn.NumBufferedFrames\0"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        img[NAME..NAME + n.len()].copy_from_slice(&n);
    }
    put(&mut img, VAR, &[p.value]);
    if p.registered {
        let words = [
            adrp(1, REG, NAME),
            addi(1, 1, NAME & 0xfff),
            adrp(2, REG + 8, VAR),
            addi(2, 2, VAR & 0xfff),
        ];
        put(&mut img, REG, &words);
    }
    if p.rotation {
        // ldr w8, [x21, #0x5c]; cmp w8, #2; b.lo; ldr w9, [x21, #0x60]; add w9, w9, #1; udiv w10, w9, w8;
        // msub w8, w10, w8, w9; str w8, [x21, #0x60]; add x8, x21, w8, uxtw #3; ldr x8, [x8, #8]
        let w = [
            0xb940_5ea8,
            0x7100_091f,
            0x5400_0103,
            0xb940_62a9,
            0x1100_0529,
            0x1ac8_092a,
            0x1b08_a548,
            0xb900_62a8,
            0x8b28_4ea8,
            0xf940_0508,
        ];
        put(&mut img, LOCK, &w);
    }
    if p.layout {
        put(&mut img, CTOR, &[STUR_Q18, LOC_XZR]);
    }
    img
}

/// Unreal Engine 4.27: `tbz; adrp x8; ldr w8, [x8, #lo]; str w8, [x20, #0x5c]`. Returns the load.
fn direct(img: &mut [u8]) -> usize {
    let a = CTOR + 0x40;
    let w = [
        TBZ_W21_1,
        adrp(8, a + 4, VAR),
        ldr_w(8, 8, VAR),
        STR_W8_X20_5C,
    ];
    put(img, a, &w);
    a + 8
}

type Found = Result<(usize, u32, u32, String), String>;

fn find(img: &[u8]) -> Found {
    ue_ring_find(img, TEXT, RO, DATA)
        .expect("no exception")
        .map(|r| (r.off, r.old, r.new, r.why))
}

fn red(f: &Found) -> bool {
    f.as_ref().is_err_and(|w| w.starts_with("[RED] "))
}

#[test]
fn direct_load() {
    let mut img = image(FULL);
    let load = direct(&mut img);
    let (off, old, new, why) = find(&img).expect("found");
    assert_eq!(
        (off, old, new),
        (load, ldr_w(8, 8, VAR), 0x5280_0088),
        "{why}"
    );
}

#[test]
fn load_through_the_got() {
    // Octopath Traveler II: the address comes from a relative relocation
    let mut img = image(FULL);
    let (a, got) = (CTOR + 0x40, DATA + 0x800);
    let w = [
        TBZ_W21_1,
        adrp(8, a + 4, got),
        ldr_x(8, 8, got),
        LDR_W8_X8,
        STR_W8_X20_5C,
    ];
    put(&mut img, a, &w);
    // MOD0 at 0x40, its dynamic section at 0x8800: DT_RELA, DT_RELASZ, then the end
    put(&mut img, 4, &[0x40]);
    img[0x40..0x44].copy_from_slice(b"MOD0");
    put(&mut img, 0x44, &[0x8800 - 0x40]);
    let dynamic: [u64; 6] = [7, 0x8900, 8, 24, 0, 0];
    for (k, v) in dynamic.iter().enumerate() {
        img[0x8800 + 8 * k..0x8800 + 8 * k + 8].copy_from_slice(&v.to_le_bytes());
    }
    let rela: [u64; 3] = [got as u64, 1027, VAR as u64];
    for (k, v) in rela.iter().enumerate() {
        img[0x8900 + 8 * k..0x8900 + 8 * k + 8].copy_from_slice(&v.to_le_bytes());
    }
    let (off, _, new, why) = find(&img).expect("found");
    assert_eq!((off, new), (a + 12, 0x5280_0088), "{why}");
}

#[test]
fn store_through_a_base_made_earlier() {
    // Pikmin 4: `str xzr, [x8, #0x5c]!` in the constructor, then `str w10, [x8]`
    let mut img = image(FULL);
    let a = CTOR + 0x40;
    put(&mut img, CTOR + 8, &[0xf805_cc1f | 8 << 5]);
    put(
        &mut img,
        a,
        &[
            TBZ_W21_1,
            adrp(10, a + 4, VAR),
            ldr_w(10, 10, VAR),
            0xb900_010a,
        ],
    );
    let (off, _, new, why) = find(&img).expect("found");
    assert_eq!((off, new), (a + 8, 0x5280_008a), "{why}");
}

#[test]
fn store_through_a_base_not_at_the_count() {
    let mut img = image(FULL);
    let a = CTOR + 0x40;
    put(
        &mut img,
        a,
        &[
            TBZ_W21_1,
            adrp(10, a + 4, VAR),
            ldr_w(10, 10, VAR),
            0xb900_010a,
        ],
    );
    assert!(red(&find(&img)));
}

#[test]
fn no_name_is_no_ring_and_no_red() {
    let mut img = image(Parts {
        name: false,
        ..FULL
    });
    direct(&mut img);
    let f = find(&img);
    assert!(f.is_err() && !red(&f));
}

#[test]
fn another_default_is_no_ring_and_no_red() {
    // Unreal Engine 5
    let mut img = image(Parts { value: 2, ..FULL });
    direct(&mut img);
    let f = find(&img);
    assert!(f.is_err() && !red(&f));
}

#[test]
fn broken_links_change_nothing_and_say_so() {
    let broken = [
        Parts {
            registered: false,
            ..FULL
        },
        Parts {
            rotation: false,
            ..FULL
        },
        Parts {
            layout: false,
            ..FULL
        },
    ];
    for p in broken {
        let mut img = image(p);
        direct(&mut img);
        assert!(red(&find(&img)));
    }
}

#[test]
fn two_loads_change_nothing() {
    let mut img = image(FULL);
    direct(&mut img);
    let a = CTOR + 0x80;
    put(
        &mut img,
        a,
        &[
            TBZ_W21_1,
            adrp(8, a + 4, VAR),
            ldr_w(8, 8, VAR),
            STR_W8_X20_5C,
        ],
    );
    assert!(red(&find(&img)));
}

#[test]
fn a_load_of_another_variable_is_no_load() {
    let mut img = image(FULL);
    let a = CTOR + 0x40;
    put(
        &mut img,
        a,
        &[
            TBZ_W21_1,
            adrp(8, a + 4, VAR + 8),
            ldr_w(8, 8, VAR + 8),
            STR_W8_X20_5C,
        ],
    );
    assert!(red(&find(&img)));
}
