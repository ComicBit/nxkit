//! `pso_offline.py` for the iOS flow: the pipeline archive of a game's shader pairs, per GPU architecture. The plan,
//! the script and the stamps are `nxb_archive::plan` (byte-identical to the script's), the archives are made by
//! `nxb_archive::build` (content-cut batches in a store), so the work folder has no `key.<arch>` files: "the same
//! archive" is the loader finding every base, as for the Mac stage.

use super::ipagen::pso_gpus;
use nxb_archive::plan;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// What `pso_offline.py --game G --msl DIR --out OUT --work WORK --gpu GPUS [--os-min V] --platform ios` is given.
pub struct Args<'a> {
    /// The profile's pair lists (`shader_pairs.tsv`, then `pso_pairs.tsv`).
    pub pairs: [PathBuf; 2],
    /// The profile's `depth_mode`.
    pub depth: &'a str,
    /// `b-<chain>`: the libraries and their reflections.
    pub msl: &'a Path,
    /// The folder the app bundles.
    pub out: &'a Path,
    /// Script and store.
    pub work: &'a Path,
    /// `--gpu`.
    pub gpus: &'a str,
    /// `--os-min`.
    pub os_min: Option<&'a str>,
}

fn sdk_major() -> Result<String, String> {
    let o = std::process::Command::new("/usr/bin/xcrun")
        .args(["-sdk", "iphoneos", "--show-sdk-version"])
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err("xcrun --show-sdk-version failed".into());
    }
    let v = String::from_utf8_lossy(&o.stdout);
    Ok(format!("{}.0", v.trim().split('.').next().unwrap_or("27")))
}

/// The script's `main` (not `--script-only`, not `--batch-size`: the batches are `nxb_archive`'s). Progress lines go
/// to `say`, as the script's stdout does.
pub fn run(a: &Args, say: &mut dyn FnMut(&str)) -> Result<(), String> {
    let red = |m: String| format!("[RED] pso_offline: {m}");
    if a.depth != "gl" && a.depth != "zero" {
        return Err(red(format!("depth_mode must be gl or zero: {}", a.depth)));
    }
    let chain = a
        .msl
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_prefix("b-"))
        .ok_or_else(|| {
            red(format!(
                "{} is not a b-<chain version> folder",
                a.msl.display()
            ))
        })?;
    let mut archs: Vec<String> = a
        .gpus
        .split(',')
        .filter(|x| !x.trim().is_empty())
        .map(plan::arch_name)
        .collect();
    archs.sort();
    archs.dedup();
    if archs.is_empty() {
        return Err(red("no GPU architecture (--gpu)".into()));
    }
    let text = plan::pair_text(&a.pairs);
    if text.is_empty() {
        let l: Vec<String> = a.pairs.iter().map(|p| p.display().to_string()).collect();
        return Err(red(format!("no pair list ({})", l.join(", "))));
    }
    let zero = a.depth == "zero";
    let t0 = Instant::now();
    let pairs = plan::read_pairs(&text);
    let p = plan::plan(a.msl, chain, &pairs, zero);
    let mk = |d: &Path| std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()));
    mk(a.work)?;
    mk(a.out)?;
    let js = a.work.join("bases.mtl4-json");
    let script = plan::script_text(a.msl, &p.bases);
    if std::fs::read_to_string(&js).ok().as_deref() != Some(&script) {
        std::fs::write(&js, &script).map_err(|e| e.to_string())?;
    }
    let names = [
        "reflection",
        "mesh",
        "stage inputs",
        "library",
        "layer",
        "primitive ID",
        "entry",
    ];
    let left: Vec<String> = names
        .iter()
        .zip(p.left)
        .map(|(k, v)| format!("{k} {v}"))
        .collect();
    say(&format!(
        "pso_offline: {} pairs: {} bases, {} left to the device (function constants), left out as the host leaves them: {} ({:.1} s)",
        pairs.len(),
        p.bases.len(),
        p.rest.len(),
        left.join(", "),
        t0.elapsed().as_secs_f64()
    ));
    if !pairs.is_empty() && p.bases.is_empty() && p.rest.is_empty() {
        return Err(red("no base to build".into()));
    }
    if pairs.is_empty() {
        say("pso_offline: no pairs (only a header): nothing to build, no archive");
    }
    if p.bases.is_empty() {
        archs.clear();
    }
    let osmin = match a.os_min {
        Some(v) => v.to_string(),
        None => sdk_major()?,
    };
    let mut toolchain = "not used (function constants only)".to_string();
    for arch in &archs {
        let dst = a.out.join(format!("{arch}.mtl4a"));
        let c = nxb_archive::Config::for_tools("ios", arch, &osmin, &js, &dst, a.work)?;
        toolchain = c.toolchain.clone();
        let t1 = Instant::now();
        say(&format!(
            "pso_offline: {arch}: metal-tt, {} bases (minutes) ...",
            p.bases.len()
        ));
        let r = nxb_archive::build(&c).map_err(|e| red(format!("{arch}: {e}")))?;
        say(&format!(
            "pso_offline: {arch}: {} MB in {:.0} s",
            r.size >> 20,
            t1.elapsed().as_secs_f64()
        ));
    }
    if let Ok(rd) = std::fs::read_dir(a.out) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            let stale = n
                .strip_suffix(".mtl4a")
                .is_some_and(|stem| !archs.iter().any(|x| x == stem));
            if stale {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let w = |n: &str, t: &str| std::fs::write(a.out.join(n), t).map_err(|e| e.to_string());
    w("prebuilt.tsv", &plan::stamp(chain, zero, &text))?;
    match plan::rest_tsv(&p.rest) {
        Some(t) => w("rest_pairs.tsv", &t)?,
        None => {
            let _ = std::fs::remove_file(a.out.join("rest_pairs.tsv"));
        }
    }
    w(
        "archive.tsv",
        &plan::archive_tsv(
            &archs,
            "ios",
            &osmin,
            &toolchain,
            p.bases.len(),
            p.rest.len(),
        ),
    )
}

