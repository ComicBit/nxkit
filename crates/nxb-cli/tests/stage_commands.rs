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
