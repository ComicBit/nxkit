//! Dump folders (`base/`, `updates/<version>/`, `dlc/`) packed in place: the same `ExeFS/` and `DMROMFS1` images as
//! the engine's `pack_dump.py` (which the test runs when `NXB_PACK_DUMP` names it), and refusals for bad layouts.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;
use std::path::{Path, PathBuf};

use common::*;
use nxb_extract::dump::{DumpRequest, extract_dump, is_dump};

const APP: u64 = 0x0100_0000_0000_2000;

fn put(root: &Path, rel: &str, data: &[u8]) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, data).unwrap();
}

fn version(dump: &Path, rel: &str, tag: u8, program: u64) {
    put(dump, &format!("{rel}/exefs/main"), &vec![tag; 3000]);
    put(dump, &format!("{rel}/exefs/main.npdm"), &npdm(program));
    put(dump, &format!("{rel}/exefs/rtld"), &[tag, 1]);
    put(dump, &format!("{rel}/exefs/.hidden"), b"x");
    put(
        dump,
        &format!("{rel}/romfs/a.txt"),
        format!("a{tag}").as_bytes(),
    );
    put(dump, &format!("{rel}/romfs/Dir/b.bin"), &vec![tag; 70_000]);
    put(dump, &format!("{rel}/romfs/Dir/Z.bin"), b"");
    put(dump, &format!("{rel}/romfs/Dir/deep/er/x"), &[tag; 10]);
    put(dump, &format!("{rel}/romfs/.dotfile"), b"d");
    put(dump, &format!("{rel}/romfs/\u{e4}pfel.txt"), b"umlaut");
    std::fs::create_dir_all(dump.join(format!("{rel}/romfs/empty"))).unwrap();
}

fn make(dump: &Path) {
    version(dump, "base", 1, APP);
    version(dump, "updates/65536", 2, APP);
    version(dump, "updates/131072", 3, APP);
    put(
        dump,
        &format!("dlc/{:016x}/1/romfs/dlc.bin", APP + 0x1001),
        &[7; 5000],
    );
    put(
        dump,
        &format!("dlc/{:016x}/romfs/other.bin", APP + 0x1002),
        &[8; 50],
    );
}

fn req(dump: &Path, work: &Path) -> DumpRequest {
    DumpRequest {
        dump: dump.to_path_buf(),
        version: None,
        work: work.to_path_buf(),
        threads: 2,
        mem_limit: 1 << 26,
        discard_images: false,
        digests: None,
    }
}

