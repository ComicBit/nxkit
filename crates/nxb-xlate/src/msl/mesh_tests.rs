//! Hand-built task and mesh programs through the MSL back end.
#![cfg(test)]

use super::mesh::{memory, own_shared};
use super::{Context, Failure, Translated, translate};
use crate::job::{MeshStage, parse_jobs};
use crate::translation::emit_test_util::{AEXIT, APT};
use nxb_pyjson::Value;

fn mesh(kind: &str) -> MeshStage {
    MeshStage {
        kind: kind.into(),
        threads: 32,
        payload: 0x20,
        task: true,
        max_v: 3,
        max_p: 1,
        vertex: vec![
            (None, 0),
            (None, 1),
            (None, 2),
            (None, 3),
            (Some(1), 0),
            (Some(1), 2),
        ],
        primitive: vec![(2, 0)],
        refused: None,
    }
}

fn mesh_json(m: &MeshStage) -> String {
    let comps = |l: Vec<(Option<i32>, i32)>| {
        let e: Vec<String> = l
            .iter()
            .map(|(n, c)| format!("[{}, {c}]", n.map_or("null".to_string(), |n| n.to_string())))
            .collect();
        format!("[{}]", e.join(", "))
    };
    let prim = comps(m.primitive.iter().map(|&(n, c)| (Some(n), c)).collect());
    format!(
        r#"{{"kind": "{}", "threads": {}, "payload": {}, "task": {}, "max_v": {}, "max_p": {}, "vertex": {}, "primitive": {}, "refused": {}}}"#,
        m.kind,
        m.threads,
        m.payload,
        m.task,
        m.max_v,
        m.max_p,
        comps(m.vertex.clone()),
        prim,
        m.refused
            .as_ref()
            .map_or("null".to_string(), |r| format!("\"{r}\""))
    )
}

/// ISBERD R1, R2 of the stage memory.
fn isberd(mode: u64) -> (u64, u64) {
    (0x923 | APT | 1 << 16 | 2 << 24, mode << 8)
}

/// AST [R2 + 4], Rb of the stage memory.
fn ast(mode: u64, rb: u64) -> (u64, u64) {
    (0x927 | APT | 2 << 24 | rb << 32 | 4 << 40, mode << 8)
}

fn run_with(
    m: &MeshStage,
    ops: &[(u64, u64)],
    mutate: impl Fn(&mut Vec<u8>),
) -> Result<Translated, Failure> {
    let (lay, size) = memory(m);
    let json = format!(
        r#"[{{"key": "m0", "stage": "cs", "arch": "sm86", "local": [{}, 1, 1], "shared": {size}, "stage_memory": [{}, {}, {}, {}], "mesh_stage": {}}}]"#,
        m.threads,
        lay[0],
        lay[1],
        lay[2],
        lay[3],
        mesh_json(m)
    );
    let job = parse_jobs(&json).expect("job").remove(0);
    let mut code = Vec::new();
    for (lo, hi) in ops.iter().chain([AEXIT].iter()) {
        code.extend(lo.to_le_bytes());
        code.extend(hi.to_le_bytes());
    }
    mutate(&mut code);
    translate(&job, &code, &Context::default())
}

fn run(m: &MeshStage, ops: &[(u64, u64)]) -> Result<Translated, Failure> {
    run_with(m, ops, |_| {})
}

fn ms_ops() -> Vec<(u64, u64)> {
    vec![
        isberd(0x78),
        isberd(0x38),
        ast(0xf8, 3),
        ast(0x88, 3),
        ast(0x80, 3),
    ]
}

fn ts_ops() -> Vec<(u64, u64)> {
    vec![isberd(0x78), ast(0xb8, 3), ast(0xb8, 1)]
}

fn extra<'a>(r: &'a super::Reflection, k: &str) -> Option<&'a Value> {
    r.extra.iter().find(|(n, _)| n == k).map(|(_, v)| v)
}

