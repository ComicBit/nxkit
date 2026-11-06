//! The job of an NVN2 task or mesh shader (`nxb_shader::job::job_for`) is accepted by the translator
//! (`nxb_xlate::msl::mesh::check`) and gives the reflection the host reads.

use nxb_shader::job::{JobOptions, job_for};
use nxb_xlate::job::Job as XJob;
use nxb_xlate::msl::{Context, mesh, translate};

/// `EXIT` of the Ampere encoding.
const EXIT: u64 = 0x94d | 0x7 << 12 | 1 << 23;

/// A package record: a 0x38 byte header, the code area (an SPH of 0x80 bytes, the instructions, for a mesh shader a
/// second SPH) and the control section.
fn record(kind: u8, ms: bool) -> Vec<u8> {
    let mut area = vec![0u8; 0x80];
    area[0x35] = 0xf0;
    area[0x36] = 0x03;
    let mut insts: Vec<u8> = Vec::new();
    for w in [EXIT, 0u64] {
        insts.extend(w.to_le_bytes());
    }
    let inst_len = insts.len();
    area.extend(insts);
    let second = area.len();
    if ms {
        let mut sph = vec![0u8; 0x80];
        sph[0x37] = 0x01;
        area.extend(sph);
    }
    let mut ctl = vec![0u8; 0xc0];
    ctl[..4].copy_from_slice(&[0x34, 0x12, 0x76, 0x98]);
    ctl[4..8].copy_from_slice(&0xc9u32.to_le_bytes());
    ctl[0x38..0x3c].copy_from_slice(&((0x80 + inst_len) as u32).to_le_bytes());
    ctl[0x56] = kind;
    ctl[0x58..0x5a].copy_from_slice(&32u16.to_le_bytes());
    ctl[0x5a..0x5c].copy_from_slice(&16u16.to_le_bytes());
    if ms {
        ctl[0x5e] = 1;
        ctl[0x60..0x62].copy_from_slice(&6u16.to_le_bytes());
        ctl[0x62..0x64].copy_from_slice(&2u16.to_le_bytes());
        ctl[0x64..0x68].copy_from_slice(&(second as u32).to_le_bytes());
        ctl[0x68] = 0x80;
    }
    let mut rec = vec![0u8; 0x38];
    rec.extend(area);
    rec.extend(ctl);
    rec
}

#[allow(clippy::expect_used, reason = "a test helper")]
fn job(package: &str, rec: &[u8]) -> nxb_shader::job::Job {
    job_for(
        "0123456789abcdef",
        package,
        rec,
        None,
        "0123456789abcdef.bin",
        JobOptions::default(),
    )
    .expect("job")
}

#[test]
fn a_mesh_job_is_a_compute_job_the_translator_accepts() {
    let j = job("ms.bin", &record(7, true));
    assert_eq!(j.json.get("stage").and_then(|v| v.as_str()), Some("cs"));
    assert_eq!(j.json.get("no_facts"), None);
    assert_eq!(j.code.len(), 16);
    assert!(j.json.get("mesh_stage").is_some());
    let x = XJob::from_value(&j.json).expect("xlate job");
    mesh::check(&x, &j.code).expect("check");
    assert_eq!(x.local, Some(vec![32, 1, 1]));
    let t = translate(&x, &j.code, &Context::default()).expect("translate");
    assert_eq!(t.stage, "ms");
    let r = t.libraries[0].reflection.as_ref().expect("reflection");
    let v = r.to_value();
    assert_eq!(v.get("entry_z").and_then(|v| v.as_str()), Some("main0_z"));
    let m = v.get("mesh_stage").expect("mesh_stage");
    assert_eq!(m.get("kind").and_then(|v| v.as_str()), Some("ms"));
    assert_eq!(m.get("threads").and_then(|v| v.as_i64()), Some(32));
}

#[test]
fn a_task_job_is_a_compute_job_the_translator_accepts() {
    let j = job("ts.bin", &record(6, false));
    let x = XJob::from_value(&j.json).expect("xlate job");
    mesh::check(&x, &j.code).expect("check");
    let t = translate(&x, &j.code, &Context::default()).expect("translate");
    assert_eq!(t.stage, "ts");
    let r = t.libraries[0].reflection.as_ref().expect("reflection");
    assert!(r.to_value().get("mesh_stage").is_some());
    assert!(r.to_value().get("entry_z").is_none());
}

#[test]
fn a_stage_with_its_own_shared_memory_is_refused_by_the_job() {
    let mut rec = record(6, false);
    // LDS in the first instruction
    rec[0x38 + 0x80..0x38 + 0x82].copy_from_slice(&0x984u16.to_le_bytes());
    let j = job("ts.bin", &rec);
    let x = XJob::from_value(&j.json).expect("xlate job");
    let e = mesh::check(&x, &j.code).expect_err("refused");
    assert!(e.contains("shared memory of its own"), "{e}");
}
