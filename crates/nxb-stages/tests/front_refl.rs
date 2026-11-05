//! Pixel and compute shader reflection (`_b.json`) against a kept Python run: spirv-cross on its `.spv`, the passes,
//! the reflection, the side fields, `fetch`, and the compiler fields, written as `json.dump(indent=1)`.
//! Env as `front_msl.rs` plus `NXB_PKG`, `NXB_PROFILE`.

use nxb_pyjson::{Value, dumps_indent};
use nxb_shader::job::{JobOptions, job_for};
use nxb_shader::msl::{common_passes, cross_args, fs_uint};
use nxb_shader::{side, target::MAC, variants::fetch};
use nxb_stages::front::{PsInfo, ps_reflection};
use nxb_stages::inputs::ShaderInputs;
use std::path::PathBuf;
use std::process::Command;

fn var(n: &str) -> Option<PathBuf> {
    std::env::var_os(n).map(PathBuf::from)
}

/// `dmsh_mtlcache.compiler_id('macosx')` through xcrun, as Python computes it.
#[allow(clippy::unwrap_used, reason = "test helper")]
fn compiler_id() -> String {
    let run = |a: &[&str]| {
        String::from_utf8(
            Command::new("/usr/bin/xcrun")
                .args(["-sdk", "macosx"])
                .args(a)
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
    };
    let mut s = String::new();
    for t in ["metal", "metallib"] {
        for l in run(&[t, "--version"]).split_inclusive('\n') {
            if !l.starts_with("InstalledDir:") {
                s.push_str(l);
            }
        }
    }
    s + &run(&["--show-sdk-version"]) + &run(&["--show-sdk-build-version"])
}

#[test]
#[ignore = "needs spirv-cross, xcrun and a kept Python work folder"]
fn reflection_matches_python() {
    let (Some(w), Some(out), Some(sc), Some(tmp), Some(pkg), Some(prof)) = (
        var("NXB_FRONT_WORK"),
        var("NXB_FRONT_OUT"),
        var("NXB_SPIRV_CROSS"),
        var("NXB_TMP"),
        var("NXB_PKG"),
        var("NXB_PROFILE"),
    ) else {
        return;
    };
    std::fs::create_dir_all(&tmp).unwrap();
    let inputs = ShaderInputs::load(&pkg, &prof).unwrap();
    let mut id = b"dmsh_mtlcache 2\0".to_vec();
    id.extend_from_slice(MAC.record().as_bytes());
    id.push(0);
    id.extend_from_slice(compiler_id().as_bytes());
    let ident = nxb_model::sha256(&id).to_hex();
    let (mut ok, mut bad) = (0usize, Vec::new());
    for e in std::fs::read_dir(&w).unwrap().flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        let Some(key) = n.strip_suffix(".spv") else {
            continue;
        };
        let (pk, rec) = inputs.record(key).unwrap();
        let stage = &pk[..2];
        if stage != "ps" && stage != "cs" {
            continue;
        }
        let job = job_for(
            key,
            &pk,
            &rec,
            inputs.facts.get(key),
            "x",
            JobOptions::default(),
        )
        .unwrap();
        let spv = std::fs::read(e.path()).unwrap();
        let msl_path = tmp.join(format!("{key}.metal"));
        Command::new(&sc)
            .args(cross_args(&spv).unwrap())
            .arg(e.path())
            .arg("--output")
            .arg(&msl_path)
            .status()
            .unwrap();
        let src = common_passes(&std::fs::read_to_string(&msl_path).unwrap()).unwrap();
        let fu = if stage == "ps" { fs_uint(&src) } else { None };
        let res =
            nxb_pyjson::parse(&std::fs::read_to_string(w.join(format!("{key}.res.json"))).unwrap())
                .unwrap();
        let (name, k) = inputs.loc[key].clone();
        let info = PsInfo {
            key,
            chain: "0ad559cdbb28",
            source: format!("{name}#{k}"),
            pc_paired: inputs.pc_of.get(key).map_or("", String::as_str),
            stage,
            src: &src,
            fu: fu.as_ref().map(|f| f.1),
            res: &res,
            job: &job.json,
            c1_hex: nxb_model::to_hex(&job.c1),
            vs: None,
        };
        let mut refl = ps_reflection(&info).unwrap();
        if stage == "ps" {
            let full = fu.as_ref().map_or(src.clone(), |f| f.0.clone());
            // ps_libraries: the side fields when the unit built (Python's _bx exists).
            if let Ok(u) = side::unit(&full)
                && out.join(format!("{key}_bx.metallib")).exists()
            {
                for (f, v) in [("alpha", &u.alpha), ("logic", &u.logic)] {
                    if !v.is_empty() {
                        refl.set(
                            f,
                            Value::obj([(
                                "entries",
                                Value::List(v.iter().map(|s| Value::str(s.clone())).collect()),
                            )]),
                        );
                    }
                }
            }
            if let Ok((_, meta)) =
                fetch::variant(&src, refl.get("bindings").unwrap_or(&Value::Null), false)
                && out.join(format!("{key}_bf.metallib")).exists()
            {
                refl.set("fetch", meta);
            }
        }
        refl.set("chain_metal", Value::str(MAC.record()));
        refl.set("chain_compiler", Value::str(ident.clone()));
        let want = std::fs::read_to_string(out.join(format!("{key}_b.json"))).unwrap();
        let got = dumps_indent(&refl, 1);
        if got == want {
            ok += 1;
        } else {
            if bad.len() < 3 {
                std::fs::write(tmp.join(format!("{key}.ours.json")), &got).unwrap();
            }
            bad.push(key.to_string());
        }
    }
    eprintln!(
        "reflection: {ok} equal, {} different, first {:?}",
        bad.len(),
        &bad[..bad.len().min(5)]
    );
    assert!(ok > 0 && bad.is_empty());
}
