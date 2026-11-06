//! Vectors made with Python's `bz2` module and round trips through the `bzip2` command.

use super::*;

fn hex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .filter_map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok())
        .collect()
}

const TEXT: &str = concat!(
    "425a68393141592653591a80f9c200000759800010400010001664d090200021290d1a64da840000791cc51b2a5a3422",
    "fcdc1922a11f1772453850901a80f9c2",
);
const RUNS: &str = concat!(
    "425a6831314159265359e114971a0000024181a00030000070008000082000219323210c0854a6016659780f17724538",
    "5090e114971a",
);
const BLOCKS: &str = concat!(
    "425a68313141592653591ed863b500138701003ff0200070400c029543099c54151e550547aa82a3e541518541519541",
    "51a54151b54151faa0a8e2a0a8e98a0ac9329acb7d86bdc00138700801ff81000382006014aa184ce2a0a8f150547b54",
    "151f2a0a8c2a0a8caa0a8d2a0a8daa0a8fd505471505474c50564994d659bac4b5000ea540400ffc08001c100300a550",
    "c2671505478a82a3d54151f550546150546550546950546d50547ea82a38a82a3a6282b24ca6b3c819956e00000f0200",
    "7fe040006200602284d0cc484949a4e0d8f47c3f1c1d17724538509061daac56",
);
const EMPTY: &str = "425a683917724538509000000000";

#[test]
fn text() {
    let got = decompress(&hex(TEXT), 100);
    assert_eq!(
        got.ok().as_deref(),
        Some(&b"hello bzip2 hello bzip2 hello world\n"[..])
    );
}

#[test]
fn runs_use_the_first_run_length_coding() {
    let mut want = vec![b'a'; 1000];
    want.extend_from_slice(b"xyz");
    want.extend_from_slice(&[b'b'; 5]);
    assert_eq!(decompress(&hex(RUNS), 2000).ok(), Some(want));
}

#[test]
fn three_blocks() {
    let want = b"abcdefghij".repeat(30000);
    assert_eq!(decompress(&hex(BLOCKS), want.len()).ok(), Some(want));
}

#[test]
fn empty_stream() {
    assert_eq!(decompress(&hex(EMPTY), 0).ok(), Some(Vec::new()));
}

#[test]
fn the_size_is_a_limit() {
    assert!(decompress(&hex(TEXT), 10).is_err());
}

#[test]
fn damage_is_an_error() {
    let mut d = hex(TEXT);
    let mid = d.len() / 2;
    d[mid] ^= 0x10;
    assert!(decompress(&d, 100).is_err());
    let d = hex(TEXT);
    assert!(decompress(&d[..d.len() - 6], 100).is_err());
    assert!(decompress(b"BZx9", 100).is_err());
}

/// Round trips through the `bzip2` command when it is installed.
#[test]
fn against_bzip2_command() {
    let mut data = Vec::new();
    let mut x = 777u32;
    for i in 0..300_000u32 {
        x = x.wrapping_mul(1103515245).wrapping_add(12345);
        let w = ["shader", "vertex", "pixel", "BNSH", "\0\0\0\0\0", "grsc"][(x >> 16) as usize % 6];
        data.extend_from_slice(w.as_bytes());
        if i % 5 == 0 {
            data.push((x >> 8) as u8);
        }
    }
    for level in ["-1", "-5", "-9"] {
        let Ok(mut child) = std::process::Command::new("bzip2")
            .args([level, "-c"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
        else {
            return;
        };
        let Some(mut i) = child.stdin.take() else {
            return;
        };
        let feed = data.clone();
        let t = std::thread::spawn(move || {
            use std::io::Write;
            let _ = i.write_all(&feed);
        });
        let Ok(o) = child.wait_with_output() else {
            return;
        };
        let _ = t.join();
        let got = decompress(&o.stdout, data.len());
        assert!(
            got.as_ref().is_ok_and(|g| *g == data),
            "level {level}: {:?}",
            got.err()
        );
    }
}
