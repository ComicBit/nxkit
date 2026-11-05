//! The real translator (`dmsh_xlate`) through `ProcessTranslator`, against the Python work folder (oracle):
//! `NXB_FRONT_WORK=… NXB_PKG=… NXB_PROFILE=… NXB_XLATE=<dmsh_xlate> NXB_DOTNET_ROOT=<dir> NXB_TMP=<scratch>
//! cargo test -p nxb-stages --test front_xlate -- --ignored`. Vertex shaders are left out: Python rewrites their `.spv`
//! after translation (`spv_single_lane`) and may translate them again (`int_inputs`).

use nxb_pyjson::{Value, dumps};
use nxb_shader::job::{JobOptions, job_for};
use nxb_stages::inputs::ShaderInputs;
use nxb_tool::{
    EnvPolicy, ProcessRegistry, ProcessTranslator, ToolId, Translator, XlateConfig, XlateJob,
};
use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::Arc;

fn var(n: &str) -> Option<PathBuf> {
    std::env::var_os(n).map(PathBuf::from)
}

#[test]
#[ignore = "needs the translator, a kept Python work folder and local game data"]
fn translations_match_python() {
    let (Some(w), Some(pkg), Some(prof), Some(xl), Some(dn), Some(tmp)) = (
        var("NXB_FRONT_WORK"),
        var("NXB_PKG"),
        var("NXB_PROFILE"),
        var("NXB_XLATE"),
        var("NXB_DOTNET_ROOT"),
        var("NXB_TMP"),
    ) else {
        return;
    };
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let inputs = ShaderInputs::load(&pkg, &prof).unwrap();
    let mut keys = Vec::new();
    for e in std::fs::read_dir(&w).unwrap().flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if let Some(k) = n.strip_suffix(".bin") {
            keys.push(k.to_string());
        }
    }
    keys.sort();
    let mut jobs = Vec::new();
    for k in &keys {
        let (pk, rec) = inputs.record(k).unwrap();
        if pk.starts_with("vs") {
            continue;
        }
        let code_path = tmp.join(format!("{k}.bin"));
        let j = job_for(
            k,
            &pk,
            &rec,
            inputs.facts.get(k),
            &code_path.to_string_lossy(),
            JobOptions::default(),
        )
        .unwrap();
        std::fs::write(&code_path, &j.code).unwrap();
        jobs.push(XlateJob {
            key: k.clone(),
            job_json: dumps(&j.json),
        });
    }
    let home = var("HOME").unwrap_or_else(|| "/".into());
    let env = EnvPolicy::translator(&home, &tmp, Some(dn.as_os_str()), None, None)
        .unwrap()
        .set("PATH", format!("{}:/usr/bin:/bin", dn.display()))
        .unwrap();
    let t = ProcessTranslator::new(
        ToolId {
            name: "dmsh_xlate".into(),
            path: xl,
            version: String::new(),
        },
        Arc::new(ProcessRegistry::new()),
        XlateConfig::new(tmp.clone(), env),
    );
    let start = std::time::Instant::now();
    // Batches of 32, as the design's default.
    let mut results = Vec::new();
    for chunk in jobs.chunks(32) {
        results.extend(t.translate(chunk).unwrap());
    }
    let wall = start.elapsed();
    let (mut ok, mut bad) = (0usize, Vec::new());
    for r in &results {
        let ws = std::fs::read(w.join(format!("{}.spv", r.key))).ok();
        let wr = std::fs::read(w.join(format!("{}.res.json", r.key))).ok();
        let same = match &r.outcome {
            Ok(o) => {
                ws.as_deref() == Some(&o.spirv[..]) && wr.as_deref() == Some(&o.resources_json[..])
            }
            Err(_) => ws.is_none(),
        };
        if same {
            ok += 1
        } else {
            bad.push(r.key.clone())
        }
    }
    eprintln!(
        "translations: {ok} equal, {} different, {} runs, {:.1} s for {} jobs",
        bad.len(),
        t.runs(),
        wall.as_secs_f64(),
        jobs.len()
    );
    eprintln!("different: {:?}", &bad[..bad.len().min(5)]);
    let _ = (OsStr::new(""), Value::Null);
    assert!(ok > 0 && bad.is_empty());
}
