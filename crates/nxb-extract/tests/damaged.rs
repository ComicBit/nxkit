//! `--allow-damaged`, `--manifest`, `--aoc` and sparse programs on synthetic packages.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;
use common::*;
use nxb_extract::{ExtractError, Request};

const APP: u64 = 0x0100_0000_0000_1000;

fn request(keys: &std::path::Path, base: &std::path::Path, out: &std::path::Path) -> Request {
    Request {
        keys: keys.to_path_buf(),
        base: base.to_path_buf(),
        out_base: out.to_path_buf(),
        threads: 2,
        mem_limit: 1 << 28,
        cache_chunks: 16,
        ..Request::default()
    }
}

/// A base game NSP with the RomFS files `rom`. `flip`: the byte of the file `marker` (a run of that value) to flip, `sparse`: mark the data
/// section sparse.
fn package(rom: &[(&str, Vec<u8>)], flip: Option<(u8, usize)>, sparse: bool) -> Vec<u8> {
    let exefs = vec![
        ("main".to_string(), vec![3u8; 0x500]),
        ("main.npdm".into(), npdm(APP)),
    ];
    let mut data = romfs_section(rom);
    if sparse {
        data.fs[0x170] = 1;
    }
    let (mut prog, plain) = nca(0, APP, vec![(0, pfs0_section(&exefs)), (1, data)]);
    if let Some((marker, k)) = flip {
        let (start, bytes, _) = &plain[1];
        let at = bytes
            .windows(64)
            .position(|w| w.iter().all(|&b| b == marker))
            .unwrap();
        prog[*start as usize + at + k] ^= 0x55;
    }
    let pid = [7u8; 16];
    let meta = pfs0_section(&[("m.cnmt".into(), cnmt(0x80, APP, 0, &[pid]))]);
    let (m, _) = nca(1, APP, vec![(0, meta)]);
    pfs0(&[
        (format!("{}.nca", hex(&pid)), prog),
        (format!("{}.cnmt.nca", hex(&[9u8; 16])), m),
    ])
}

fn rom() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("good.bin", vec![0x11; 0x9000]),
        ("bad.bin", vec![0xAB; 0x9000]),
        ("tail.bin", vec![0x22; 0x9000]),
        ("m.nro", vec![0x33; 0x100]),
    ]
}

/// (path, bytes, damaged flag) of the files of an image.
fn flags(img: &[u8]) -> Vec<(String, bool)> {
    let u64at = |o: usize| u64::from_le_bytes(img[o..o + 8].try_into().unwrap()) as usize;
    let (ioff, n) = (u64at(8), u64at(24));
    let pool = ioff + n * 24;
    let mut v = Vec::new();
    for k in 1..n {
        let e = ioff + k * 24;
        let no = u32::from_le_bytes(img[e + 16..e + 20].try_into().unwrap()) as usize;
        let nl = u16::from_le_bytes(img[e + 20..e + 22].try_into().unwrap()) as usize;
        v.push((
            String::from_utf8_lossy(&img[pool + no..pool + no + nl]).into_owned(),
            img[e + 23] != 0,
        ));
    }
    v.sort();
    v
}

#[test]
fn damaged_file_stops_the_run_unless_allowed() {
    let dir = tmp("dmg");
    let keys = write_keys(&dir);
    let pkg = dir.join("g.nsp");
    // 0x5000 into bad.bin: a block that only that file uses
    std::fs::write(&pkg, package(&rom(), Some((0xAB, 0x5000)), false)).unwrap();
    let r = nxb_extract::extract(&request(&keys, &pkg, &dir.join("strict")));
    match r {
        Err(ExtractError::Damaged(v)) => assert_eq!(v, vec!["/bad.bin".to_string()]),
        other => panic!("{other:?}"),
    }
    assert!(!dir.join("strict/RomFS.img").exists());
    let mut req = request(&keys, &pkg, &dir.join("lenient"));
    req.allow_damaged = true;
    nxb_extract::extract(&req).unwrap();
    let img = std::fs::read(dir.join("lenient/RomFS.img")).unwrap();
    let f = flags(&img);
    assert_eq!(
        f.iter()
            .filter(|x| x.1)
            .map(|x| x.0.as_str())
            .collect::<Vec<_>>(),
        ["bad.bin"]
    );
    let files = read_image(&img);
    let bad = &files.iter().find(|x| x.0 == "/bad.bin").unwrap().1;
    assert_eq!(bad.len(), 0x9000);
    assert_eq!(bad[0x5000], 0xAB ^ 0x55, "written as read");
    assert_eq!(
        files.iter().find(|x| x.0 == "/good.bin").unwrap().1,
        vec![0x11; 0x9000]
    );
    cleanup(&dir);
}

