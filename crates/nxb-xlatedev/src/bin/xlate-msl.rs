//! `xlate-msl <out-dir> <key>...`: translate shaders of a split package and write each library's MSL.
//!
//! Environment: `NXB_PKG` (split package folder), `NXB_PROFILE` (folder with `shader_map.tsv` and `shader_facts.tsv`).
//! Each library goes to `<out-dir>/<key><suffix>.metal`. Paired vertex shaders get no partner inputs.

use nxb_pyjson::Value;
use nxb_shader::job::{JobOptions, job_for};
use nxb_stages::inputs::ShaderInputs;
use nxb_xlate::job::Job;
use nxb_xlate::msl::{Context, translate};
use std::path::PathBuf;
use std::process::ExitCode;

fn one(inputs: &ShaderInputs, key: &str, out: &std::path::Path) -> Result<usize, String> {
    let (pk, rec) = inputs.record(key).map_err(|e| e.to_string())?;
    let j = job_for(
        key,
        &pk,
        &rec,
        inputs.facts.get(key),
        &format!("{key}.bin"),
        JobOptions::default(),
    )
    .map_err(|e| e.0)?;
    let xj = Job::from_value(&j.json)?;
    let (name, index) = inputs.loc.get(key).cloned().unwrap_or_default();
    let pc = inputs.pc_of.get(key).cloned().unwrap_or_default();
    let ctx = Context {
        chain: "dev",
        source: format!("{name}#{index}"),
        pc_paired: &pc,
        partners: &[],
        no_facts: j.json.get("no_facts") == Some(&Value::Bool(true)),
        dual_source: inputs.dual_want.contains(key),
        fetch_outputs_only: inputs.bf_want.contains(key),
    };
    let t = translate(&xj, &j.code, &ctx).map_err(|f| format!("{}: {}", f.reason, f.detail))?;
    for l in &t.libraries {
        std::fs::write(out.join(format!("{key}{}.metal", l.suffix)), &l.msl)
            .map_err(|e| e.to_string())?;
    }
    Ok(t.libraries.len())
}

fn run() -> Result<(), String> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let Some((out, keys)) = a.split_first() else {
        return Err("usage: xlate-msl <out-dir> <key>...".into());
    };
    let env = |n: &str| {
        std::env::var_os(n)
            .map(PathBuf::from)
            .ok_or(format!("{n} not set"))
    };
    let inputs =
        ShaderInputs::load(&env("NXB_PKG")?, &env("NXB_PROFILE")?).map_err(|e| e.to_string())?;
    let out = PathBuf::from(out);
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let mut fail = 0;
    for k in keys {
        match one(&inputs, k, &out) {
            Ok(n) => println!("{k}: {n} libraries"),
            Err(e) => {
                fail += 1;
                eprintln!("{k}: {e}");
            }
        }
    }
    if fail > 0 {
        return Err(format!("{fail} failed"));
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xlate-msl: {e}");
            ExitCode::FAILURE
        }
    }
}
