use super::*;
use crate::ir::text;

fn run_cfg(ir: &mut Ir, emit: &str) -> Result<Cfg, String> {
    let funcs = text::parse(ir, emit)?;
    Cfg::create(ir, &funcs[0].blocks[0].ops)
}

#[test]
fn cfg_and_ssa_match_dumps() {
    let Some(entries) = crate::corpus::load() else {
        return;
    };
    let (mut keys, mut cfg_ok, mut ssa_ok) = (0, 0, 0);
    let mut seen = std::collections::HashSet::new();
    for e in &entries {
        if !seen.insert(e.job.key.clone()) {
            continue;
        }
        let (Some(emit), Some(cfg_t), Some(ssa_t)) = (
            e.first_attempt("emit"),
            e.first_attempt("cfg"),
            e.first_attempt("ssa"),
        ) else {
            continue;
        };
        keys += 1;
        let mut ir = Ir::default();
        let mut cfg = run_cfg(&mut ir, &emit).expect("create");
        register_usage::fixup_calls(&mut ir, &cfg.blocks, &[]).expect("fixup");
        let mut out = String::new();
        text::print_blocks(&ir, 0, &cfg.blocks, &mut out);
        if out == cfg_t {
            cfg_ok += 1;
        } else {
            eprintln!("cfg differs: {}", e.job.key);
        }
        cfg.find_dominators().expect("dom");
        cfg.find_dominance_frontiers().expect("df");
        ssa::rename(&mut ir, &mut cfg).expect("ssa");
        let mut out = String::new();
        text::print_blocks(&ir, 0, &cfg.blocks, &mut out);
        if out == ssa_t {
            ssa_ok += 1;
        } else {
            eprintln!("ssa differs: {}", e.job.key);
        }
    }
    eprintln!("cfg identical {cfg_ok}/{keys}, ssa identical {ssa_ok}/{keys}");
    assert_eq!(cfg_ok, keys);
    assert_eq!(ssa_ok, keys);
}
