//! The function matcher on a synthetic image with unwind tables (the cases of `dm/tools/tests/test_nso_match.py`
//! that the port reads: the trap word, and the fingerprint around it).

use crate::matchfn::MatchImage;

const RET: u32 = 0xd65f_03c0;
const NOP: u32 = 0xd503_201f;
const MOV: u32 = 0xaa17_03e0;
const TRAP: u32 = 0xe7ff_defe;

fn put(out: &mut [u8], at: usize, words: &[u32]) {
    for (k, w) in words.iter().enumerate() {
        out[at + 4 * k..at + 4 * k + 4].copy_from_slice(&w.to_le_bytes());
    }
}

fn put_i32(out: &mut [u8], at: usize, v: i32) {
    out[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

/// A MOD0 and `.eh_frame_hdr` that describe the functions `(start, words)`, sorted by start.
fn image(functions: &[(usize, Vec<u32>)]) -> Vec<u8> {
    let mut out = vec![0u8; 0x4000];
    put(&mut out, 4, &[0x20]);
    out[0x20..0x24].copy_from_slice(b"MOD0");
    put_i32(&mut out, 0x30, 0x3000 - 0x20);
    put_i32(&mut out, 0x34, 0x300c + functions.len() as i32 * 8 - 0x20);
    out[0x3000..0x3004].copy_from_slice(&[1, 0x1b, 3, 0x3b]);
    put_i32(&mut out, 0x3004, 0x3200 - 0x3004);
    put(&mut out, 0x3008, &[functions.len() as u32]);
    // a zR CIE with pcrel/sdata4 PCs and four-byte lengths
    put(&mut out, 0x3200, &[16, 0]);
    out[0x3208..0x3211].copy_from_slice(b"\x01zR\0\x04\x78\x1e\x01\x1b");
    for (i, (start, words)) in functions.iter().enumerate() {
        put(&mut out, *start, words);
        let fde = 0x3240 + i * 24;
        put_i32(&mut out, 0x300c + i * 8, *start as i32 - 0x3000);
        put_i32(&mut out, 0x300c + i * 8 + 4, fde as i32 - 0x3000);
        put(&mut out, fde, &[16, (fde + 4 - 0x3200) as u32]);
        put_i32(&mut out, fde + 8, *start as i32 - (fde as i32 + 8));
        put(&mut out, fde + 12, &[words.len() as u32 * 4]);
    }
    out
}

/// A function that calls `target` by `bl`, then `b.eq +8`.
fn caller(start: usize, target: usize) -> Vec<u32> {
    let bl = 0x9400_0000 | (((target as i64 - (start as i64 + 12)) / 4) as u32 & 0x03ff_ffff);
    vec![0x9000_0008, 0x9100_0108, MOV, bl, 0x5400_0040, NOP, RET]
}

fn fingerprint(callee: &[u32]) -> Result<String, String> {
    let img = image(&[(0x100, caller(0x100, 0x500)), (0x500, callee.to_vec())]);
    MatchImage::new(&img, 0x2000)?.fingerprint(0x100)
}

#[test]
fn the_trap_word_is_read_and_fingerprinted() {
    let trapped = fingerprint(&[0xd280_0020, RET, TRAP]).expect("trap word");
    // the word itself counts, not the brk it is read as
    let brk = fingerprint(&[0xd280_0020, RET, 0xd420_0000]).expect("brk");
    assert_ne!(trapped, brk);
    // any other undecodable word still refuses
    let e = fingerprint(&[0xd280_0020, RET, 0xffff_ffff]).unwrap_err();
    assert!(e.contains("undecodable"), "{e}");
}
