//! The rewrite-site census on small synthetic modules, against the output of the reference script `insn_census.py`
//! (`tools/gen_synth_census.py` makes the modules and `tests/data/census/<case>.txt`).

use nxb_rewrite::census;

#[test]
fn the_census_equals_the_reference_script() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/census");
    for case in ["sites", "plain", "nothing"] {
        let nso = std::fs::read(dir.join(format!("{case}.nso"))).expect("module");
        let want = std::fs::read_to_string(dir.join(format!("{case}.txt"))).expect("output");
        let got = census(&format!("{case}.nso"), &nso).expect("census");
        assert_eq!(got, want, "{case}");
    }
}

#[test]
fn a_damaged_module_is_an_error() {
    assert!(census("bad.nso", b"NSO0 too short").is_err());
    assert!(census("bad.nso", &[]).is_err());
}
