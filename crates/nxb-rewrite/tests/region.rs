//! The finder `ue_region` on small synthetic main modules, against the output of the reference script
//! (`tools/gen_synth_region.py` makes the modules and `tests/data/region/expected.tsv` and `<case>.log`).
//! Each case compares the length and the SHA-256 of each output file and the lines the script prints.

use nxb_model::sha256;
use nxb_rewrite::{Options, patches::PatchTable, rewrite};

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[test]
fn engine_region_equals_the_reference_script() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/region");
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
    }
}

#[test]
fn engine_region_gate_and_size_move_are_in_the_image() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/region");
    let opts = Options {
        name: "main".into(),
        ..Options::default()
    };
    let out = rewrite(
        &std::fs::read(dir.join("x1-after.nso")).expect("module"),
        &opts,
        &PatchTable::default(),
    )
    .expect("rewrite");
    let obs = out
        .files
        .iter()
        .find(|(n, _)| n.ends_with(".obs.tsv"))
        .map(|(_, d)| String::from_utf8_lossy(d).into_owned())
        .unwrap_or_default();
    // the gate number is OBS_UE_REGION (26) with the register (x8) in the upper bits
    assert!(obs.contains("\t0x81a\t") || obs.contains("2074"), "{obs}");
}
