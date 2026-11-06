//! `nxb build` as the app binary calls it: events on standard output in json mode, exit codes, argument errors.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::process::Command;

fn build(args: &[&str]) -> (Option<i32>, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_nxb"))
        .arg("build")
        .args(args)
        .env_remove("NXB_ENGINE")
        .output()
        .expect("run");
    (
        o.status.code(),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("nxb-cli-build-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Every standard output line of a json run is one JSON object.
fn assert_ndjson(out: &str) {
    assert!(!out.trim().is_empty());
    for l in out.lines() {
        let v = nxb_pyjson::parse(l).unwrap_or_else(|_| panic!("not JSON: {l}"));
        assert!(v.get("ev").is_some(), "{l}");
    }
}

#[test]
fn json_mode_standard_output_is_events_only() {
    let d = tmp("json");
    let p = d.to_str().unwrap();
    for args in [
        vec![p, "--progress", "json", "--bogus"],
        vec![p, "--progress", "json"],
        vec![p, "--progress=json", "--platform", "nope"],
    ] {
        let (_, out, _) = build(&args);
        assert_ndjson(&out);
    }
}

fn last_msg(out: &str) -> String {
    let l = out.lines().last().unwrap_or_default();
    nxb_pyjson::parse(l)
        .ok()
        .and_then(|v| v.get("msg").and_then(|m| m.as_str().map(str::to_string)))
        .unwrap_or_default()
}

#[test]
fn a_folder_without_a_package_is_a_clear_error() {
    let d = tmp("empty");
    std::fs::write(d.join("notes.txt"), b"x").unwrap();
    let (code, out, _) = build(&[d.to_str().unwrap(), "--progress", "json"]);
    assert_eq!(code, Some(1));
    assert!(last_msg(&out).contains("no game package"), "{out}");
    let (code, out, _) = build(&["/nonexistent-nxb-folder", "--progress", "json"]);
    assert_eq!(code, Some(1));
    assert!(last_msg(&out).contains("is not a folder"), "{out}");
}

#[test]
fn exit_codes_by_class() {
    let d = tmp("codes");
    let p = d.to_str().unwrap();
    // bad arguments 2, not implemented 4
    assert_eq!(build(&[p, "--platform", "tv"]).0, Some(2));
    assert_eq!(build(&[p, "--threads"]).0, Some(2));
    assert_eq!(build(&[p, "--platform", "mac"]).0, Some(4));
    assert_eq!(build(&[p, "--program", "3"]).0, Some(4));
    assert_eq!(
        build(&[p, "--platform", "mac-app", "--romfs", "separate"]).0,
        Some(2)
    );
    assert_eq!(
        build(&[p, "--platform", "ios", "--seen-shaders"]).0,
        Some(2)
    );
    // a package, a profile, no keys: 2 without -k, 6 with a missing keys file
    let t = "0100000000000000";
    // an empty PFS0 (no files): a package for the folder scan, no content
    let mut pfs0 = vec![0u8; 0x200];
    pfs0[..4].copy_from_slice(b"PFS0");
    std::fs::write(d.join(format!("Game [{t}][v0].nsp")), &pfs0).unwrap();
    let eng = d.join("engine");
    std::fs::create_dir_all(eng.join("dm/games").join(t)).unwrap();
    std::fs::write(eng.join("dm/games").join(t).join("game.tsv"), "name\tG\n").unwrap();
    let lib = d.join("lib");
    let work = d.join("work");
    let base = [
        p,
        "--platform",
        "mac-app",
        "--engine",
        eng.to_str().unwrap(),
        "--shaders",
        lib.to_str().unwrap(),
        "--work-dir",
        work.to_str().unwrap(),
        "--progress",
        "json",
    ];
    assert_eq!(build(&base).0, Some(2));
    let mut with_keys = base.to_vec();
    with_keys.extend(["-k", "/nonexistent-prod.keys"]);
    assert_eq!(build(&with_keys).0, Some(6));
    // a game without a profile is surveyed first, from the package: it needs the keys like any package
    std::fs::write(d.join("Other [0100000000001000][v0].nsp"), &pfs0).unwrap();
    std::fs::remove_file(d.join(format!("Game [{t}][v0].nsp"))).unwrap();
    assert_eq!(build(&base).0, Some(2));
    assert_eq!(build(&with_keys).0, Some(6));
}

fn npdm(program: u64) -> Vec<u8> {
    let mut d = vec![0u8; 0x200];
    d[..4].copy_from_slice(b"META");
    d[0x70..0x74].copy_from_slice(&0x100u32.to_le_bytes());
    d[0x74..0x78].copy_from_slice(&0x40u32.to_le_bytes());
    d[0x100..0x104].copy_from_slice(b"ACI0");
    d[0x110..0x118].copy_from_slice(&program.to_le_bytes());
    d
}

/// A dump folder is found by layout (folder, `--dump`), its versions and DLC are listed, and its title picks the profile.
#[test]
fn dump_folder_is_an_input() {
    let app = 0x0100_0000_0000_2000u64;
    let d = tmp("dump");
    let put = |rel: &str, data: &[u8]| {
        let p = d.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, data).unwrap();
    };
    put("base/exefs/main", &[1; 100]);
    put("base/exefs/main.npdm", &npdm(app));
    put("base/romfs/a.txt", b"a");
    put("updates/65536/exefs/main", &[2; 100]);
    put("updates/65536/exefs/main.npdm", &npdm(app));
    put("updates/65536/romfs/a.txt", b"b");
    put(&format!("dlc/{:016x}/1/romfs/x", app + 0x1001), b"x");
    let eng = tmp("dump-engine");
    let prof = eng.join("dm/games/0100000000002000");
    std::fs::create_dir_all(&prof).unwrap();
    std::fs::write(prof.join("game.tsv"), "name\tG\n").unwrap();
    let lib = tmp("dump-lib");
    let run = |extra: &[&str]| {
        let mut a = vec![
            "--dry-run",
            "--platform",
            "mac-app",
            "--engine",
            eng.to_str().unwrap(),
            "--shaders",
            lib.to_str().unwrap(),
        ];
        a.extend(extra);
        build(&a)
    };
    for extra in [
        vec![d.to_str().unwrap()],
        vec!["--dump", d.to_str().unwrap()],
    ] {
        let (code, out, err) = run(&extra);
        assert_eq!(code, Some(0), "{out}{err}");
        assert!(out.contains("input Dump"), "{out}");
        assert!(
            out.contains("game 0100000000002000 package v65536"),
            "{out}"
        );
        assert!(out.contains("update v65536"), "{out}");
        assert!(
            out.contains("dlc 0100000000003001") || out.contains("dlc 0100000000003"),
            "{out}"
        );
    }
    // a run unpacks the packed version and the base game into the work folder (the rewrite then stops: no real profile)
    let work = d.join("work");
    let (_, out, _) = build(&[
        d.to_str().unwrap(),
        "--platform",
        "mac-app",
        "--engine",
        eng.to_str().unwrap(),
        "--shaders",
        lib.to_str().unwrap(),
        "--work-dir",
        work.to_str().unwrap(),
        "--progress",
        "json",
    ]);
    assert!(work.join("extract/v65536/RomFS.img").is_file(), "{out}");
    assert!(work.join("extract/v0/RomFS.img").is_file(), "{out}");
    // the wrong game
    let (code, out, _) = run(&[d.to_str().unwrap(), "--title-id", "0100000000009000"]);
    assert_eq!(code, Some(1));
    assert!(out.contains("no game 0100000000009000"), "{out}");
    // a file that is no NUP
    let f = d.join("base/exefs/main");
    let (code, out, _) = run(&[f.to_str().unwrap()]);
    assert_eq!(code, Some(1));
    assert!(out.contains("not a folder or a NUP file"), "{out}");
}
