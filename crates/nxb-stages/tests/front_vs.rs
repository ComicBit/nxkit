//! The vertex path against a kept Python run: real translator, spirv-cross, passes, entry points, reflection.
//! Env as `front_xlate.rs` plus `NXB_FRONT_OUT`, `NXB_SPIRV_CROSS`.

use nxb_pyjson::{Value, dumps, dumps_indent};
use nxb_shader::job::{JobOptions, job_for};
use nxb_shader::target::MAC;
use nxb_stages::inputs::ShaderInputs;
use nxb_stages::vertex::vertex;
use nxb_tool::{EnvPolicy, ProcessRegistry, ProcessTranslator, ToolId, XlateConfig};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

fn var(n: &str) -> Option<PathBuf> {
    std::env::var_os(n).map(PathBuf::from)
}

#[test]
#[ignore = "needs the translator, spirv-cross, a kept Python work folder and local game data"]
fn vertex_matches_python() {
    let (Some(w), Some(out), Some(sc), Some(tmp), Some(pkg), Some(prof), Some(xl), Some(dn)) = (
        var("NXB_FRONT_WORK"),
        var("NXB_FRONT_OUT"),
        var("NXB_SPIRV_CROSS"),
        var("NXB_TMP"),
        var("NXB_PKG"),
        var("NXB_PROFILE"),
        var("NXB_XLATE"),
        var("NXB_DOTNET_ROOT"),
    ) else {
        return;
    };
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let inputs = ShaderInputs::load(&pkg, &prof).unwrap();
    let mut pairs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for t in ["shader_pairs.tsv", "pso_pairs.tsv"] {
        let p = if prof.join(t).exists() {
            prof.join(t)
        } else {
            pkg.parent().unwrap().join(t)
        };
        for l in std::fs::read_to_string(p).unwrap_or_default().lines() {
            if !l.trim().is_empty()
                && !l.starts_with('#')
                && let Some((v, ps)) = l.split_once(char::is_whitespace)
            {
                pairs
                    .entry(v.to_string())
                    .or_default()
                    .insert(ps.trim().to_string());
            }
        }
    }
    let home = var("HOME").unwrap_or_else(|| "/".into());
    let env = EnvPolicy::translator(&home, &tmp, Some(dn.as_os_str()), None, None)
        .unwrap()
        .set("PATH", format!("{}:/usr/bin:/bin", dn.display()))
        .unwrap();
    let tr = ProcessTranslator::new(
        ToolId {
            name: "dmsh_xlate".into(),
            path: xl,
            version: String::new(),
        },
        Arc::new(ProcessRegistry::new()),
        XlateConfig::new(tmp.clone(), env),
    );
    let n = std::sync::atomic::AtomicU64::new(0);
    let cross = |spv: &[u8], args: &[String]| -> Result<String, String> {
        let k = n.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (sp, mp) = (
            tmp.join(format!("c{k}.spv")),
            tmp.join(format!("c{k}.metal")),
        );
        std::fs::write(&sp, spv).map_err(|e| e.to_string())?;
        let st = Command::new(&sc)
            .args(args)
            .arg(&sp)
            .arg("--output")
            .arg(&mp)
            .status()
            .map_err(|e| e.to_string())?;
        if !st.success() {
            return Err("spirv-cross failed".into());
        }
        std::fs::read_to_string(&mp).map_err(|e| e.to_string())
    };
    let ident = std::fs::read_to_string(
        out.join(
            std::fs::read_dir(&out)
                .unwrap()
                .flatten()
                .find(|e| e.file_name().to_string_lossy().ends_with("_b.json"))
                .unwrap()
                .file_name(),
        ),
    )
    .ok()
    .and_then(|t| nxb_pyjson::parse(&t).ok())
    .and_then(|v| v.get("chain_compiler").cloned())
    .unwrap();
    let chain_of = |out: &std::path::Path| -> String {
        let f = std::fs::read_dir(out)
            .unwrap()
            .flatten()
            .find(|e| e.file_name().to_string_lossy().ends_with("_b.json"))
            .unwrap();
        let v = nxb_pyjson::parse(&std::fs::read_to_string(f.path()).unwrap()).unwrap();
        v.get("chain").and_then(|c| c.as_str()).unwrap().to_string()
    };
    let (mut ok_src, mut ok_refl, mut bad) = (0usize, 0usize, Vec::new());
    for e in std::fs::read_dir(&w).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(key) = name.strip_suffix(".bin") else {
            continue;
        };
        let (pk, rec) = inputs.record(key).unwrap();
        if !pk.starts_with("vs") {
            continue;
        }
        let code = tmp.join(format!("{key}.bin"));
        let job = job_for(
            key,
            &pk,
            &rec,
            inputs.facts.get(key),
            &code.to_string_lossy(),
            JobOptions::default(),
        )
        .unwrap();
        std::fs::write(&code, &job.code).unwrap();
        let pixels: Vec<String> = pairs
            .get(key)
            .map(|s| {
                s.iter()
                    .filter_map(|p| std::fs::read_to_string(out.join(format!("{p}_b.metal"))).ok())
                    .collect()
            })
            .unwrap_or_default();
        let (pname, k) = inputs.loc[key].clone();
        let r = vertex(
            key,
            &job.json,
            &tr,
            &tmp,
            &cross,
            &pixels,
            (
                &pname,
                k.to_string(),
                inputs.pc_of.get(key).map_or("", String::as_str),
                nxb_model::to_hex(&job.c1),
            ),
            &chain_of(&out),
            None,
        );
        let want_src = std::fs::read_to_string(out.join(format!("{key}_b.metal"))).unwrap();
        let want_json = std::fs::read_to_string(out.join(format!("{key}_b.json"))).unwrap();
        match r {
            Ok(mut o) => {
                o.refl.set("chain_metal", Value::str(MAC.record()));
                o.refl.set("chain_compiler", ident.clone());
                let s_ok = o.full == want_src;
                let j_ok = dumps_indent(&o.refl, 1) == want_json;
                ok_src += usize::from(s_ok);
                ok_refl += usize::from(j_ok);
                if !(s_ok && j_ok) {
                    if bad.len() < 4 {
                        std::fs::write(tmp.join(format!("{key}.ours.metal")), &o.full).unwrap();
                        std::fs::write(
                            tmp.join(format!("{key}.ours.json")),
                            dumps_indent(&o.refl, 1),
                        )
                        .unwrap();
                    }
                    bad.push(format!("{key} src {s_ok} json {j_ok}"));
                }
            }
            Err((r, d)) => bad.push(format!("{key}: {r}: {d}")),
        }
    }
    eprintln!(
        "vertex: sources {ok_src} equal, reflections {ok_refl} equal, problems {}: {:?}",
        bad.len(),
        &bad[..bad.len().min(6)]
    );
    let _ = dumps(&Value::Null);
    assert!(bad.is_empty());
}
