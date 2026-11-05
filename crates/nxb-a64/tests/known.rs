//! Fixed instruction words with the text Capstone 5 prints for them, and encoder words from the Python assembler.

use nxb_a64::{decode, encode};

fn text(w: u32) -> String {
    decode(w, 0x100_0000)
        .map(|i| format!("{} {}", i.mn, i.op_str()))
        .unwrap_or_default()
}

#[test]
fn decodes_known_words() {
    assert_eq!(text(0x6b00_003f), "cmp w1, w0");
    assert_eq!(text(0xd61f_0240), "br x18");
    assert_eq!(text(0xa9bf_7bfd), "stp x29, x30, [sp, #-0x10]!");
    assert_eq!(text(0x1a9f_07e2), "cset w2, ne");
    assert_eq!(text(0x5300_7dcd), "lsr w13, w14, #0");
    assert_eq!(text(0x8800_8d94 ^ 0x0001_0000), "stlxr w1, w20, [x12]");
    assert!(decode(0x001e_9f58, 0).is_none());
    let i = decode(0xb400_0040, 0x1000).map(|i| i.target());
    assert_eq!(i, Some(Some(0x1008)));
}

#[test]
fn encoders_match_the_python_assembler() {
    assert_eq!(encode::stp(1, 2, 31, -16), 0xa93f_0be1);
    assert_eq!(encode::mrs_tpidrro(5), 0xd53b_d065);
    assert_eq!(encode::b(0x1000, 0x2000), Ok(0x1400_0400));
    assert!(encode::b(0, 1 << 28).is_err());
    assert_eq!(
        encode::adrp(3, 0x1000, 0x5000),
        Ok(0x9000_0003 | (4 << 29 >> 29 << 29) | (1 << 5))
    );
}