#[test]
fn manifest_lists_files_and_copies_nro_modules() {
    let dir = tmp("manifest");
    let keys = write_keys(&dir);
    let pkg = dir.join("g.nsp");
    std::fs::write(&pkg, package(&rom(), Some((0xAB, 0x100)), false)).unwrap();
    let mut req = request(&keys, &pkg, &dir.join("o"));
    req.manifest = true;
    // a damaged file does not stop the survey, and the run ends as damaged
    assert!(matches!(
        nxb_extract::extract(&req),
        Err(ExtractError::Damaged(v)) if v == ["/bad.bin"]
    ));
    let t = std::fs::read_to_string(dir.join("o/romfs_manifest.tsv")).unwrap();
    let want = format!(
        "# path\tbytes\tfirst 32 bytes (hex)\n/bad.bin\t36864\t\n/good.bin\t36864\t{}\n/m.nro\t256\t{}\n/tail.bin\t36864\t{}\n",
        "11".repeat(32),
        "33".repeat(32),
        "22".repeat(32)
    );
    // the flipped byte lies in the block that holds the first bytes of bad.bin (and the end of good.bin, whose head is fine)
    assert_eq!(t, want);
    assert_eq!(
        std::fs::read(dir.join("o/romfs_nro/m.nro")).unwrap(),
        vec![0x33; 0x100]
    );
    assert!(!dir.join("o/RomFS.img").exists());
    assert!(dir.join("o/ExeFS/main").is_file());
    cleanup(&dir);
}

#[test]
fn aoc_writes_the_data_nca_as_an_image() {
    let dir = tmp("aoc");
    let keys = write_keys(&dir);
    let files = vec![("dlc.bin", vec![9u8; 0x5000]), ("b.txt", b"hello".to_vec())];
    let (n, _) = nca(5, APP | 0x1001, vec![(0, romfs_section(&files))]);
    let pkg = dir.join("dlc.nsp");
    std::fs::write(&pkg, pfs0(&[(format!("{}.nca", hex(&[5u8; 16])), n)])).unwrap();
    let req = request(&keys, &pkg, &dir.join("o"));
    nxb_extract::aoc::aoc(&req, None).unwrap().closing.wait();
    let got = read_image(&std::fs::read(dir.join("o/RomFS.img")).unwrap());
    assert_eq!(got.len(), 2);
    assert_eq!(got[0], ("/b.txt".to_string(), b"hello".to_vec()));
    assert!(matches!(
        nxb_extract::aoc::aoc(&req, Some(5)),
        Err(ExtractError::Format(_))
    ));
    cleanup(&dir);
}

#[test]
fn sparse_program_is_a_named_unsupported_error() {
    let dir = tmp("sparse");
    let keys = write_keys(&dir);
    let pkg = dir.join("g.nsp");
    std::fs::write(&pkg, package(&rom(), None, true)).unwrap();
    let r = nxb_extract::extract(&request(&keys, &pkg, &dir.join("o")));
    assert!(matches!(r, Err(ExtractError::Unsupported(m)) if m.contains("sparse")));
    cleanup(&dir);
}

#[test]
fn unsupported_exits_with_4_and_damaged_with_3() {
    let dir = tmp("exitcodes");
    let keys = write_keys(&dir);
    let pkg = dir.join("g.nsp");
    let run = |out: &str| {
        nxb_extract::cli::main(&[
            "--keys".into(),
            keys.display().to_string(),
            "--base".into(),
            pkg.display().to_string(),
            "--out".into(),
            dir.join(out).display().to_string(),
        ])
    };
    std::fs::write(&pkg, package(&rom(), None, true)).unwrap();
    assert_eq!(run("a"), 4, "sparse program");
    std::fs::write(&pkg, package(&rom(), Some((0xAB, 0x5000)), false)).unwrap();
    assert_eq!(run("b"), 3, "damaged data");
    std::fs::write(&pkg, package(&rom(), None, false)).unwrap();
    assert_eq!(run("c"), 0);
    cleanup(&dir);
}
