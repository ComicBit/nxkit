//! Tests of the JIT image with synthetic captures (no game data). The expected bytes come from
//! `dm/tools/jit_capture.py` run on the same captures (`NXB_JIT_DUMP` writes them).

use super::a64::{Kind, adrp_add, branch_target, seqs, set_branch};
use super::*;

const RX1: u64 = 0x1_0000_0000;
const RX2: u64 = 0x1_8000_0000;
const RX3: u64 = 0x1_c000_0000;
const MAIN1: u64 = 0x71_0000_0000;
const MAIN2: u64 = 0x71_8000_0000;
const NRO: u64 = 0x72_0000_0000;
const RET: u32 = 0xd65f03c0;

/// One synthetic request.
#[derive(Clone)]
struct Cap {
    cmd: u32,
    dsize: u64,
    raw: [u8; 32],
    ret: i32,
    o0: u64,
    o1: u64,
    regs: Vec<(&'static str, u64, u64, u64)>,
    code: Vec<u32>,
    ro: Vec<u8>,
    out: Vec<u64>,
}

fn cap(cmd: u32, rx: u64, main: u64, o0: u64, code: Vec<u32>) -> Cap {
    let mut raw = [0u8; 32];
    for (i, x) in raw.iter_mut().enumerate() {
        *x = i as u8 ^ cmd as u8;
    }
    Cap {
        cmd,
        dsize: 28,
        raw,
        ret: 0,
        o0,
        o1: 0,
        regs: vec![
            ("jit_rx", rx, 0x10_0000, 0),
            ("jit_ro", rx + 0x4000_0000, 0x1_0000, 0),
            ("jit_tmem", 0x3_0000_0000, 0x1000, 0),
            ("main", main, 0x2_0000, 0x1000),
            ("nro", NRO, 0x8000, 0),
        ],
        code,
        ro: Vec::new(),
        out: Vec::new(),
    }
}

/// The `DMJ5` bytes of a request (`hle_jit.c`), with an input of 3 bytes.
fn dmj5(c: &Cap) -> Vec<u8> {
    let code: Vec<u8> = c.code.iter().flat_map(|w| w.to_le_bytes()).collect();
    let out: Vec<u8> = c.out.iter().flat_map(|w| w.to_le_bytes()).collect();
    let mut b = b"DMJ5".to_vec();
    b.extend_from_slice(&c.cmd.to_le_bytes());
    for v in [c.o0, code.len() as u64, 3, c.dsize] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(&[0x5a; 32]);
    b.extend_from_slice(&c.raw);
    b.extend_from_slice(&c.raw);
    b.extend_from_slice(&(out.len() as u64).to_le_bytes());
    b.extend_from_slice(&c.ret.to_le_bytes());
    for v in [c.o0, code.len() as u64, c.o1, c.ro.len() as u64] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(&(c.regs.len() as u32).to_le_bytes());
    for (name, base, size, pre) in &c.regs {
        let mut n = [0u8; 16];
        n[..name.len()].copy_from_slice(name.as_bytes());
        b.extend_from_slice(&n);
        for v in [base, size, pre] {
            b.extend_from_slice(&v.to_le_bytes());
        }
    }
    b.extend_from_slice(&code);
    b.extend_from_slice(&c.ro);
    b.extend_from_slice(&out);
    b.extend_from_slice(b"in!");
    b
}

/// `movz x<rd>, #, lsl #48; movk lsl #32; movk lsl #16; movk` of `v`.
fn seq(rd: u32, v: u64) -> Vec<u32> {
    let h = |s: u32| ((v >> s) & 0xffff) as u32;
    vec![
        0xd2e00000 | h(48) << 5 | rd,
        0xf2c00000 | h(32) << 5 | rd,
        0xf2a00000 | h(16) << 5 | rd,
        0xf2800000 | h(0) << 5 | rd,
    ]
}

fn bl(disp: i64) -> u32 {
    0x94000000 | ((disp / 4) as u32 & 0x3ffffff)
}

fn b(disp: i64) -> u32 {
    0x14000000 | ((disp / 4) as u32 & 0x3ffffff)
}

fn cbz(disp: i64) -> u32 {
    0xb4000000 | (((disp / 4) as u32 & 0x7ffff) << 5)
}

/// The boot request: an address into main, a constant, an address into another request's code, a call out, a
/// conditional branch and a bit test inside, and an output with host addresses.
fn boot(rx: u64, main: u64) -> Cap {
    let mut code = seq(1, main + 0x100);
    code.extend(seq(2, 0x1234_5678_9abc));
    code.extend(seq(3, rx + 0x1008));
    code.push(bl(0x1000 - 0x30));
    code.push(0x54000041);
    code.push(0x36000020);
    code.push(RET);
    let mut c = cap(4, rx, main, 0, code);
    c.ro = (1..=8).collect();
    c.o1 = 0x40;
    c.out = vec![rx + 0x1000, main + 0x200, 0x42, main - 0x10];
    c.ret = -2;
    c
}

/// Case A: two boots in other layouts (with padding bytes that differ), a request at another offset in each, a
/// request captured once, and a third session whose code calls code it did not capture.
fn case_a() -> Vec<Vec<(String, Cap)>> {
    let mut s = Vec::new();
    for (n, (rx, main)) in [(RX1, MAIN1), (RX2, MAIN2)].into_iter().enumerate() {
        let mut first = boot(rx, main);
        for x in &mut first.raw[8..24] {
            *x = n as u8 * 0x11;
        }
        let mut second = cap(
            4,
            rx,
            main,
            0x1000,
            vec![0xd503201f, 0xd503201f, cbz(-0x1008), RET],
        );
        second.raw[0] = 0x99;
        let at = if n == 0 { 0x2000 } else { 0x3000 };
        let mut code = seq(4, rx + at + 8);
        code.extend([bl(-(at as i64) - 0x10), RET, 0xd503201f, 0xd503201f]);
        let mut moved = cap(3, rx, main, at, code);
        moved.out = vec![7, 0x1234];
        moved.ro = vec![9, 8, 7, 6];
        let mut f = vec![
            ("a.jit".to_string(), first),
            ("b.jit".to_string(), second),
            ("c.jit".to_string(), moved),
        ];
        if n == 0 {
            let mut once = cap(5, rx, main, 0x4000, vec![b(0x1000 - 0x4000), RET]);
            once.out = vec![0xdead_beef, 0x5_0000_0000];
            f.push(("d.jit".to_string(), once));
        }
        s.push(f);
    }
    s.push(vec![(
        "e.jit".to_string(),
        cap(6, RX3, MAIN1, 0x10, vec![bl(0x8000), RET]),
    )]);
    s
}

/// Case B: one request whose code differs between the two boots (a constant), no profile keys.
fn case_b() -> Vec<Vec<(String, Cap)>> {
    [(RX1, MAIN1, 0x11u64), (RX2, MAIN2, 0x22)]
        .into_iter()
        .map(|(rx, main, k)| {
            let mut code = seq(0, k);
            code.push(RET);
            vec![("x.jit".to_string(), cap(2, rx, main, 0x100, code))]
        })
        .collect()
}

/// `jit_key_data 4:0-7,24-27` and `jit_out_code 4:entries=4,count=0x20004,size=0x20000`.
fn keys_a() -> Keys {
    let mut m = [0u8; 32];
    for i in (0..8).chain(24..28) {
        m[i] = 0xff;
    }
    Keys {
        masks: BTreeMap::from([(4, m)]),
        out_codes: BTreeMap::from([(4, (4, 0x20004, 0x20000))]),
    }
}

fn records(case: &[Vec<(String, Cap)>]) -> Vec<Record> {
    case.iter()
        .flatten()
        .map(|(n, c)| Record::parse(n, &dmj5(c)).unwrap())
        .collect()
}

fn sha(b: &[u8]) -> String {
    nxb_model::sha256(b).to_hex()
}

/// Writes the captures of both cases under `NXB_JIT_DUMP` for `jit_capture.py` (`A/s0`, `A/s1`, `A/s2`, `B/s0`,
/// `B/s1` and the profile `A/prof/game.tsv`).
#[test]
fn dump_for_the_reference() {
    let Some(dir) = std::env::var_os("NXB_JIT_DUMP") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    for (tag, case) in [("A", case_a()), ("B", case_b())] {
        for (i, s) in case.iter().enumerate() {
            let d = dir.join(tag).join(format!("s{i}"));
            std::fs::create_dir_all(&d).unwrap();
            for (n, c) in s {
                std::fs::write(d.join(n), dmj5(c)).unwrap();
            }
        }
    }
    let p = dir.join("A/prof");
    std::fs::create_dir_all(&p).unwrap();
    std::fs::write(
        p.join("game.tsv"),
        "jit_key_data\t4:0-7,24-27\njit_out_code\t4:entries=4,count=0x20004,size=0x20000\n",
    )
    .unwrap();
}

const TSV_A: &str = "# nn::jit image (dm/tools/jit_capture.py) from 2 captures
layout\t0x100000\t0x10000\t0x4000
keydata\t4\tffffffffffffffff00000000000000000000000000000000ffffffff00000000
outcode\t4\t0x4\t0x20004\t0x20000
mod\tmain\t0x104000\t0x20000\t0x1000
mod\tnro\t0x124000\t0x8000\t0x0
req\t38d518e26840320d6324a5e6f71339b7\t4\t0x0\t0x40\t0x40\t0x8\t-2\t0x0\t0x20\t0=jit_rx+1000,8=main+200,18=main+-10
req\tcebfac503f2fc1060e2cb9e46d899088\t4\t0x1000\t0x10\t0x0\t0x0\t0\t0x28\t0x0\t-
req\t521bdf727ea9ba734b6c1c38018c48fd\t3\t0x1040\t0x20\t0x0\t0x4\t0\t0x28\t0x10\t-
req\tf2ecb2938fbae7f6a2a2163ecffdaa66\t5\t0x1080\t0x8\t0x0\t0x0\t0\t0x3c\t0x10\t-
";

const NOTE_SAME: &str =
    "nro was at the same base in both captures; its addresses in the output cannot be told apart";

#[test]
fn case_a_matches_the_reference() {
    let mut notes = Vec::new();
    let img = build(records(&case_a()), &keys_a(), &mut notes).unwrap();
    assert_eq!(img.tsv, TSV_A);
    assert_eq!(img.img.len(), 0x4000);
    assert_eq!(
        sha(&img.img),
        "bb2b5e691588e8c38c19f947d34dd1052d32d0b0e1d288462babb4d58091a7c7"
    );
    assert_eq!(
        sha(&img.out),
        "286986cfb7f33218b0f3d533b3fc8f6bf25267cce6d92bb5b2bc650d79b96754"
    );
    assert_eq!(
        img.summary,
        "4 requests from 2 captures: {4: 2, 3: 1, 5: 1} by command; image 0.0 MB; 3 address sequences made \
         PC-relative, 1 constants left, 4 branches moved, 3 output relocations; 3 requests seen in more than one \
         capture, 0 of them different after relocation"
    );
    let want: Vec<String> = [
        "[RED] capture session 0x1c0000000 left out (code offset 0x8010 of session 0x1c0000000 lies in no \
         captured request): its 1 requests, 1 of them captured in no other session"
            .to_string(),
        format!("[RED] 38d518e26840320d6324a5e6f71339b7: {NOTE_SAME}"),
        format!("[RED] cebfac503f2fc1060e2cb9e46d899088: {NOTE_SAME}"),
        format!("[RED] 521bdf727ea9ba734b6c1c38018c48fd: {NOTE_SAME}"),
    ]
    .into();
    assert_eq!(notes, want);
    assert!(img.disagree.is_empty());
}

#[test]
fn case_b_reports_the_request_that_differs() {
    let mut notes = Vec::new();
    let img = build(records(&case_b()), &Keys::default(), &mut notes).unwrap();
    assert_eq!(
        img.tsv,
        "# nn::jit image (dm/tools/jit_capture.py) from 2 captures
layout\t0x100000\t0x10000\t0x4000
mod\tmain\t0x104000\t0x20000\t0x1000
mod\tnro\t0x124000\t0x8000\t0x0
req\t51cc7616be97a2cfd1f06911efb32db4\t2\t0x100\t0x14\t0x0\t0x0\t0\t0x0\t0x0\t-
"
    );
    assert_eq!(
        sha(&img.img),
        "7fbb3a17c24b35d698a6224fe29026d7a7bbf8f2c25e2aa6291a4cfb75effce2"
    );
    assert!(img.out.is_empty());
    assert_eq!(img.disagree, ["51cc7616be97a2cfd1f06911efb32db4"]);
    assert_eq!(
        notes,
        [
            format!("[RED] 51cc7616be97a2cfd1f06911efb32db4: {NOTE_SAME}"),
            "[RED] requests that differ between captures: 51cc7616be97a2cfd1f06911efb32db4"
                .to_string(),
        ]
    );
    assert!(img.summary.ends_with(
        "1 requests seen in more than one capture, 1 of them different after relocation"
    ));
}

#[test]
fn a_request_captured_once_with_a_host_address_fails() {
    let mut notes = Vec::new();
    let e = build(records(&case_a()), &Keys::default(), &mut notes).unwrap_err();
    assert!(
        e.to_string().contains(
            "df38fc27a21c7da429a6412a39190c3b: output word +0x0 (0x100001000) may be a host address and the \
             request was captured once: capture it again in another run"
        ),
        "{e}"
    );
}

#[test]
fn a_literal_form_fails() {
    let c = cap(1, RX1, MAIN1, 0, vec![0x10000000, RET]);
    let e = build(
        records(&[vec![("l.jit".into(), c)]]),
        &Keys::default(),
        &mut Vec::new(),
    )
    .unwrap_err();
    assert!(
        e.to_string().contains("a literal / adr form at +0x0"),
        "{e}"
    );
}

#[test]
fn an_image_larger_than_the_code_memory_fails() {
    let mut c = cap(1, RX1, MAIN1, 0, vec![RET]);
    c.o0 = 0xffffc;
    c.code = vec![RET; 2];
    let e = build(
        records(&[vec![("l.jit".into(), c)]]),
        &Keys::default(),
        &mut Vec::new(),
    )
    .unwrap_err();
    assert!(
        e.to_string()
            .contains("does not fit the code memory (0x100000)"),
        "{e}"
    );
}

#[test]
fn nothing_left_is_no_captures() {
    let e = build(Vec::new(), &Keys::default(), &mut Vec::new()).unwrap_err();
    assert!(e.to_string().contains("no captures"), "{e}");
    let lost = cap(6, RX3, MAIN1, 0x10, vec![bl(0x8000), RET]);
    let mut notes = Vec::new();
    let e = build(
        records(&[vec![("e.jit".into(), lost)]]),
        &Keys::default(),
        &mut notes,
    )
    .unwrap_err();
    assert!(e.to_string().contains("no captures"), "{e}");
    assert_eq!(notes.len(), 1);
}

#[test]
fn a_bad_file_is_an_error() {
    let c = dmj5(&boot(RX1, MAIN1));
    assert!(Record::parse("x", b"DMJ4").is_err());
    for n in [3, 40, 200, c.len() - 4] {
        let e = Record::parse("x.jit", &c[..n]).unwrap_err();
        assert!(e.to_string().contains("x.jit"), "{e}");
    }
    assert!(Record::parse("x.jit", &c).is_ok());
    let mut odd = boot(RX1, MAIN1);
    odd.regs.retain(|r| r.0 != "jit_rx");
    assert!(Record::parse("x.jit", &dmj5(&odd)).is_err());
}

#[test]
fn the_mask_makes_the_key() {
    let a = Record::parse("a", &dmj5(&boot(RX1, MAIN1))).unwrap();
    let mut other = boot(RX2, MAIN2);
    other.raw[10] = 0x77;
    let b = Record::parse("b", &dmj5(&other)).unwrap();
    assert_ne!(a.key(&Keys::default()), b.key(&Keys::default()));
    assert_eq!(a.key(&keys_a()), b.key(&keys_a()));
    other.raw[3] = 0x77;
    let b = Record::parse("b", &dmj5(&other)).unwrap();
    assert_ne!(a.key(&keys_a()), b.key(&keys_a()));
}

#[test]
fn the_a64_forms() {
    let mut code: Vec<u8> = seq(5, 0x1122_3344_5566_7788)
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();
    code.extend_from_slice(&RET.to_le_bytes());
    assert_eq!(
        seqs(&a64::words(&code)),
        vec![(0, 5, 0x1122_3344_5566_7788)]
    );
    assert_eq!(branch_target(bl(-8), 0x10), Some((Kind::B26, Some(8))));
    assert_eq!(branch_target(cbz(0x40), 0), Some((Kind::B19, Some(0x40))));
    assert_eq!(branch_target(0x36000020, 0), Some((Kind::B14, Some(4))));
    assert_eq!(branch_target(0x58000000, 0), Some((Kind::Lit, None)));
    assert_eq!(branch_target(RET, 0), None);
    assert_eq!(set_branch(bl(0), Kind::B26, -8), Ok(bl(-8)));
    assert!(set_branch(0x36000000, Kind::B14, 1 << 15).is_err());
    assert_eq!(
        adrp_add(3, 0x1000, 0x5123),
        Ok([
            0x90000003 | (1 << 5),
            0x91000000 | (0x123 << 10) | (3 << 5) | 3,
            a64::NOP,
            a64::NOP
        ])
    );
    assert!(adrp_add(0, 0, 1 << 33).is_err());
}
