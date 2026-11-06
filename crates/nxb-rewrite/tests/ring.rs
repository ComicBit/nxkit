//! The finder `ue_ring` on small synthetic Unreal-like main modules, against the output of the reference script
//! (`tools/gen_synth_ring.py` makes the modules and `tests/data/ring/expected.tsv` and `<case>.log`).
//! Each case compares the length and the SHA-256 of each output file and the lines the script prints.

use nxb_model::sha256;
use nxb_rewrite::{Options, patches::PatchTable, rewrite};

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[test]
fn engine_ring_equals_the_reference_script() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/ring");
    let expected = std::fs::read_to_string(dir.join("expected.tsv")).expect("expected.tsv");
    let mut cases: Vec<String> = std::fs::read_dir(&dir)
        .expect("fixture folder")
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.strip_suffix(".nso").map(str::to_string)
        })
        .collect();
    cases.sort();
    assert!(cases.len() >= 10, "{cases:?}");
    let opts = Options {
        name: "main".into(),
        ..Options::default()
    };
    for case in &cases {
        let nso = std::fs::read(dir.join(format!("{case}.nso"))).expect("module");
        let out = rewrite(&nso, &opts, &PatchTable::default()).expect("rewrite");
        let mut want: Vec<(&str, usize, &str)> = expected
            .lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                (f[0] == case.as_str()).then(|| (f[1], f[2].parse().expect("length"), f[3]))
            })
            .collect();
        for (name, data) in &out.files {
            if name.ends_with("build_src.sha256") {
                continue;
            }
            let at = want
                .iter()
                .position(|w| w.0 == name)
                .unwrap_or_else(|| panic!("{case}: {name} has no reference"));
            let (_, len, sum) = want.remove(at);
            assert_eq!(data.len(), len, "{case}: {name}: length");
            assert_eq!(hex(&sha256(data).0), sum, "{case}: {name}");
        }
        assert!(want.is_empty(), "{case}: not written: {want:?}");
        let log = std::fs::read_to_string(dir.join(format!("{case}.log"))).expect("log");
        let script: Vec<&str> = log.lines().filter(|l| !l.starts_with("main {")).collect();
        let ours: Vec<&str> = out
            .log
            .iter()
            .map(String::as_str)
            .filter(|l| !l.starts_with("main {"))
            .collect();
        assert_eq!(ours, script, "{case}: log");
        let found = ours
            .iter()
            .any(|l| l.contains("4 copies") && !l.starts_with("[RED]"));
        let red = ours.iter().any(|l| l.starts_with("[RED] main: Unreal's"));
        let rings = ["direct", "got", "base-earlier"].contains(&case.as_str());
        assert_eq!(found, rings, "{case}: found");
        let reds = [
            "not-registered",
            "no-rotation",
            "no-layout",
            "two-loads",
            "other-variable",
            "base-not-at-count",
        ];
        assert_eq!(red, reds.contains(&case.as_str()), "{case}: RED");
    }
}
