//! Hand-built geometry programs through the MSL back end.
#![cfg(test)]

use super::{Context, Failure, Translated, translate};
use crate::job::{Job, parse_jobs};
use crate::translation::emit_test_util::{PRED7, RET};

/// `OUT.EMIT_THEN_CUT RZ, RZ, 0` and friends: the type is the two bits at 39.
fn out_i(ty: u64) -> u64 {
    0x3DB8u64 << 50 | 0xff | 0xff << 8 | PRED7 | ty << 39
}

fn job(extra: &str) -> Job {
    let json = format!(r#"[{{"key": "gs0", "stage": "gs"{extra}}}]"#);
    parse_jobs(&json).expect("job").remove(0)
}

/// A Maxwell geometry program: the 0x50 byte header (one thread per primitive, triangle strip, 6 vertices) and the ops.
fn code(word0: u32, ops: &[u64]) -> Vec<u8> {
    let mut h = [0u32; 20];
    h[0] = 4 << 10 | word0;
    h[2] = 1 << 24;
    h[3] = 7 << 24;
    h[4] = 6;
    let mut words: Vec<u64> = vec![0];
    for (i, w) in ops.iter().chain([RET].iter()).enumerate() {
        words.push(*w);
        if i % 3 == 2 {
            words.push(0);
        }
    }
    let mut c: Vec<u8> = h.iter().flat_map(|w| w.to_le_bytes()).collect();
    c.extend(words.iter().flat_map(|w| w.to_le_bytes()));
    c
}

fn run(j: &Job, c: &[u8]) -> Result<Translated, Failure> {
    translate(j, c, &Context::default())
}

fn extra<'a>(r: &'a super::Reflection, k: &str) -> Option<&'a nxb_pyjson::Value> {
    r.extra.iter().find(|(n, _)| n == k).map(|(_, v)| v)
}

fn emitting() -> Translated {
    run(&job(""), &code(0, &[out_i(3)])).expect("translate")
}

#[test]
fn a_geometry_shader_has_the_kernel_libraries() {
    let t = emitting();
    let names: Vec<&str> = t.libraries.iter().map(|l| l.suffix.as_str()).collect();
    assert_eq!(
        names,
        ["_b", "_bg0", "_bg1", "_bg2", "_bg3", "_bg4", "_bgv"]
    );
    for l in &t.libraries {
        assert!(l.reflection.is_some(), "{}", l.suffix);
    }
}

#[test]
fn a_kernel_is_a_compute_entry_with_the_helper_buffers() {
    let t = emitting();
    let l = &t.libraries[4];
    assert_eq!(l.suffix, "_bg3");
    assert!(l.msl.contains("kernel void main0"), "{}", l.msl);
    assert!(
        l.msl
            .contains("device geometry_vb_output_block* geometry_vb_output")
    );
    assert!(l.msl.contains("geometry_vb_output"));
    assert!(l.msl.contains("geometry_ib_output"));
    let r = l.reflection.as_ref().expect("reflection");
    assert_eq!(r.key, "gs0_bg3");
    assert_eq!(r.threadgroup_size, Some([32, 1, 1]));
    assert!(r.drv_unfillable.is_empty());
    let g = extra(r, "geometry").expect("geometry");
    assert_eq!(g.get("vertices").and_then(|v| v.as_i64()), Some(6));
    assert_eq!(g.get("output").and_then(|v| v.as_i64()), Some(7));
    assert_eq!(g.get("index_stride").and_then(|v| v.as_i64()), Some(12));
    assert!(g.get("stride").and_then(|v| v.as_i64()).unwrap_or(0) > 0);
    let slots: Vec<(&str, Option<i64>)> = r
        .bindings
        .iter()
        .filter(|b| b.kind.as_str() == "geometry")
        .map(|b| (b.msl_name.as_str(), b.reg))
        .collect();
    assert!(slots.contains(&("geometry_vb_output", Some(1))));
    assert!(slots.contains(&("geometry_ib_output", Some(2))));
}

#[test]
fn the_base_library_is_the_first_kernel() {
    let t = emitting();
    assert_eq!(t.libraries[0].msl, t.libraries[1].msl);
    let r = t.libraries[0].reflection.as_ref().expect("reflection");
    assert_eq!(r.key, "gs0_b");
}

