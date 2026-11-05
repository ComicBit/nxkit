//! `xlate-corpus <ref-jobs-dir> <out-dir>`: rebuild the translator job files of a reference job set with fresh code files.
//!
//! Environment: `NXB_PKG` (split package folder), `NXB_PROFILE` (game profile folder with `shader_map.tsv` and
//! `shader_facts.tsv`). For each `job*.json` in `<ref-jobs-dir>` it writes `<out-dir>/jobs/<same name>` and
//! `<out-dir>/bin/<key>.bin`, and checks that each new job object equals the reference one except for `code`.
//! With `--all` instead of `<ref-jobs-dir>` it writes every key of the map, 128 keys a job file.

use nxb_pyjson::{Value, dumps, parse};
use nxb_shader::job::{JobOptions, job_for};
use nxb_stages::inputs::ShaderInputs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn job(inputs: &ShaderInputs, key: &str, out: &Path) -> Result<Value, String> {
    let (pk, rec) = inputs.record(key).map_err(|e| e.to_string())?;
    let code = out.join("bin").join(format!("{key}.bin"));
    let j = job_for(
        key,
        &pk,
        &rec,
        inputs.facts.get(key),
        &code.to_string_lossy(),
        JobOptions::default(),
    )
    .map_err(|e| e.0)?;
    std::fs::write(&code, &j.code).map_err(|e| e.to_string())?;
    Ok(j.json)
}

fn run() -> Result<(), String> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let [src, out] = a.as_slice() else {
        return Err("usage: xlate-corpus <ref-jobs-dir>|--all <out-dir>".into());
    };
    let env = |n: &str| {
        std::env::var_os(n)
            .map(PathBuf::from)
            .ok_or(format!("{n} not set"))
    };
    let inputs =
        ShaderInputs::load(&env("NXB_PKG")?, &env("NXB_PROFILE")?).map_err(|e| e.to_string())?;
    let out = PathBuf::from(out);
    for d in ["jobs", "bin"] {
        std::fs::create_dir_all(out.join(d)).map_err(|e| e.to_string())?;
    }
    let (mut n, mut diff, mut fail) = (0usize, 0usize, 0usize);
    if src == "--all" {
        let keys: Vec<&String> = inputs.loc.keys().collect();
        for (i, chunk) in keys.chunks(128).enumerate() {
            let mut list = Vec::new();
            for k in chunk {
                match job(&inputs, k, &out) {
                    Ok(j) => list.push(j),
                    Err(e) => {
                        fail += 1;
                        eprintln!("{k}: {e}");
                    }
                }
                n += 1;
            }
            let p = out.join("jobs").join(format!("job_all_{i:04}.json"));
            std::fs::write(p, dumps(&Value::List(list))).map_err(|e| e.to_string())?;
        }
    } else {
        let mut files: Vec<PathBuf> = std::fs::read_dir(src)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        files.sort();
        for f in files {
            let text = std::fs::read_to_string(&f).map_err(|e| e.to_string())?;
            let refs = parse(&text).map_err(|e| format!("{}: {e:?}", f.display()))?;
            let mut list = Vec::new();
            for r in refs.as_list().unwrap_or(&[]) {
                let key = r
                    .get("key")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                n += 1;
                match job(&inputs, &key, &out) {
                    Ok(mut j) => {
                        // `attr` of a vertex re-translation comes from the MSL pass `int_inputs`: keep the reference's.
                        if let Some(at) = r.get("attr") {
                            j.set("attr", at.clone());
                        }
                        let mut a = j.clone();
                        let mut b = r.clone();
                        a.remove("code");
                        b.remove("code");
                        if dumps(&a) != dumps(&b) {
                            diff += 1;
                            eprintln!(
                                "{key}: job differs\n  new {}\n  ref {}",
                                dumps(&a),
                                dumps(&b)
                            );
                        }
                        list.push(j);
                    }
                    Err(e) => {
                        fail += 1;
                        eprintln!("{key}: {e}");
                    }
                }
            }
            let name = f.file_name().unwrap_or_default();
            std::fs::write(out.join("jobs").join(name), dumps(&Value::List(list)))
                .map_err(|e| e.to_string())?;
        }
    }
    println!("{n} jobs, {diff} differ from the reference, {fail} failed");
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xlate-corpus: {e}");
            ExitCode::FAILURE
        }
    }
}
