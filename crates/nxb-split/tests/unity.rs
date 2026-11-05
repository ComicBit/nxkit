//! Unity readers against the reference (`unityfs.py`, the `unity_*` functions of `shader_split.py`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use nxb_model::sha256;

#[test]
fn lzma_blocks_match_python() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/");
    for line in std::fs::read_to_string(format!("{dir}un_lzma.txt"))
        .unwrap()
        .lines()
    {
        let f: Vec<&str> = line.split('\t').collect();
        let d = std::fs::read(format!("{dir}un_lzma_{}.bin", f[0])).unwrap();
        let n: usize = f[2].parse().unwrap();
        let out = nxb_split::unity::lzma::decompress(&d, n).unwrap();
        assert_eq!(sha256(&out).to_hex(), f[1], "{}", f[0]);
    }
}

/// The two passes of the split over a tree of files: first every file, then the files without type trees with the
/// trees of every bundle and the known layouts.
fn run_tree(dir: &str, main: &[u8]) -> String {
    use nxb_split::scanfile::{Env, scan_file};
    use nxb_split::shaders::Profile;
    use nxb_split::src::Mem;
    use nxb_split::unity::ser::{Trees, known_trees};
    use nxb_split::zstd::Dicts;
    let common = nxb_split::unity::bundle::common_strings(main).unwrap();
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort_by_key(|n| n.replace("__", "/"));
    let (prof, dicts) = (Profile::default(), Dicts::new());
    let mut results = Vec::new();
    let mut trees = Trees::new();
    for n in &names {
        let d = std::fs::read(format!("{dir}/{n}")).unwrap();
        let rel = n.replace("__", "/");
        let env = Env {
            dicts: &dicts,
            common: Some(&common),
            trees: None,
        };
        let r = scan_file(
            &prof,
            &Mem(&d),
            &format!("/{rel}"),
            &format!("/{rel}"),
            &env,
            &mut Vec::new(),
        );
        if let Ok(Some(r)) = &r {
            trees.extend(r.trees.clone());
        }
        results.push((rel, d, r));
    }
    let defer: Vec<usize> = (0..results.len())
        .filter(|&i| matches!(&results[i].2, Ok(Some(r)) if r.defer))
        .collect();
    if !defer.is_empty() {
        for (k, v) in known_trees().unwrap() {
            trees.entry(k).or_insert(v);
        }
        let mut again = Vec::new();
        for &i in &defer {
            let env = Env {
                dicts: &dicts,
                common: Some(&common),
                trees: Some(&trees),
            };
            let (rel, d, _) = &results[i];
            let r = scan_file(
                &prof,
                &Mem(d),
                &format!("/{rel}"),
                &format!("/{rel}"),
                &env,
                &mut Vec::new(),
            );
            if matches!(&r, Ok(Some(r)) if r.defer) {
                return "FATAL\n".into();
            }
            again.push((rel.clone(), d.clone(), r));
        }
        results.retain(|r| !matches!(&r.2, Ok(Some(x)) if x.defer));
        results.extend(again);
    }
    let mut out = String::new();
    for (rel, _, r) in results {
        out += &format!("F\t{rel}\n");
        match r {
            Err(_) => out += "E\n",
            Ok(None) => {}
            Ok(Some(r)) => {
                for (_, f) in &r.keys {
                    out += &format!(
                        "K\t{}\t{}\t{}\t{}\n",
                        f.place,
                        f.stage,
                        &sha256(&f.ctl).to_hex()[..16],
                        &sha256(&f.code).to_hex()[..16]
                    );
                }
                for (v, p, prog) in &r.pairs {
                    out += &format!("P\t{v}\t{p}\t{prog}\n");
                }
            }
        }
    }
    out
}

#[test]
fn unity_files_match_the_reference() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/");
    let main = std::fs::read(format!("{dir}un_main.bin")).unwrap();
    let mut cases: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| {
            e.unwrap().file_name().to_str().and_then(|n| {
                n.strip_prefix("un_")?
                    .strip_suffix(".exp")
                    .map(str::to_string)
            })
        })
        .collect();
    cases.sort();
    assert!(!cases.is_empty());
    let mut bad = Vec::new();
    for c in cases {
        let want = std::fs::read_to_string(format!("{dir}un_{c}.exp")).unwrap();
        let got = run_tree(&format!("{dir}un_{c}"), &main);
        if got != want {
            bad.push(format!("{c}:\n want {want}\n got  {got}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
