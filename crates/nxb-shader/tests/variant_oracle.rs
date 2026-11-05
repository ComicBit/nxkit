//! Differential test of the variant ports (`dmsh_pull`, `dmsh_vin`, `dmsh_fetch`) against the Python oracle TSV on a local
//! output folder: `NXB_VARIANT_ORACLE=<tsv> NXB_SIDE_BDIR=<b-dir> cargo test -p nxb-shader --test variant_oracle -- --ignored`.
//! TSV lines: key, kind, sha256 of the variant source or `none`, `json.dumps(info)` or the reason.

use nxb_pyjson::{Value, dumps};
use nxb_shader::variants::{fetch, pull, vin};
use std::path::PathBuf;

fn line(r: Result<(String, Value), String>) -> String {
    match r {
        Ok((src, info)) => format!(
            "{}\t{}",
            nxb_model::sha256(src.as_bytes()).to_hex(),
            dumps(&info)
        ),
        Err(why) => format!("none\t{why}"),
    }
}

#[test]
#[ignore = "needs local game data and the Python oracle's TSV"]
fn variants_match_python() {
    let (Some(tsv), Some(dir)) = (
        std::env::var_os("NXB_VARIANT_ORACLE"),
        std::env::var_os("NXB_SIDE_BDIR"),
    ) else {
        return;
    };
    let dir = PathBuf::from(dir);
    let text = std::fs::read_to_string(tsv).unwrap();
    let mut stats = std::collections::BTreeMap::<String, (usize, usize)>::new();
    let mut bad = Vec::new();
    for l in text.lines() {
        let f: Vec<&str> = l.splitn(4, '\t').collect();
        let (key, kind) = (f[0], f[1]);
        let msl = std::fs::read_to_string(dir.join(format!("{key}_b.metal"))).unwrap();
        let got = match kind {
            "pull" => line(pull::variant(&msl)),
            "vin" => line(vin::variant(&msl)),
            "fetch" => {
                let refl = nxb_pyjson::parse(
                    &std::fs::read_to_string(dir.join(format!("{key}_b.json"))).unwrap(),
                )
                .unwrap();
                let bindings = refl.get("bindings").cloned().unwrap_or(Value::List(vec![]));
                line(fetch::variant(&msl, &bindings, false))
            }
            _ => continue,
        };
        let want = format!("{}\t{}", f[2], f[3]);
        let e = stats.entry(kind.to_string()).or_default();
        if got == want {
            e.0 += 1;
        } else {
            e.1 += 1;
            bad.push(format!("{key} {kind}:\n want {want}\n got  {got}"));
        }
    }
    eprintln!("variants (equal, different): {stats:?}");
    for b in bad.iter().take(6) {
        eprintln!("{b}");
    }
    assert!(bad.is_empty());
}
