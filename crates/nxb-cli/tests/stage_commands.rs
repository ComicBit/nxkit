//! Each `nxb` stage command keeps the exit codes of the reference command line on small inputs.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn nxb() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_nxb"))
}

fn run(exe: &Path, args: &[&str]) -> (Option<i32>, String, String) {
    let o = Command::new(exe)
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

/// `nxb <sub> <args>`.
fn stage(sub: &str, args: &[&str]) -> (Option<i32>, String, String) {
    let mut a = vec![sub];
    a.extend(args);
    run(&nxb(), &a)
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("nxb-cli-stage-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn usage_and_failures_match() {
    let t = tmp("fail");
    let t = t.to_str().unwrap();
    let nope = format!("{t}/missing.tsv");
    assert_eq!(stage("split", &[]).0, Some(2));
    let r = stage(
        "split",
        &[
            "--game",
            &nope,
            "--pkg",
            t,
            "--map",
            t,
            "--facts",
            t,
            "--pairs",
            t,
            "--pso-pairs",
            t,
            t,
        ],
    );
    assert_ne!(r.0, Some(0));
    assert_eq!(stage("ios", &[]).0, Some(2));
    assert_eq!(stage("archive", &[]).0, Some(2));
    assert_eq!(stage("pso", &[]).0, Some(1));
    assert_eq!(stage("ipa", &[]).0, Some(2));
    assert_eq!(stage("app", &[]).0, Some(2));
    assert_ne!(stage("sign", &[]).0, Some(0));
    assert_ne!(stage("sign", &["cdhash", &nope]).0, Some(0));
    assert_eq!(stage("extract", &[]).0, Some(2));
    let _ = std::fs::remove_dir_all(t);
}

#[test]
fn ipa_writes_a_zip() {
    let t = tmp("ipa");
    std::fs::write(t.join("a.txt"), b"hello zip").unwrap();
    let src = format!("Payload/a.txt={}", t.join("a.txt").display());
    let o1 = t.join("1.ipa");
    let a = run(&nxb(), &["ipa", o1.to_str().unwrap(), &src]);
    assert_eq!((a.0, a.1.as_str()), (Some(0), "packed: 2 entries\n"));
    let bytes = std::fs::read(&o1).unwrap();
    assert_eq!(&bytes[..4], b"PK\x03\x04");
    let _ = std::fs::remove_dir_all(&t);
}

#[test]
fn airc_is_a_command() {
    assert_eq!(stage("airc", &[]).0, Some(2));
    let t = tmp("airc");
    let src = t.join("k.metal");
    std::fs::write(
        &src,
        "kernel void k(device float* a [[buffer(0)]], uint i [[thread_position_in_grid]]) { a[i] = 1.0; }\n",
    )
    .unwrap();
    let ll = t.join("k.ll");
    let r = stage(
        "airc",
        &["emit", src.to_str().unwrap(), "-o", ll.to_str().unwrap()],
    );
    assert_eq!(r.0, Some(0), "{}", r.2);
    assert!(std::fs::read_to_string(&ll).unwrap().contains("define"));
    let _ = std::fs::remove_dir_all(&t);
}

/// The real Mac/iOS shader stage, with a synthetic Maxwell compute shader that only exits.
#[cfg(target_os = "macos")]
#[test]
fn full_shader_build_stamps_both_targets_and_retires_old_ios_outputs() {
    if !Command::new("/usr/bin/xcrun")
        .args(["--sdk", "iphoneos", "metal", "--version"])
        .output()
        .is_ok_and(|o| o.status.success())
    {
        return;
    }
    let d = tmp("ios-library-stamp");
    let ios = d.join("ios/b-test");
    std::fs::create_dir_all(&ios).unwrap();
    let key = "0123456789abcdef";
    let mut record = vec![0; 0x38];
    record[..8].copy_from_slice(b"BTCDBTCD");
    for word in [0_u64, 0xe300_0000_0007_000f, 0, 0] {
        record.extend(word.to_le_bytes());
    }
    record.extend([0x34, 0x12, 0x76, 0x98]);
    record.resize(record.len() + 0x800, 0);
    std::fs::write(d.join("cs_test.bin"), record).unwrap();
    std::fs::write(
        d.join("shader_map.tsv"),
        format!("cs_test.bin\t0\t0\t{key}\t0\n"),
    )
    .unwrap();
    std::fs::write(
        d.join("shader_facts.tsv"),
        format!("{key}\t{{\"local\":[1,1,1]}}\n"),
    )
    .unwrap();
    let obsolete = ios.join("fedcba9876543210_b.metallib");
    std::fs::write(&obsolete, "old library").unwrap();
    let run = |keys: &[&str]| {
        let result = Command::new(nxb())
            .current_dir(&d)
            .args(["shaders", "build", ".", "cache", "--profile", "."])
            .args([
                "--game",
                "test-game",
                "--chain",
                "test",
                "--inputs",
                "test-inputs",
            ])
            .args(["--store", "store", "--work", "work", "--scratch", "scratch"])
            .args(["--ios", "ios", "--own-front", "off", "--jobs", "1"])
            .args(keys)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    };
    run(&[]);
    for lib in [&ios, &d.join("cache/b-test")] {
        let binary = std::fs::read(lib.join(format!("{key}_b.metallib"))).unwrap();
        assert!(binary.starts_with(b"MTLB"));
    }
    let stamp = std::fs::read(d.join("cache/b-test/inputs.tsv")).unwrap();
    assert_eq!(std::fs::read(ios.join("inputs.tsv")).unwrap(), stamp);
    assert!(
        !obsolete.exists(),
        "a stamp must never certify stale iOS libraries"
    );
    run(&[key]);
    assert!(
        !ios.join("inputs.tsv").exists(),
        "a subset cannot certify a full iOS library"
    );
    let _ = std::fs::remove_dir_all(d);
}
