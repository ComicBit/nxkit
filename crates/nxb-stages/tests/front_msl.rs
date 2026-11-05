//! spirv-cross and the MSL passes against Python's final sources: from the translator's `.spv` of a kept Python work
//! folder (pixel and compute shaders, whose `.spv` Python does not rewrite) to `<key>_b.metal` of the same run.
//! `NXB_FRONT_WORK=… NXB_FRONT_OUT=<b-dir of that run> NXB_SPIRV_CROSS=<spirv-cross> NXB_TMP=<scratch>
//! cargo test -p nxb-stages --test front_msl -- --ignored`

use nxb_shader::msl::{common_passes, cross_args, fs_uint};
use std::path::PathBuf;
use std::process::Command;

fn var(n: &str) -> Option<PathBuf> {
    std::env::var_os(n).map(PathBuf::from)
}

#[test]
#[ignore = "needs spirv-cross and a kept Python work folder"]
fn msl_matches_python() {
    let (Some(w), Some(out), Some(sc), Some(tmp)) = (
        var("NXB_FRONT_WORK"),
        var("NXB_FRONT_OUT"),
        var("NXB_SPIRV_CROSS"),
        var("NXB_TMP"),
    ) else {
        return;
    };
    std::fs::create_dir_all(&tmp).unwrap();
    let mut stats = std::collections::BTreeMap::<String, (usize, usize)>::new();
    let mut bad = Vec::new();
    for e in std::fs::read_dir(&w).unwrap().flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        let Some(key) = n.strip_suffix(".spv") else {
            continue;
        };
        let refl = std::fs::read_to_string(out.join(format!("{key}_b.json"))).unwrap_or_default();
        let stage = nxb_pyjson::parse(&refl)
            .ok()
            .and_then(|v| v.get("stage").and_then(|s| s.as_str().map(str::to_string)))
            .unwrap_or_default();
        if stage != "ps" && stage != "cs" {
            continue;
        }
        let spv = std::fs::read(e.path()).unwrap();
        let msl_path = tmp.join(format!("{key}.metal"));
        let st = Command::new(&sc)
            .args(cross_args(&spv).unwrap())
            .arg(e.path())
            .arg("--output")
            .arg(&msl_path)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .status()
            .unwrap();
        assert!(st.success());
        let raw = std::fs::read_to_string(&msl_path).unwrap();
        let mut src = common_passes(&raw).unwrap();
        if stage == "ps"
            && let Some((fu, _)) = fs_uint(&src)
        {
            src = fu;
        }
        let want = std::fs::read_to_string(out.join(format!("{key}_b.metal"))).unwrap();
        let s = stats.entry(stage.clone()).or_default();
        if src == want {
            s.0 += 1;
        } else {
            s.1 += 1;
            if bad.len() < 6 {
                std::fs::write(tmp.join(format!("{key}.ours.metal")), &src).unwrap();
                bad.push(key.to_string());
            }
        }
    }
    eprintln!("msl (equal, different) by stage: {stats:?}; first different: {bad:?}");
    assert!(stats.values().all(|s| s.1 == 0));
}