#[test]
fn the_reader_draws_the_kernel_output() {
    let t = emitting();
    let l = t.libraries.last().expect("reader");
    assert_eq!(l.suffix, "_bgv");
    assert!(l.msl.contains("vertex "), "{}", l.msl);
    assert!(l.msl.contains("main0_z"), "{}", l.msl);
    assert!(l.msl.contains("vb_input"), "{}", l.msl);
    let r = l.reflection.as_ref().expect("reflection");
    assert_eq!(r.key, "gs0_bgv");
    assert_eq!(r.stage, "vs");
    assert_eq!(r.msl_entry_kind, "vertex");
    assert!(r.notes.is_empty());
    assert!(r.vertex_inputs.is_empty());
    assert!(r.entries.iter().any(|(k, _)| k == "entry_z"));
    assert!(extra(r, "geometry_reader").is_some());
    assert!(
        r.bindings
            .iter()
            .any(|b| b.msl_name == "vb_input" && b.reg == Some(1))
    );
}

#[test]
fn only_the_main_entry_converts_depth() {
    let t = emitting();
    let m = &t.libraries.last().expect("reader").msl;
    let (main, z) = m.split_once("main0_z").expect("two entries");
    assert!(main.contains("fma(out.gl_Position.z"), "{m}");
    assert!(!z.contains("fma(out.gl_Position.z"), "{m}");
}

#[test]
fn a_bad_header_fails_the_translation() {
    let mut c = code(0, &[out_i(3)]);
    c[12..16].copy_from_slice(&(2u32 << 24).to_le_bytes());
    let e = run(&job(""), &c).expect_err("topology");
    assert_eq!(e.reason, "translation failed");
    assert!(e.detail.contains("output declaration"), "{}", e.detail);
}

#[test]
fn transform_feedback_is_not_ported() {
    let e = run(
        &job(r#", "tf": [null, null, null, null]"#),
        &code(0, &[out_i(3)]),
    )
    .expect_err("tf");
    assert_eq!(e.reason, "not ported");
    assert_eq!(
        e.detail,
        "unported: transform feedback of a geometry shader"
    );
}

#[test]
fn tessellation_is_not_ported() {
    for stage in ["hs", "ds"] {
        let json = format!(r#"[{{"key": "t", "stage": "{stage}"}}]"#);
        let j = parse_jobs(&json).expect("job").remove(0);
        let e = run(&j, &[0; 64]).expect_err("tessellation");
        assert_eq!(e.reason, "not ported");
        assert!(e.detail.contains("tessellation"), "{}", e.detail);
    }
}

/// The kernels and the reader compile with Apple's compiler. Run by hand: `cargo test -p nxb-xlate -- --ignored metal`.
#[test]
#[ignore]
fn metal_compiles_the_geometry_libraries() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scratch");
    std::fs::create_dir_all(&dir).expect("scratch");
    for l in &emitting().libraries {
        let src = dir.join(format!("geom{}.metal", l.suffix));
        let air = dir.join(format!("geom{}.air", l.suffix));
        std::fs::write(&src, &l.msl).expect("write");
        let o = std::process::Command::new("xcrun")
            .args([
                "-sdk",
                "macosx",
                "metal",
                "-c",
                "-O1",
                "-fpreserve-invariance",
                "-std=metal3.0",
            ])
            .arg(&src)
            .arg("-o")
            .arg(&air)
            .output()
            .expect("run metal");
        assert!(
            o.status.success(),
            "{}: {}\n{}",
            l.suffix,
            String::from_utf8_lossy(&o.stderr),
            l.msl
        );
    }
}

#[test]
fn transform_feedback_of_a_vertex_shader_is_not_ported() {
    let j = parse_jobs(r#"[{"key": "v", "stage": "vs", "tf": [null, null, null, null]}]"#)
        .expect("job")
        .remove(0);
    let mut c: Vec<u8> = [1u32 << 10; 1]
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();
    c.resize(0x50, 0);
    c.extend([0u64, RET, 0, 0].iter().flat_map(|w| w.to_le_bytes()));
    let e = run(&j, &c).expect_err("tf");
    assert_eq!(e.reason, "not ported");
    assert_eq!(e.detail, "unported: transform feedback of a vertex shader");
}