#[test]
fn the_stage_memory_follows_the_layout_of_the_engine() {
    let ts = mesh("ts");
    assert_eq!(memory(&ts), ([0, 16, 48, 80], 48 + 32));
    let ms = mesh("ms");
    // one block of 7 components of 32 words each, then 4 + 3 primitive bytes
    assert_eq!(memory(&ms), ([0, 16, 48, 48 + 7 * 128], 48 + 7 * 128 + 8));
    let wide = MeshStage {
        max_v: 33,
        payload: 0,
        ..mesh("ms")
    };
    assert_eq!(memory(&wide).0[2], 16);
    assert_eq!(memory(&wide).0[3], 16 + 2 * 7 * 128);
}

#[test]
fn shared_memory_instructions_are_found_by_opcode() {
    for op in [0x984u64, 0x388, 0x988, 0x38c, 0x98c, 0xf8c] {
        let mut code = op.to_le_bytes().to_vec();
        code.extend([0u8; 8]);
        assert!(own_shared(&code), "{op:x}");
    }
    let mut code = 0x923u64.to_le_bytes().to_vec();
    code.extend([0u8; 8]);
    assert!(!own_shared(&code));
}

#[test]
fn a_mesh_shader_is_a_body_and_two_mesh_functions() {
    let t = run(&mesh("ms"), &ms_ops()).expect("translate");
    assert_eq!(t.stage, "ms");
    assert_eq!(t.libraries.len(), 1);
    let msl = &t.libraries[0].msl;
    let body = "static inline __attribute__((always_inline)) void dm_stage_body(";
    assert!(msl.contains(body), "{msl}");
    assert!(msl.contains("threadgroup uint (&sm0)["), "{msl}");
    assert!(!msl.contains("kernel void"), "{msl}");
    // the byte stores of the primitive indices are a compare-and-swap loop on the stage memory
    assert!(msl.contains("threadgroup atomic_uint*"), "{msl}");
    assert!(
        msl.contains("atomic_compare_exchange_weak_explicit"),
        "{msl}"
    );
    assert!(msl.contains("[[mesh]] void main0("), "{msl}");
    assert!(msl.contains("[[mesh]] void main0_z("), "{msl}");
    assert!(
        msl.contains("metal::mesh<dm_vertex_t, dm_prim_t, 3, 1, metal::topology::triangle>"),
        "{msl}"
    );
    assert!(msl.contains("float4 a1 [[user(locn1)]];"), "{msl}");
    assert!(msl.contains("float4 p2 [[user(locn2)]];"), "{msl}");
    assert!(
        msl.contains("const object_data dm_payload_t& dm_payload [[payload]]"),
        "{msl}"
    );
    assert!(!msl.contains("dm_first"), "{msl}");
    // the depth convention only in main0
    assert_eq!(msl.matches("v.position.z = fma(").count(), 1, "{msl}");
    // a component the SPH does not enable is 0, and 1 for w
    assert!(
        msl.contains("v.a1 = float4(as_type<float>(dm_mem[b + 128u]), 0.0, as_type<float>(dm_mem[b + 160u]), 1.0);"),
        "{msl}"
    );
    assert!(
        msl.contains("p.p2 = float4(as_type<float>(dm_mem[b + 192u]), 0.0, 0.0, 1.0);"),
        "{msl}"
    );
}

#[test]
fn a_mesh_shader_reflection_names_its_stage() {
    let t = run(&mesh("ms"), &ms_ops()).expect("translate");
    let r = t.libraries[0].reflection.as_ref().expect("reflection");
    assert_eq!(r.key, "m0_b");
    assert_eq!(r.stage, "ms");
    assert_eq!(r.threadgroup_size, Some([32, 1, 1]));
    assert!(
        r.entries
            .iter()
            .any(|(k, v)| k == "entry_z" && v.as_str() == Some("main0_z"))
    );
    assert_eq!(r.to_value().get("msl_entry_kind"), Some(&Value::Null));
    let m = extra(r, "mesh_stage").expect("mesh_stage");
    assert_eq!(m.get("kind").and_then(Value::as_str), Some("ms"));
    assert_eq!(m.get("threads").and_then(Value::as_i64), Some(32));
    assert_eq!(m.get("payload").and_then(Value::as_i64), Some(0x20));
    assert_eq!(m.get("task"), Some(&Value::Bool(true)));
}

