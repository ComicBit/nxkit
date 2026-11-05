//! Differential test of the c1_guard and cb_extent ports against the Python oracle TSV on a local output folder:
//! `NXB_GUARD_ORACLE=<tsv> NXB_SIDE_BDIR=<b-dir> cargo test -p nxb-shader --test guard_oracle -- --ignored`.
//! TSV lines: `c1 <name> <key|->` and `cb <name> <row|->`.

use nxb_shader::guards::{c1_unproven, cb_row};
use std::path::PathBuf;

#[test]
#[ignore = "needs local game data and the Python oracle's TSV"]
fn guards_match_python() {
    let (Some(tsv), Some(dir)) = (
        std::env::var_os("NXB_GUARD_ORACLE"),
        std::env::var_os("NXB_SIDE_BDIR"),
    ) else {
        return;
    };
    let dir = PathBuf::from(dir);
    let text = std::fs::read_to_string(tsv).unwrap();
    let (mut eq, mut bad) = ([0usize; 2], Vec::new());
    for l in text.lines() {
        let f: Vec<&str> = l.splitn(3, '\t').collect();
        let (kind, name, want) = (f[0], f[1], f[2]);
        let key = name.split("_b").next().unwrap();
        let refl = std::fs::read_to_string(dir.join(format!("{key}_b.json")))
            .ok()
            .and_then(|t| nxb_pyjson::parse(&t).ok());
        let got = match kind {
            "c1" => {
                let src = std::fs::read_to_string(dir.join(name)).unwrap();
                if c1_unproven(&src, refl.as_ref()) {
                    key.to_string()
                } else {
                    "-".into()
                }
            }
            _ => {
                let src = std::fs::read_to_string(dir.join(name.replace(".json", ".metal")))
                    .unwrap_or_default();
                refl.as_ref()
                    .and_then(|r| cb_row(&src, r))
                    .map_or("-".into(), |toks| format!("{key}\t{toks}"))
            }
        };
        let i = usize::from(kind != "c1");
        if got == want {
            eq[i] += 1;
        } else {
            bad.push(format!("{kind} {name}: want {want:?} got {got:?}"));
        }
    }
    eprintln!(
        "guards equal: c1 {}, cb {}; different {}",
        eq[0],
        eq[1],
        bad.len()
    );
    for b in bad.iter().take(8) {
        eprintln!("{b}");
    }
    assert!(bad.is_empty());
}
