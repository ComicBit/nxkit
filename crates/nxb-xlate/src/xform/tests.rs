use super::*;
use crate::ir::text;
use crate::opt::tests::{replay_res, to_ssa};
use crate::opt::{Env, PassCtx};

fn stage_of(s: &str) -> ShaderStage {
    match s {
        "cs" => ShaderStage::Compute,
        "vs" => ShaderStage::Vertex,
        "gs" => ShaderStage::Geometry,
        _ => ShaderStage::Fragment,
    }
}

#[test]
fn xform_vs_dump() {
    let Some(entries) = crate::corpus::load() else {
        return;
    };
    let Some(dump) = std::env::var_os("NXB_XLATE_DUMP") else {
        return;
    };
    let (mut keys, mut ok, mut errs) = (0, 0, 0);
    let mut seen = std::collections::HashSet::new();
    for e in &entries {
        if !seen.insert(e.job.key.clone()) {
            continue;
        }
        let p = std::path::Path::new(&dump);
        let (Some(emit), Some(xf), Ok(res_t), Ok(dec)) = (
            e.first_attempt("emit"),
            e.first_attempt("xform"),
            std::fs::read_to_string(p.join(format!("{}.res.txt", e.job.key))),
            std::fs::read_to_string(p.join(format!("{}.decoded.txt", e.job.key))),
        ) else {
            continue;
        };
        keys += 1;
        let features: u32 = dec
            .lines()
            .find(|l| l.starts_with("features="))
            .and_then(|l| l[9..].split(' ').next())
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let stage = stage_of(&e.job.stage);
        let mut ir = Ir::default();
        let mut cfg = to_ssa(&mut ir, &emit).expect("ssa");
        let (mut rm, mut acc) = replay_res(&e.job, &res_t, "emit");
        let mut env = Env {
            rm: &mut rm,
            acc: &mut acc,
        };
        let pctx = PassCtx {
            fragment: stage == ShaderStage::Fragment,
            host_supports_float64: true,
        };
        let mut features = features;
        let mut hfm = Hfm::new(1, stage);
        let mut out = String::new();
        let r = crate::opt::run_pass(&mut ir, &mut cfg.blocks, &mut env, &pctx, &mut hfm).and_then(
            |()| {
                let mut ctx = XformCtx {
                    stage,
                    used_features: &mut features,
                    hfm: &mut hfm,
                    definitions: None,
                };
                run_pass(&mut ir, &mut cfg.blocks, &mut env, &mut ctx)
            },
        );
        match r {
            Ok(()) => text::print_blocks(&ir, 0, &cfg.blocks, &mut out),
            Err(m) => {
                errs += 1;
                if std::env::var_os("NXB_LIST").is_some() {
                    eprintln!("err {} {m}", e.job.key);
                }
            }
        }
        if out == xf {
            ok += 1;
        } else if std::env::var_os("NXB_LIST").is_some() {
            eprintln!("fail {}", e.job.key);
        } else if std::env::var_os("NXB_SHOW").is_some_and(|k| k == *e.job.key) {
            let _ = std::fs::write(std::env::temp_dir().join("xform-mine.txt"), &out);
            let _ = std::fs::write(std::env::temp_dir().join("xform-ref.txt"), &xf);
        }
    }
    eprintln!("xform identical {ok}/{keys} ({errs} errors)");
}
