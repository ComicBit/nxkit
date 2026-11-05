//! The reflection fields `alpha` and `logic` that `dmsh_build.ps_libraries` writes for a pixel shader with side entry
//! points (`{'entries': [...]}` for each non-empty list), checked against every `_b.json` of a local output folder:
//! `NXB_SIDE_BDIR=<b-dir> cargo test -p nxb-shader --test side_fields -- --ignored`.

use nxb_pyjson::{Value, dumps};
use nxb_shader::side::unit;

#[test]
#[ignore = "needs a local output folder"]
fn alpha_logic_fields_match() {
    let Some(dir) = std::env::var_os("NXB_SIDE_BDIR") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let (mut ok, mut bad) = (0usize, Vec::new());
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(key) = name.strip_suffix("_b.metal") else {
            continue;
        };
        let msl = std::fs::read_to_string(e.path()).unwrap();
        let refl =
            nxb_pyjson::parse(&std::fs::read_to_string(dir.join(format!("{key}_b.json"))).unwrap())
                .unwrap();
        // Python makes the fields only when the split library built (the `_bx` exists).
        let split = dir.join(format!("{key}_bx.metallib")).exists();
        let (want_a, want_l) = match unit(&msl) {
            Ok(u) if split => {
                let f = |v: &[String]| {
                    (!v.is_empty()).then(|| {
                        dumps(&Value::obj([(
                            "entries",
                            Value::List(v.iter().map(|s| Value::str(s.clone())).collect()),
                        )]))
                    })
                };
                (f(&u.alpha), f(&u.logic))
            }
            _ => (None, None),
        };
        let got_a = refl.get("alpha").map(dumps);
        let got_l = refl.get("logic").map(dumps);
        if want_a == got_a && want_l == got_l {
            ok += 1;
        } else {
            bad.push(format!(
                "{key}: want {want_a:?} {want_l:?} got {got_a:?} {got_l:?}"
            ));
        }
    }
    eprintln!("alpha/logic fields: {ok} equal, {} different", bad.len());
    for b in bad.iter().take(5) {
        eprintln!("{b}");
    }
    assert!(bad.is_empty());
}
