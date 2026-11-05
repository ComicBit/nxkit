//! Round trip of the Python chain's own JSON files: parse each `*.json` of a local `b-<V>/` folder and write it again with
//! `indent=1`, the bytes must equal the file (`json.dump(..., ensure_ascii=False, indent=1)`).
//! `NXB_BDIR=<b-dir> cargo test -p nxb-pyjson --test real_files -- --ignored`

#[test]
#[ignore = "needs a local output folder"]
fn round_trip_real_files() {
    let Some(dir) = std::env::var_os("NXB_BDIR") else {
        return;
    };
    let (mut ok, mut bad) = (0, Vec::new());
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "json") {
            continue;
        }
        let text = std::fs::read_to_string(&p).unwrap();
        let v = nxb_pyjson::parse(&text).unwrap();
        // `_b_build.json` is written without indent (`json.dump(res, f, ensure_ascii=False)`).
        if nxb_pyjson::dumps_indent(&v, 1) == text || nxb_pyjson::dumps(&v) == text {
            ok += 1;
        } else {
            bad.push(p.display().to_string());
        }
    }
    eprintln!("json round trip: {ok} equal, {} different", bad.len());
    for b in bad.iter().take(5) {
        eprintln!("{b}");
    }
    assert!(bad.is_empty());
}
