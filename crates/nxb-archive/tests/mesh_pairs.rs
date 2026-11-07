//! A mesh shader's pairs are left out of the archive plan: the host builds mesh pipelines at the draw.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use nxb_archive::plan::plan;
use std::fs;
use std::path::PathBuf;

fn dir() -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("mesh_pairs");
    fs::create_dir_all(&d).unwrap();
    d
}

fn reflection(d: &std::path::Path, key: &str, extra: &str) {
    let text = format!("{{\"chain\": \"abc\", \"entry\": \"main0\"{extra}}}");
    fs::write(d.join(format!("{key}_b.json")), text).unwrap();
}

#[test]
fn a_mesh_pair_is_counted_and_not_planned() {
    let d = dir();
    reflection(&d, "ms", ", \"mesh_stage\": {\"kind\": \"ms\"}");
    reflection(&d, "vs", "");
    reflection(&d, "ps", "");
    let pairs = [
        ("ms".to_string(), "ps".to_string()),
        ("vs".to_string(), "ps".to_string()),
    ];
    let p = plan(&d, "abc", &pairs, false);
    assert_eq!(p.left[1], 1);
    // The vertex pair goes on to the library check, which fails here: there is no metallib.
    assert_eq!(p.left[3], 1);
    assert!(p.bases.is_empty());
}
