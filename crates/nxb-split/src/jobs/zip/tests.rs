//! A zip made with Python's `zipfile`: one stored, one bzip2 and one lzma member (with an end marker).

use super::*;
use crate::src::Mem;

const ZIP: &str = concat!(
    "504b030414000000000000002100fe58c68f110000001100000005000000612e74787468656c6c6f207a6970",
    "2073746f7265640a504b03042e0000000c00000021003cb8768c44000000f100000005000000622e747874",
    "425a68393141592653597b29ce6500003c59800010400010001264c0102000508069a680a5480d3d4d71b26",
    "a8f89c93d26a8ef09b260984c27e2ee48a70a120f6539cca0504b03043f0002000e0000002100fe0e835922",
    "000000dd00000005000000632e747874090405005d0000800000341949ee8de9148a6aa5d6bca6ec6f6774",
    "c7ffffe8e38000504b0102140314000000000000002100fe58c68f1100000011000000050000000000000000",
    "000000800100000000612e747874504b01022e032e0000000c00000021003cb8768c44000000f100000005",
    "0000000000000000000000800134000000622e747874504b01023f033f0002000e0000002100fe0e835922",
    "000000dd00000005000000000000000000000080019b000000632e747874504b0506000000000300030099",
    "000000e00000000000"
);

fn hex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .filter_map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok())
        .collect()
}

fn read_all(zip: &[u8]) -> R<Vec<(String, Vec<u8>)>> {
    let s = Mem(zip);
    members(&s)?
        .iter()
        .map(|m| Ok((m.name.clone(), read(&s, m)?)))
        .collect()
}

#[test]
fn bzip2_and_lzma_members_read() {
    let got = read_all(&hex(ZIP)).ok();
    let want = vec![
        ("a.txt".to_string(), b"hello zip stored\n".to_vec()),
        (
            "b.txt".to_string(),
            [&b"hello bzip2 "[..].repeat(20), &b"\n"[..]].concat(),
        ),
        (
            "c.txt".to_string(),
            [&b"hello lzma "[..].repeat(20), &b"\n"[..]].concat(),
        ),
    ];
    assert_eq!(got, Some(want));
}

#[test]
fn a_damaged_member_fails_its_check() {
    let mut z = hex(ZIP);
    // inside the bzip2 member
    z[0x52] ^= 0x04;
    assert!(read_all(&z).is_err());
    let mut z = hex(ZIP);
    // inside the lzma member
    z[0xe2] ^= 0x04;
    assert!(read_all(&z).is_err());
}

/// A larger member through the `xz` command (`--format=lzma`: 13 header bytes, the first five are the properties).
#[test]
fn lzma_against_xz_command() {
    let mut data = Vec::new();
    let mut x = 99u32;
    for i in 0..200_000u32 {
        x = x.wrapping_mul(1103515245).wrapping_add(12345);
        let w = ["shader", "vertex", "pixel", "BNSH", "\0\0\0\0\0", "grsc"][(x >> 16) as usize % 6];
        data.extend_from_slice(w.as_bytes());
        if i % 5 == 0 {
            data.push((x >> 8) as u8);
        }
    }
    let Ok(mut child) = std::process::Command::new("xz")
        .args(["--format=lzma", "-c", "-6"])
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
    let mut member = vec![9, 4, 5, 0];
    member.extend_from_slice(&o.stdout[..5]);
    member.extend_from_slice(&o.stdout[13..]);
    let got = lzma_member(&member, data.len() + 1);
    assert!(got.as_ref().is_ok_and(|g| *g == data), "{:?}", got.err());
}