/// The pipeline archive step of `build_ipa.sh`: the folder of the archive, or empty when there is none.
pub(crate) fn stage(
    i: &super::ipa::Inputs,
    g: &crate::host::profile::Game,
    gfx: &str,
    wmv: (&str, &str, &str),
    say: &mut dyn FnMut(&str),
) -> Result<String, String> {
    let mut pso = String::new();
    let gd = g.dir.display().to_string();
    let (w, msl, v) = wmv;
    let p4 = g.get_or("pso_mtl4", "off");
    let pg = g.get_or("pso_gpu", "");
    let (gpus, warn) = pso_gpus(&i.pso_gpus, &p4, &pg);
    if let Some(wn) = warn {
        say(&format!("profile {gd}: {wn}"));
    }
    let pairs = g.dir.join("shader_pairs.tsv").is_file();
    if gfx == "nvn" && !gpus.is_empty() && gpus != "none" && pairs {
        pso = format!("{w}/build/ios/pso/bundle");
        say(&format!(
            "pipeline archive for {gpus} (metal-tt; minutes when its inputs changed)"
        ));
        let work_pso = format!("{w}/build/ios/pso/work");
        let depth = g.get_or("depth_mode", "gl");
        let (msl_dir, out_dir, work_dir) = (
            PathBuf::from(format!("{msl}/b-{v}")),
            PathBuf::from(&pso),
            PathBuf::from(&work_pso),
        );
        let args = Args {
            pairs: [g.table("shader_pairs.tsv"), g.table("pso_pairs.tsv")],
            depth: &depth,
            msl: &msl_dir,
            out: &out_dir,
            work: &work_dir,
            gpus: &gpus,
            os_min: i.pso_ios_min.as_deref().filter(|x| !x.is_empty()),
        };
        match run(&args, &mut *say) {
            Ok(()) => {}
            Err(e) => {
                say(&e);
                say(
                    "[RED] pipeline archive: pso_offline.py failed (its output is above); the app is built without one and the device builds its pipeline bases on its first start (DM_PSO_GPUS=none skips this step)",
                );
                super::script::rm_rf(Path::new(&pso))?;
                pso.clear();
            }
        }
    } else if gfx == "nvn" {
        let why = if i.pso_gpus.is_some() {
            format!(
                "DM_PSO_GPUS={}",
                if gpus.is_empty() { "empty" } else { &gpus }
            )
        } else {
            "the profile does not set pso_mtl4 on; DM_PSO_GPUS=g18p builds one".into()
        };
        say(&format!(
            "no pipeline archive ({why}{})",
            if pairs { "" } else { ", no shader_pairs.tsv" }
        ));
    }
    Ok(pso)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Oracle: `NXB_ORACLE_ENGINE`, `NXB_ORACLE_PYTHON`, `NXB_ORACLE_PSO_MSL` (a `b-<chain>` folder) and
    /// `NXB_ORACLE_PSO_PAIRS` (a pair list, a few dozen lines are enough): the script's iOS run and [`run`] with the
    /// same inputs. `prebuilt.tsv`, `archive.tsv`, `rest_pairs.tsv` and the script are byte-identical, the archive
    /// is compared by size class only (Apple's chain is not reproducible).
    #[test]
    fn archive_folder_equals_the_reference_script() {
        let (Some(e), Some(py), Some(msl), Some(pairs)) = (
            std::env::var_os("NXB_ORACLE_ENGINE"),
            std::env::var_os("NXB_ORACLE_PYTHON"),
            std::env::var_os("NXB_ORACLE_PSO_MSL"),
            std::env::var_os("NXB_ORACLE_PSO_PAIRS"),
        ) else {
            return;
        };
        let d = std::env::temp_dir().join(format!("nxb-ios-pso-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let o = std::process::Command::new(py)
            .arg(Path::new(&e).join("dm/gfx/tools/pso_offline.py"))
            .args(["--platform", "ios", "--os-min", "27.0", "--gpu", "g18p"])
            .arg("--msl")
            .arg(&msl)
            .arg("--pairs")
            .arg(&pairs)
            .arg("--out")
            .arg(d.join("py/out"))
            .arg("--work")
            .arg(d.join("py/work"))
            .output()
            .unwrap();
        assert!(o.status.success());
        let a = Args {
            pairs: [PathBuf::from(&pairs), PathBuf::from("/nonexistent")],
            depth: "gl",
            msl: Path::new(&msl),
            out: &d.join("rs/out"),
            work: &d.join("rs/work"),
            gpus: "g18p",
            os_min: Some("27.0"),
        };
        let mut lines = vec![];
        run(&a, &mut |l| lines.push(l.to_string())).unwrap();
        let first = String::from_utf8(o.stdout)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .split(" (")
            .next()
            .unwrap()
            .to_string();
        assert_eq!(lines[0].split(" (").next().unwrap(), first);
        for f in ["prebuilt.tsv", "archive.tsv"] {
            assert_eq!(
                std::fs::read(d.join("rs/out").join(f)).unwrap(),
                std::fs::read(d.join("py/out").join(f)).unwrap(),
                "{f}"
            );
        }
        assert_eq!(
            std::fs::read(d.join("rs/work/bases.mtl4-json")).unwrap(),
            std::fs::read(d.join("py/work/bases.mtl4-json")).unwrap()
        );
        let (x, y) = (
            std::fs::metadata(d.join("rs/out/applegpu_g18p.mtl4a"))
                .unwrap()
                .len(),
            std::fs::metadata(d.join("py/out/applegpu_g18p.mtl4a"))
                .unwrap()
                .len(),
        );
        assert!(x > y / 2 && x < y * 2, "{x} vs {y}");
        let _ = std::fs::remove_dir_all(&d);
    }
}
