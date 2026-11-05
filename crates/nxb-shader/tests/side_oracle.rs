//! Differential test of the `dmsh_side` port against the Python oracle on a local output folder (game data stays local).
//!
//! `NXB_SIDE_ORACLE=<tsv> NXB_SIDE_BDIR=<b-dir> cargo test -p nxb-shader --test side_oracle -- --ignored`, the TSV made
//! by the oracle script (one line per `<key>_b.metal`: name, sha256 of the unit or `none`, reason or alpha names, logic
//! names).

use nxb_shader::side::unit;
use std::path::PathBuf;

#[test]
#[ignore = "needs local game data and the Python oracle's TSV"]
fn unit_matches_python() {
    let (Some(tsv), Some(dir)) = (
        std::env::var_os("NXB_SIDE_ORACLE"),
        std::env::var_os("NXB_SIDE_BDIR"),
    ) else {
        eprintln!("skipped: NXB_SIDE_ORACLE and NXB_SIDE_BDIR not set");
        return;
    };
    let text = std::fs::read_to_string(tsv).unwrap();
    let (mut ok, mut bad) = (0usize, Vec::new());
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let msl = std::fs::read_to_string(PathBuf::from(&dir).join(f[0])).unwrap();
        let got = match unit(&msl) {
            Ok(u) => format!(
                "{}\t{}\t{}",
                nxb_model::sha256(u.source.as_bytes()).to_hex(),
                u.alpha.join(","),
                u.logic.join(",")
            ),
            Err(why) => format!("none\t{why}\t"),
        };
        let want = format!("{}\t{}\t{}", f[1], f[2], f.get(3).unwrap_or(&""));
        if got == want {
            ok += 1;
        } else {
            bad.push(format!("{}: want {want:?} got {got:?}", f[0]));
        }
    }
    eprintln!("side unit: {ok} equal, {} different", bad.len());
    for b in bad.iter().take(10) {
        eprintln!("{b}");
    }
    assert!(bad.is_empty());
}
