use crate::patches::PatchTable;

/// A small synthetic snapshot with one row of every kind (no game data).
const SNAP: &str = "file\tsdk_patches.py\t00\n\
file\t../games/0100000000000001/patches.py\t11\n\
file\t../games/0100000000000001/game.tsv\t22\n\
bscope\taaaa\t0100000000000001\n\
cuts\taaaa\t4096\t1f2003d5\td503201f\tthe \"reason\"\\ttab\n\
cuts\taaaa\t8192\t1f2003d5\t00000000\tsecond\n\
obs\taaaa\t64\t3\t1f2003d5\tobserve\n\
cap\taaaa\tDYNAMIC_RESOLUTION\n\
frame\taaaa\n\
fs\taaaa\tlabel\t0\n\
fs\taaaa\tadrp\t16\tL0+0\tS+0\n\
fs\taaaa\tword\t3573751839\n\
fs\taaaa\tsite\t4096\t1f2003d5\n\
fs\taaaa\tb\tL0+8\t4096\t2147483648\n\
fs\taaaa\tput\t4096\twhy\tb:4096:L0+0:0;w:3573751839\n\
fs\taaaa\tscale\n\
bscope\tbbbb\tsdk\n\
cutsempty\tbbbb\n\
obsempty\tbbbb\n\
rule\tsdk\tmain\tfix a\t0\t0\t276\tfp1,fp2\n\
rcut\tsdk\tmain\tfix a\t0\t0\t8\t1f2003d5\td503201f\twhy\n\
rule\t0100000000000001\tmain\tmulti\t0\t1\t100\tfp3\n\
rule\t0100000000000001\tmain\tmulti\t0\t2\t200\tfp4\n\
rcut\t0100000000000001\tmain\tmulti\t0\t2\t4\t1f2003d5\td503201f\tw\n\
gbuild\t0100000000000001\tbuild\tv0\t1.0.0\tmain=aaaa\n";

#[test]
fn toml_round_trip_equals_the_snapshot() {
    let t = PatchTable::parse(SNAP).expect("parses");
    let files = t.to_toml();
    let t2 = PatchTable::from_toml(&files).expect("reads");
    assert_eq!(t.to_snapshot(), t2.to_snapshot());
    assert_eq!(files, t2.to_toml());
    // every kind survived
    assert_eq!(t2.cuts["aaaa"].len(), 2);
    assert_eq!(t2.cuts["aaaa"][0].why, "the \"reason\"\ttab");
    assert!(t2.cuts["bbbb"].is_empty() && t2.obs["bbbb"].is_empty());
    assert_eq!(t2.frame_ops["aaaa"].len(), 7);
    assert!(t2.rules["0100000000000001"]["main"][0].multi);
    // the canonical snapshot holds every row of the input (fsno carries no reason, order is canonical)
    let mut a: Vec<&str> = SNAP.lines().collect();
    let c = t.to_snapshot();
    let mut b: Vec<&str> = c.lines().collect();
    a.sort_unstable();
    b.sort_unstable();
    assert_eq!(a, b);
}

#[test]
fn bindings_and_original_bytes_are_checked() {
    let files = PatchTable::parse(SNAP).expect("parses").to_toml();
    let edit = |from: &str, to: &str| -> Vec<(String, String)> {
        files
            .iter()
            .map(|(n, t)| (n.clone(), t.replacen(from, to, 1)))
            .collect()
    };
    let e = PatchTable::from_toml(&edit("binding = \"sdk\"", "binding = \"bogus\"")).unwrap_err();
    assert!(e.to_string().contains("unknown binding"), "{e}");
    let e = PatchTable::from_toml(&edit("old = \"1f2003d5\"", "old = \"\"")).unwrap_err();
    assert!(e.to_string().contains("original bytes"), "{e}");
    let e = PatchTable::from_toml(&edit("kind = \"cut\"", "kind = \"splice\"")).unwrap_err();
    assert!(e.to_string().contains("unknown rule kind"), "{e}");
}

#[test]
fn damaged_files_are_errors() {
    for bad in [
        "scope = \"x\"\n[[rule]]\nbinding = \"build\nkind = 1\n",
        "scope = \"x\"\n[[rule]]\nbinding = build\n",
        "scope = \"x\"\n[[rule.step]]\nop = \"word\"\n",
        "[[rule]]\nkey\n",
        "scope = \"x\"\nscope = \"y\"\n",
    ] {
        assert!(
            PatchTable::from_toml(&[("x.toml".into(), bad.into())]).is_err(),
            "{bad:?}"
        );
    }
}