#[test]
fn a_mesh_shader_without_a_task_reads_the_first_workgroup_from_the_host() {
    let m = MeshStage {
        task: false,
        ..mesh("ms")
    };
    let t = run(&m, &ms_ops()).expect("translate");
    let msl = &t.libraries[0].msl;
    assert!(
        msl.contains("constant uint* dm_first [[buffer(29)]]"),
        "{msl}"
    );
    assert!(msl.contains("dm_mem[0] = dm_first[0] + "), "{msl}");
    assert!(!msl.contains("object_data dm_payload_t&"), "{msl}");
    let r = t.libraries[0].reflection.as_ref().expect("reflection");
    let ms = extra(r, "mesh_stage").expect("mesh_stage");
    assert_eq!(ms.get("task"), Some(&Value::Bool(false)));
}

#[test]
fn a_task_shader_is_an_object_function() {
    let t = run(&mesh("ts"), &ts_ops()).expect("translate");
    assert_eq!(t.stage, "ts");
    let msl = &t.libraries[0].msl;
    assert!(msl.contains("[[object]] void main0("), "{msl}");
    assert!(
        msl.contains("object_data dm_payload_t& dm_payload [[payload]], mesh_grid_properties dm_grid, constant uint* dm_first [[buffer(29)]]"),
        "{msl}"
    );
    assert!(msl.contains("struct dm_payload_t { uint w[8]; };"), "{msl}");
    assert!(msl.contains("dm_payload.w[i] = dm_mem[12u + i];"), "{msl}");
    assert!(
        msl.contains("set_threadgroups_per_grid(uint3(min(dm_mem[13], 1024u), 1u, 1u))"),
        "{msl}"
    );
    assert!(!msl.contains("[[mesh]]"), "{msl}");
    let r = t.libraries[0].reflection.as_ref().expect("reflection");
    assert_eq!(r.stage, "ts");
    assert!(!r.entries.iter().any(|(k, _)| k == "entry_z"));
}

#[test]
fn the_wrapper_has_thread_ids_of_its_own_when_the_body_reads_none() {
    let t = run(&mesh("ts"), &ts_ops()).expect("translate");
    let msl = &t.libraries[0].msl;
    assert!(
        msl.contains("uint dm_lid [[thread_position_in_threadgroup]]"),
        "{msl}"
    );
    assert!(
        msl.contains("uint dm_tg [[threadgroup_position_in_grid]]"),
        "{msl}"
    );
}

#[test]
fn the_wrapper_passes_the_thread_ids_the_body_reads() {
    // S2R R3, SR_TID.X and S2R R4, SR_CTAID.X on write barrier 0, then payload stores that wait for it
    let s2r = |rd: u64, sr: u64| (0x919 | APT | rd << 16, sr << 8);
    let wait = |(lo, hi): (u64, u64)| (lo, hi | 1 << 52);
    let ops = [
        isberd(0x78),
        s2r(3, 0x21),
        s2r(4, 0x25),
        wait(ast(0xb8, 3)),
        ast(0xb8, 4),
    ];
    let t = run(&mesh("ts"), &ops).expect("translate");
    let msl = &t.libraries[0].msl;
    assert!(
        msl.contains("uint3 gl_LocalInvocationID [[thread_position_in_threadgroup]]"),
        "{msl}"
    );
    assert!(
        msl.contains("uint3 gl_WorkGroupID [[threadgroup_position_in_grid]]"),
        "{msl}"
    );
    assert!(!msl.contains("dm_lid"), "{msl}");
    assert!(msl.contains("gl_LocalInvocationID.x == 0u"), "{msl}");
    assert!(msl.contains("dm_first[0] + gl_WorkGroupID.x;"), "{msl}");
    assert!(
        msl.contains("dm_stage_body(gl_LocalInvocationID, gl_WorkGroupID, dm_mem);"),
        "{msl}"
    );
}

