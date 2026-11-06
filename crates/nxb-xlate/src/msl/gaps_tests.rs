//! Tests of the gap closing features: each builds a small structured program and checks the MSL and the reflection.
//! The ignored test compiles the samples with Apple's compiler: `cargo test -p nxb-xlate -- --ignored gaps_metal`.

use super::emit::Options;
use super::entry::build;
use super::tests::{B, compute};
use super::{Context, Translated};
use crate::codegen::props::MemoryDef;
use crate::structured::types::AggType;

fn translate(b: &B) -> Translated {
    let ctx = Context {
        chain: "V",
        source: "p#0".into(),
        ..Context::default()
    };
    build(&b.s, &ctx, &Options::default()).expect("build")
}

fn msl(b: &B) -> String {
    translate(b).libraries.remove(0).msl
}

/// The compute sample with a 48 KB shared array next to its small one.
fn big_shared() -> B {
    let mut b = compute();
    b.s.props.properties.shared_memories.push(MemoryDef {
        id: 1,
        name: "big".into(),
        ty: AggType(AggType::U32.0 | AggType::ARRAY.0),
        array_length: 12288,
    });
    b
}

fn big_shared_with_workgroup_id() -> B {
    let mut b = big_shared();
    b.io(
        crate::ir::StorageKind::Input,
        crate::ir::IoVariable::CtaId,
        0,
    );
    b.load_io(
        crate::ir::StorageKind::Input,
        crate::ir::IoVariable::CtaId,
        None,
        0,
        AggType::U32,
    );
    b
}

#[test]
fn shared_memory_past_the_limit_spills_to_a_device_buffer() {
    let t = translate(&big_shared());
    let l = &t.libraries[0];
    for want in [
        "device uchar* dm_tg [[buffer(30)]]",
        "uint3 dm_ntg [[threadgroups_per_grid]]",
        "uint3 dm_tgid [[threadgroup_position_in_grid]]",
        "ulong dm_tgbase = ulong(((dm_tgid.z * dm_ntg.y + dm_tgid.y) * dm_ntg.x + dm_tgid.x) % 512u) * 49184ul;",
        "device uint (&sm0)[8] = *(device uint (*)[8])(dm_tg + dm_tgbase + 0);",
        "device uint (&sm1)[12288] = *(device uint (*)[12288])(dm_tg + dm_tgbase + 32);",
        "threadgroup_barrier(mem_flags::mem_device | mem_flags::mem_threadgroup);",
        "reinterpret_cast<device atomic_uint*>",
    ] {
        assert!(l.msl.contains(want), "missing {want}\n{}", l.msl);
    }
    assert!(!l.msl.contains("threadgroup uint"), "{}", l.msl);
    let r = l.reflection.as_ref().expect("reflection");
    assert!(
        r.extra
            .iter()
            .any(|(k, v)| k == "tg_spill" && *v == nxb_pyjson::Value::Int(49184))
    );
}

#[test]
fn the_workgroup_id_parameter_is_reused_by_the_spill() {
    let m = msl(&big_shared_with_workgroup_id());
    assert!(m.contains("gl_WorkGroupID [[threadgroup_position_in_grid]]"));
    assert!(!m.contains("dm_tgid"), "{m}");
}

#[test]
fn shared_memory_within_the_limit_stays_in_the_threadgroup() {
    let t = translate(&compute());
    let l = &t.libraries[0];
    assert!(l.msl.contains("threadgroup uint sm0[8];"));
    assert!(!l.msl.contains("dm_tg"));
    let r = l.reflection.as_ref().expect("reflection");
    assert!(r.extra.iter().all(|(k, _)| k != "tg_spill"));
}

/// The samples compile with Apple's compiler.
#[test]
#[ignore]
fn gaps_metal_compiles_the_samples() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scratch");
    std::fs::create_dir_all(&dir).expect("scratch");
    let samples = [("cs_spill", big_shared()), ("cs_spill_wg", big_shared_with_workgroup_id())];
    for (n, b) in samples {
        let src = dir.join(format!("gaps_{n}.metal"));
        let air = dir.join(format!("gaps_{n}.air"));
        let text = msl(&b);
        std::fs::write(&src, &text).expect("write");
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
            "{n}: {}\n{text}",
            String::from_utf8_lossy(&o.stderr)
        );
    }
}
