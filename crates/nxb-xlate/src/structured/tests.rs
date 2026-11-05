use super::program::{FunctionIn, make_structured_program};
use super::text::ast_text;
use crate::codegen::props::Props;
use crate::codegen::testutil::*;
use crate::ir::{Ir, text};

/// Structure every key's final `xform` IR and compare the AST with the probe's `ast` dump.
#[test]
fn ast_matches_probe() {
    let Some(entries) = crate::corpus::load() else {
        eprintln!("skipped: corpus env missing");
        return;
    };
    let (mut ok, mut bad) = (0, 0);
    for e in &entries {
        let key = &e.job.key;
        let (Some(xf), Some(pr), Some(want)) = (
            last_attempt(&e.dump_dir, key, "sfunc"),
            last_attempt(&e.dump_dir, key, "props"),
            last_attempt(&e.dump_dir, key, "ast"),
        ) else {
            continue;
        };
        let props = Props::parse(&pr).unwrap();
        let mut ir = Ir::default();
        let mut funcs = text::parse(&mut ir, &xf).unwrap();
        apply_use_order(
            &mut ir,
            &funcs,
            &last_attempt(&e.dump_dir, key, "suses").unwrap(),
        )
        .unwrap();
        let meta = last_attempt(&e.dump_dir, key, "sfuncmeta").unwrap();
        let metas: Vec<Vec<&str>> = meta
            .lines()
            .skip(1)
            .map(|l| l.split(' ').collect())
            .collect();
        let mut ins: Vec<FunctionIn> = funcs
            .iter_mut()
            .map(|f| FunctionIn {
                name: metas[f.index][1].to_string(),
                blocks: &mut f.blocks,
                in_arguments_count: metas[f.index][3].parse().unwrap(),
                out_arguments_count: metas[f.index][4].parse().unwrap(),
                returns_value: metas[f.index][2] == "1",
            })
            .collect();
        let mut res = ReplayResources {
            map: props.cb_slot_to_binding.clone(),
        };
        let got = make_structured_program(
            &mut ir,
            &mut ins,
            &props.definitions,
            &mut res,
            props.passthrough_attributes,
            true,
        )
        .map(|i| ast_text(&i));
        match got {
            Ok(g) if g == want => ok += 1,
            Ok(g) => {
                bad += 1;
                if bad <= 3 {
                    let pos = g.lines().zip(want.lines()).position(|(a, b)| a != b);
                    eprintln!(
                        "{key}: differs at line {pos:?}\n got: {:?}\nwant: {:?}",
                        pos.and_then(|p| g.lines().nth(p)),
                        pos.and_then(|p| want.lines().nth(p))
                    );
                }
            }
            Err(err) => {
                bad += 1;
                if bad <= 3 {
                    eprintln!("{key}: {err}");
                }
            }
        }
    }
    eprintln!("ast identical: {ok} of {} (bad {bad})", ok + bad);
    assert_eq!(bad, 0);
}
