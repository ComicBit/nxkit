//! Tests of the pixel entries: the side units, the derived entries and the pixel facts.

use super::tests::{B, fragment};
use super::{Side, Translated};
use nxb_pyjson::Value;

fn reflection(t: &Translated) -> Value {
    t.libraries[0]
        .reflection
        .as_ref()
        .expect("reflection")
        .to_value()
}

fn names(v: &Value, key: &str) -> Vec<String> {
    let e = v.get(key).and_then(|a| a.get("entries")).expect(key);
    match e {
        Value::List(l) => l
            .iter()
            .map(|n| n.as_str().expect("name").to_string())
            .collect(),
        _ => panic!("entries are a list"),
    }
}

/// `fragment()` with the colour 0 pad as its only colour 0.
fn side_sample() -> (B, Translated) {
    let b = fragment();
    let t = b.translate();
    (b, t)
}

#[test]
fn side_entries_wrap_the_body() {
    let (_, t) = side_sample();
    let msl = &t.libraries[0].msl;
    assert!(msl.contains("static main0_out dm_body_main0("));
    assert!(msl.contains("fragment main0_out main0("));
    assert!(msl.contains("fragment main0_out main0_a("));
    assert!(msl.contains("[[buffer(30)]]"));
    assert!(
        msl.contains("if (!dm_alpha_pass(float(dm_r.c0_pad.w), dm_alpha)) discard_fragment();")
    );
    assert!(msl.contains("fragment main0_out main0_l("));
    assert!(msl.contains("constant bool dm_lfb0 [[function_constant(200)]];"));
    assert!(msl.contains("constant bool dm_lfb1 [[function_constant(201)]];"));
    assert!(msl.contains("float4 dm_fb1 [[color(1), function_constant(dm_lfb1)]]"));
    assert!(msl.contains(
        "if (dm_lfb1) dm_r.c1 = float4(dm_logic_norm(float4(dm_r.c1), dm_fb1, dm_lop, 1u));"
    ));
    assert!(msl.contains("constant dm_logic_state& dm_lop [[buffer(29)]]"));
}

#[test]
fn side_names_and_fields() {
    let (_, t) = side_sample();
    assert_eq!(
        t.side,
        Some(Side {
            alpha: vec!["main0_a".into()],
            logic: vec!["main0_l".into()],
        })
    );
    let r = reflection(&t);
    assert_eq!(names(&r, "alpha"), ["main0_a"]);
    assert_eq!(names(&r, "logic"), ["main0_l"]);
}

#[test]
fn a_shader_without_colours_has_no_side_entries() {
    let mut b = B::new(crate::attr::Stage::Fragment, "ps");
    b.s.props.definitions.imap_types = vec![[2, 2, 2, 2]];
    let t = b.translate();
    let msl = &t.libraries[0].msl;
    assert!(!msl.contains("dm_body_"));
    assert!(t.side.is_none());
    let r = reflection(&t);
    assert!(r.get("alpha").is_none());
    assert!(r.get("logic").is_none());
}

#[test]
fn the_early_attribute_goes_to_every_entry() {
    let mut b = fragment();
    b.s.props.definitions.early_z_force = true;
    let msl = b.msl();
    let n = msl.matches("[[early_fragment_tests]] fragment").count();
    assert_eq!(n, 3, "{msl}");
}

#[test]
fn the_side_entries_are_entry_functions() {
    let (_, t) = side_sample();
    let s = t.side.expect("side");
    let all: Vec<&str> = s.alpha.iter().chain(&s.logic).map(String::as_str).collect();
    for n in all {
        assert!(
            t.libraries[0]
                .msl
                .contains(&format!("fragment main0_out {n}("))
        );
    }
}

#[test]
#[ignore]
fn metal_builds_the_side_entries() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scratch");
    std::fs::create_dir_all(&dir).expect("scratch");
    let (_, t) = side_sample();
    let src = dir.join("side_ps.metal");
    let air = dir.join("side_ps.air");
    let lib = dir.join("side_ps.metallib");
    std::fs::write(&src, &t.libraries[0].msl).expect("write");
    let run = |args: &[&std::ffi::OsStr]| {
        let o = std::process::Command::new("xcrun")
            .args(["-sdk", "macosx"])
            .args(args)
            .output()
            .expect("run");
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    };
    use std::ffi::OsStr;
    run(&[
        OsStr::new("metal"),
        OsStr::new("-c"),
        OsStr::new("-O1"),
        OsStr::new("-std=metal3.0"),
        src.as_os_str(),
        OsStr::new("-o"),
        air.as_os_str(),
    ]);
    run(&[
        OsStr::new("metallib"),
        air.as_os_str(),
        OsStr::new("-o"),
        lib.as_os_str(),
    ]);
    // `nxb-shader` (the cut) is not a dependency of this crate: the library keeps every entry name for it.
    let bytes = std::fs::read(&lib).expect("lib");
    let side = t.side.expect("side");
    for n in side.alpha.iter().chain(&side.logic) {
        let at = |w: &[u8]| bytes.windows(w.len()).any(|x| x == w);
        assert!(at(n.as_bytes()), "{n} is a function of the library");
    }
}
