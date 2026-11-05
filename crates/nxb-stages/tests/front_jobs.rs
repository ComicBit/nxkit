//! Translator jobs against a Python work folder kept from a run of the engine's `dmsh_build.py` (oracle):
//! `NXB_FRONT_WORK=<dmsh_ work dir> NXB_PKG=<pkg dir> NXB_PROFILE=<profile dir> cargo test -p nxb-stages --test front_jobs -- --ignored`.
//! Every job of its `job_*.json` files must equal ours (the `code` path aside) and its `<key>.bin` our code bytes.

use nxb_pyjson::{Value, dumps};
use nxb_shader::job::{JobOptions, job_for};
use nxb_stages::inputs::ShaderInputs;
use std::path::PathBuf;

#[test]
#[ignore = "needs a kept Python work folder and local game data"]
fn jobs_match_python() {
    let (Some(w), Some(pkg), Some(prof)) = (
        std::env::var_os("NXB_FRONT_WORK"),
        std::env::var_os("NXB_PKG"),
        std::env::var_os("NXB_PROFILE"),
    ) else {
        return;
    };
    let w = PathBuf::from(w);
    let inputs = ShaderInputs::load(&PathBuf::from(pkg), &PathBuf::from(prof)).unwrap();
    let (mut ok, mut bad) = (0usize, Vec::new());
    for e in std::fs::read_dir(&w).unwrap().flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if !n.starts_with("job_") {
            continue;
        }
        let list = nxb_pyjson::parse(&std::fs::read_to_string(e.path()).unwrap()).unwrap();
        for want in list.as_list().unwrap() {
            let key = want.get("key").and_then(Value::as_str).unwrap().to_string();
            let path = want
                .get("code")
                .and_then(Value::as_str)
                .unwrap()
                .to_string();
            let (pkgname, rec) = inputs.record(&key).unwrap();
            let got = job_for(
                &key,
                &pkgname,
                &rec,
                inputs.facts.get(&key),
                &path,
                JobOptions::default(),
            )
            .unwrap();
            let code_ok = std::fs::read(&path).map(|c| c == got.code).unwrap_or(false);
            if dumps(&got.json) == dumps(want) && code_ok {
                ok += 1;
            } else {
                bad.push(format!(
                    "{key}: code {code_ok}\n want {}\n got  {}",
                    dumps(want),
                    dumps(&got.json)
                ));
            }
        }
    }
    eprintln!("jobs: {ok} equal, {} different", bad.len());
    for b in bad.iter().take(3) {
        eprintln!("{b}");
    }
    assert!(ok > 0 && bad.is_empty());
}