#[test]
fn a_refused_mesh_shader_says_why() {
    let own = run_with(&mesh("ms"), &ms_ops(), |c| {
        c.splice(0..2, 0x984u16.to_le_bytes());
    });
    let e = own.expect_err("refused");
    assert_eq!(e.reason, "mesh stage failed");
    assert!(
        e.detail.contains("shared memory of its own"),
        "{}",
        e.detail
    );
    let big = MeshStage {
        payload: 0x9000,
        ..mesh("ts")
    };
    let e = run(&big, &ts_ops()).expect_err("too big");
    assert_eq!(e.reason, "mesh stage failed");
    let refused = MeshStage {
        refused: Some("by the build".into()),
        ..mesh("ms")
    };
    let e = run(&refused, &ms_ops()).expect_err("refused");
    assert_eq!(e.detail, "by the build");
}

#[test]
fn facts_that_metal_cannot_hold_are_refused() {
    let wide = MeshStage {
        max_v: 257,
        ..mesh("ms")
    };
    let e = run(&wide, &ms_ops()).expect_err("vertices");
    assert!(e.detail.contains("exceed a Metal mesh"), "{}", e.detail);
    let nopos = MeshStage {
        vertex: vec![(Some(1), 0)],
        ..mesh("ms")
    };
    let e = run(&nopos, &ms_ops()).expect_err("position");
    assert!(e.detail.contains("no position"), "{}", e.detail);
    let none = MeshStage {
        threads: 0,
        ..mesh("ms")
    };
    let e = run(&none, &ms_ops()).expect_err("threads");
    assert_eq!(e.reason, "mesh stage failed");
}

#[test]
fn a_job_with_another_stage_memory_is_refused() {
    let m = mesh("ms");
    let (lay, size) = memory(&m);
    let json = format!(
        r#"[{{"key": "m0", "stage": "cs", "arch": "sm86", "local": [32, 1, 1], "shared": {size}, "stage_memory": [0, 16, {}, {}], "mesh_stage": {}}}]"#,
        lay[2] + 4,
        lay[3],
        mesh_json(&m)
    );
    let job = parse_jobs(&json).expect("job").remove(0);
    let e = translate(&job, &[0u8; 16], &Context::default()).expect_err("layout");
    assert_eq!(e.reason, "mesh stage failed");
    assert!(e.detail.contains("does not match"), "{}", e.detail);
}

#[test]
fn a_task_or_mesh_stage_name_is_not_a_job_stage() {
    let job = parse_jobs(r#"[{"key": "m0", "stage": "ms"}]"#)
        .expect("job")
        .remove(0);
    let e = translate(&job, &[], &Context::default()).expect_err("stage");
    assert!(
        e.detail.contains("stage cs with a mesh_stage"),
        "{}",
        e.detail
    );
}

/// The wrappers compile with Apple's compiler. Run by hand: `cargo test -p nxb-xlate -- --ignored metal`.
#[test]
#[ignore]
fn metal_compiles_the_task_and_mesh_functions() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scratch");
    std::fs::create_dir_all(&dir).expect("scratch");
    let no_task = MeshStage {
        task: false,
        ..mesh("ms")
    };
    let cases = [
        ("ts", mesh("ts"), ts_ops()),
        ("ms", mesh("ms"), ms_ops()),
        ("msn", no_task, ms_ops()),
    ];
    for (name, m, ops) in cases {
        let l = &run(&m, &ops).expect("translate").libraries[0];
        let src = dir.join(format!("mesh_{name}.metal"));
        let air = dir.join(format!("mesh_{name}.air"));
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
            "{name}: {}\n{}",
            String::from_utf8_lossy(&o.stderr),
            l.msl
        );
    }
}
