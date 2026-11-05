//! File-level fixtures against the reference `scan_file_in` (`data/sm_manifest.txt`: name, path.`data/sm_<name>.bin` is
//! the file and `data/sm_<name>.exp` the rows: `K` keys in order, `P` pairs, or `ERR` for an unreadable file, `UNSUP`
//! for a reader that stays unsupported).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use nxb_model::sha256;
use nxb_split::SplitError;
use nxb_split::scanfile::Env;
use nxb_split::shaders::Profile;
use nxb_split::src::Mem;
use nxb_split::zstd::Dicts;

fn rows(r: Result<Option<nxb_split::records::FileResult>, SplitError>) -> String {
    match r {
        Err(SplitError::Unsupported(_)) => "UNSUP\n".into(),
        Err(_) => "ERR\n".into(),
        Ok(None) => String::new(),
        Ok(Some(r)) => {
            let mut s = String::new();
            for (_, f) in &r.keys {
                s += &format!(
                    "K\t{}\t{}\t{}\t{}\n",
                    f.place,
                    f.stage,
                    &sha256(&f.ctl).to_hex()[..16],
                    &sha256(&f.code).to_hex()[..16]
                );
            }
            for (v, p, prog) in &r.pairs {
                s += &format!("P\t{}\t{}\t{}\n", &v[..], &p[..], prog);
            }
            s
        }
    }
}

#[test]
fn small_readers_match_the_reference() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/");
    let man = std::fs::read_to_string(format!("{dir}sm_manifest.txt")).unwrap();
    let mut bad = Vec::new();
    for line in man.lines().filter(|l| !l.is_empty()) {
        let (name, path) = line.split_once('\t').unwrap();
        let d = std::fs::read(format!("{dir}sm_{name}.bin")).unwrap();
        let want = std::fs::read_to_string(format!("{dir}sm_{name}.exp")).unwrap();
        let label = format!("/{path}");
        let r = nxb_split::scanfile::scan_file(
            &Profile::default(),
            &Mem(&d),
            &label,
            &label,
            &Env {
                dicts: &Dicts::new(),
                common: None,
                trees: None,
            },
            &mut Vec::new(),
        );
        let got = rows(r);
        if got != want {
            bad.push(format!("{name}:\n want {want:?}\n got  {got:?}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn xnb_decoders_match_the_reference() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/");
    for line in std::fs::read_to_string(format!("{dir}sm_xnb_dec.txt"))
        .unwrap()
        .lines()
    {
        let f: Vec<&str> = line.split('\t').collect();
        let d = std::fs::read(format!("{dir}sm_{}.bin", f[0])).unwrap();
        let x = nxb_split::xnb::xnb_decompress(&d, false).unwrap();
        assert_eq!(sha256(&x).to_hex(), f[1], "{}", f[0]);
        assert_eq!(x.len().to_string(), f[2]);
    }
}

#[test]
fn direct_readers_match_the_reference() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/");
    let man = std::fs::read_to_string(format!("{dir}sm_direct.txt")).unwrap();
    let mut bad = Vec::new();
    for line in man.lines().filter(|l| !l.is_empty()) {
        let (f, name) = line.split_once('\t').unwrap();
        let d = std::fs::read(format!("{dir}sm_{name}.bin")).unwrap();
        let want = std::fs::read_to_string(format!("{dir}sm_{name}.exp")).unwrap();
        let r = match f {
            "sdf" => nxb_split::capcom::sdf_shaders(&d, "/x"),
            "retro_main" => nxb_split::retro::retro_main_forms(&d, "/x"),
            "raw" => nxb_split::raw::raw_shaders(&d, "/x", &std::collections::HashSet::new()),
            other => panic!("unknown reader {other}"),
        };
        let got = match r {
            Err(_) => "ERR\n".to_string(),
            Ok(v) => v
                .iter()
                .map(|f| {
                    format!(
                        "K\t{}\t{}\t{}\t{}\n",
                        f.place,
                        f.stage,
                        &sha256(&f.ctl).to_hex()[..16],
                        &sha256(&f.code).to_hex()[..16]
                    )
                })
                .collect(),
        };
        if got != want {
            bad.push(format!("{name}:\n want {want:?}\n got  {got:?}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