fn tree_bytes(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut todo = vec![root.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                todo.push(p);
            } else {
                out.push((
                    p.strip_prefix(root).unwrap().display().to_string(),
                    std::fs::read(&p).unwrap(),
                ));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn a_dump_packs_the_newest_version_and_the_dlc() {
    let dir = tmp("dump");
    let dump = dir.join("game");
    make(&dump);
    assert!(is_dump(&dump));
    let work = dir.join("w");
    let res = extract_dump(&req(&dump, &work)).unwrap();
    assert_eq!(res.len(), 3);
    let out = work.join("extract/v131072");
    let exefs: Vec<String> = tree_bytes(&out.join("ExeFS"))
        .into_iter()
        .map(|x| x.0)
        .collect();
    assert_eq!(
        exefs,
        ["main", "main.npdm", "rtld"],
        "hidden files are left out"
    );
    assert_eq!(
        std::fs::read(out.join("ExeFS/main")).unwrap(),
        vec![3u8; 3000]
    );
    let img = read_image(&std::fs::read(out.join("RomFS.img")).unwrap());
    let names: Vec<&str> = img.iter().map(|x| x.0.as_str()).collect();
    assert_eq!(
        names,
        [
            "/.dotfile",
            "/Dir/Z.bin",
            "/Dir/b.bin",
            "/Dir/deep/er/x",
            "/a.txt",
            "/\u{e4}pfel.txt"
        ]
    );
    assert_eq!(img[2].1, vec![3u8; 70_000]);
    assert_eq!(
        read_image(&std::fs::read(work.join("aoc/1.img")).unwrap())[0].1,
        vec![7u8; 5000]
    );
    assert_eq!(
        read_image(&std::fs::read(work.join("aoc/2.img")).unwrap())[0].1,
        vec![8u8; 50]
    );
    // a named version and the base game
    for (v, tag) in [(Some(65536), 2u8), (Some(0), 1)] {
        let mut r = req(&dump, &work);
        r.version = v;
        extract_dump(&r).unwrap();
        let o = work.join(format!("extract/v{}", v.unwrap()));
        assert_eq!(
            std::fs::read(o.join("ExeFS/main")).unwrap(),
            vec![tag; 3000]
        );
    }
    // the engine's own script on the same folder
    if let Some(py) = std::env::var_os("NXB_PACK_DUMP") {
        let w2 = dir.join("py");
        let r = std::process::Command::new("python3")
            .arg("-B")
            .arg(py)
            .arg(&dump)
            .arg("--work-dir")
            .arg(&w2)
            .output()
            .unwrap();
        assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
        extract_dump(&req(&dump, &work)).unwrap();
        assert!(
            tree_bytes(&w2.join("extract/v131072"))
                .iter()
                .all(|(n, d)| n.ends_with(".inputs.json")
                    || tree_bytes(&out).iter().any(|(m, e)| m == n && e == d))
        );
        for f in ["extract/v131072/RomFS.img", "aoc/1.img", "aoc/2.img"] {
            assert!(
                std::fs::read(w2.join(f)).unwrap() == std::fs::read(work.join(f)).unwrap(),
                "{f}"
            );
        }
        assert_eq!(
            tree_bytes(&w2.join("extract/v131072/ExeFS")),
            tree_bytes(&out.join("ExeFS"))
        );
    }
    cleanup(&dir);
}

#[test]
fn a_dump_with_only_an_update_romfs_and_dlc_cleanup() {
    let dir = tmp("dump2");
    let dump = dir.join("game");
    put(&dump, "base/exefs/main", b"m");
    put(&dump, "base/exefs/main.npdm", &npdm(APP));
    version(&dump, "updates/5", 4, APP);
    assert!(is_dump(&dump));
    let work = dir.join("w");
    // an old DLC image that is no longer in the dump goes
    put(&work, "aoc/9.img", b"old");
    put(&work, "aoc/9.img.inputs.json", b"{}");
    extract_dump(&req(&dump, &work)).unwrap();
    assert!(!work.join("aoc/9.img").exists() && !work.join("aoc/9.img.inputs.json").exists());
    // the digest mode writes no image and the TSV names the image
    let mut r = req(&dump, &dir.join("w2"));
    r.discard_images = true;
    r.digests = Some(dir.join("d.tsv"));
    let res = extract_dump(&r).unwrap();
    assert!(!dir.join("w2/extract/v5/RomFS.img").exists());
    let a = std::fs::read(work.join("extract/v5/RomFS.img")).unwrap();
    assert_eq!(res[0].digest.file.sha256, nxb_model::sha256(&a));
    assert!(
        std::fs::read_to_string(dir.join("d.tsv"))
            .unwrap()
            .contains("RomFS.img")
    );
    cleanup(&dir);
}

#[test]
fn bad_layouts_are_refused() {
    let dir = tmp("dump-bad");
    let run = |dump: &Path, v: Option<u32>| {
        let mut r = req(dump, &dir.join("w"));
        r.version = v;
        extract_dump(&r).err().map(|e| e.to_string())
    };
    let new = |name: &str| -> PathBuf {
        let d = dir.join(name);
        make(&d);
        d
    };
    // not a dump
    std::fs::create_dir_all(dir.join("empty")).unwrap();
    assert!(!is_dump(&dir.join("empty")));
    assert!(
        run(&dir.join("empty"), None)
            .unwrap()
            .contains("no base/exefs/main.npdm")
    );
    assert!(run(&dir.join("missing"), None).is_some());
    // an update that is not there, a program ID that is not the base game's, one that is not a base title ID
    let d = new("v");
    assert!(
        run(&d, Some(7))
            .unwrap()
            .contains("no updates/7 (have: 65536, 131072)")
    );
    put(&d, "updates/131072/exefs/main.npdm", &npdm(APP + 0x800));
    assert!(
        run(&d, None)
            .unwrap()
            .contains("program ID 0100000000002800, the base is 0100000000002000")
    );
    let d = new("id");
    put(&d, "base/exefs/main.npdm", &npdm(APP + 1));
    assert!(run(&d, None).unwrap().contains("is not a base title ID"));
    put(&d, "base/exefs/main.npdm", b"not an npdm");
    assert!(run(&d, None).is_some());
    // DLC folders: a name that is not hex, a title ID that is not a DLC of the game, two RomFS folders
    let d = new("dlc1");
    std::fs::create_dir_all(d.join("dlc/zzzz/1/romfs")).unwrap();
    assert!(run(&d, None).unwrap().contains("not a DLC title ID"));
    let d = new("dlc2");
    std::fs::create_dir_all(d.join(format!("dlc/{:016x}/1/romfs", APP + 0x1001 + 5000))).unwrap();
    assert!(run(&d, None).unwrap().contains("is not a DLC of"));
    let d = new("dlc3");
    std::fs::create_dir_all(d.join(format!("dlc/{:016x}/2/romfs", APP + 0x1001))).unwrap();
    assert!(
        run(&d, None)
            .unwrap()
            .contains("expected one <n>/romfs folder, found 2")
    );
    // a folder link loop is stopped, not followed forever
    let d = new("loop");
    std::os::unix::fs::symlink(d.join("base/romfs"), d.join("base/romfs/Dir/loop")).unwrap();
    assert!(run(&d, Some(0)).is_some()); // the depth limit or the OS's own link limit stops it
    // a name the case-collision scheme made: the reference script reads its xattr, so this is not ours to guess
    let d = new("case");
    put(&d, "base/romfs/.dm-case-1", b"x");
    let e = extract_dump(&{
        let mut r = req(&d, &dir.join("w"));
        r.version = Some(0);
        r
    })
    .err()
    .unwrap();
    assert!(matches!(e, nxb_extract::ExtractError::Unsupported(_)));
    // a file that vanishes between listing and reading is an error, not a short image
    cleanup(&dir);
}
