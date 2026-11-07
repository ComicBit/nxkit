//! What the options of the NSP script do in `nxb build`: the environment file with `--no-motion`, the partial library of
//! `--seen-shaders`, the Metal compiler check (`--allow-other-metal`), the codesign identity of `--mac-sign`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::Args;
use crate::events::Progress;
use crate::graph::Build;
use crate::host::Runner;

/// `~/` and relative paths resolved, as the script resolves a path it was given.
pub(crate) fn expand(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    let full = match s.strip_prefix("~/") {
        Some(rest) => PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest),
        None => p.to_path_buf(),
    };
    std::path::absolute(&full).unwrap_or(full)
}

/// The switches of an environment file (`env_file_pairs`): `KEY=VALUE` lines, `#` starts a comment.
pub(crate) fn env_pairs(path: &Path) -> Result<Vec<(String, String)>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        crate::exit::fail(
            2,
            format!("cannot read the environment file (--env-file): {e}"),
        )
    })?;
    let mut out: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        if k.trim().is_empty() || k.starts_with('#') {
            continue;
        }
        match out.iter_mut().find(|(n, _)| n == k.trim()) {
            Some(slot) => slot.1 = v.to_string(),
            None => out.push((k.trim().to_string(), v.to_string())),
        }
    }
    Ok(out)
}

/// The environment file the app bakes in (`build_env_file`): `--env-file` as it is, or, for `--no-motion`, its
/// switches plus `DM_MOTION=0` in `<work>/build_env.txt`.
pub(crate) fn env_file(a: &Args, work: &Path) -> Result<Option<PathBuf>, String> {
    let given = a.env_file.as_deref().map(expand);
    if let Some(g) = &given
        && !g.is_file()
    {
        return Err(crate::exit::fail(
            2,
            format!(
                "cannot read the environment file (--env-file): {}: no such file",
                g.display()
            ),
        ));
    }
    if a.motion.unwrap_or(true) {
        return Ok(given);
    }
    let mut pairs = match &given {
        Some(g) => env_pairs(g)?,
        None => Vec::new(),
    };
    match pairs.iter_mut().find(|(k, _)| k == "DM_MOTION") {
        Some(slot) => slot.1 = "0".into(),
        None => pairs.push(("DM_MOTION".into(), "0".into())),
    }
    let f = work.join("build_env.txt");
    std::fs::create_dir_all(work).map_err(|e| format!("{}: {e}", work.display()))?;
    let text: String = pairs.iter().map(|(k, v)| format!("{k}={v}\n")).collect();
    std::fs::write(&f, text).map_err(|e| format!("{}: {e}", f.display()))?;
    Ok(Some(f))
}

fn hex16_at(s: &str) -> Option<&str> {
    let h = s.get(..16)?;
    h.chars()
        .all(|c| matches!(c, '0'..='9' | 'a'..='f'))
        .then_some(h)
}

/// The shader keys a host log reports missing (`[RED] shader <key> (...) failed to load:` and `[RED] failed to load
/// shader <key>_b:`).
fn missing_in(text: &str, out: &mut BTreeSet<String>) {
    for line in text.lines() {
        let Some(i) = line.find("[RED] ") else {
            continue;
        };
        let rest = &line[i + 6..];
        if let Some(r) = rest.strip_prefix("shader ")
            && let Some(k) = hex16_at(r)
            && r[16..].starts_with(" (")
            && r.contains(") failed to load:")
        {
            out.insert(k.to_string());
        } else if let Some(r) = rest.strip_prefix("failed to load shader ")
            && let Some(k) = hex16_at(r)
            && r[16..].starts_with("_b:")
        {
            out.insert(k.to_string());
        }
    }
}

/// `--seen-shaders`: the keys of the partial Mac library (`seen_keys`): the profile's `shader_seen.tsv`, those of the
/// last partial build (`<work>/shader/partial-keys.txt`) and those the logs report missing, only keys of the shader
/// map. Writes the list and returns the arguments with it as the manifest of a partial build.
pub(crate) fn seen_shaders(a: &Args, b: &Build, p: &Progress) -> Result<Args, String> {
    let dir = &b.profile.0;
    let listed_text = std::fs::read_to_string(dir.join("shader_seen.tsv")).map_err(|e| {
        format!(
            "--seen-shaders needs the profile's shader_seen.tsv ({}): {e}",
            dir.join("shader_seen.tsv").display()
        )
    })?;
    let listed: BTreeSet<String> = listed_text
        .lines()
        .filter_map(|l| {
            hex16_at(l).filter(|_| {
                l.get(16..17)
                    .is_none_or(|c| !c.chars().all(char::is_alphanumeric))
            })
        })
        .map(str::to_string)
        .collect();
    let map = std::fs::read_to_string(dir.join("shader_map.tsv"))
        .map_err(|e| format!("{}: {e}", dir.join("shader_map.tsv").display()))?;
    let known: BTreeSet<String> = map
        .lines()
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            (c.len() > 4 && c[0].ends_with(".bin")).then(|| c[3].to_string())
        })
        .collect();
    let sh = b.work.join("shader");
    let done = sh.join("partial-keys.txt");
    let earlier: BTreeSet<String> = std::fs::read_to_string(&done)
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_string)
        .collect();
    let mut found = BTreeSet::new();
    let home = std::env::var("HOME").unwrap_or_default();
    let logs = PathBuf::from(home).join("Library/Logs/nxnative");
    for log in [
        b.work.join("run/host.log"),
        logs.join(format!("{}.log", b.title)),
        logs.join(format!("{}.prev.log", b.title)),
    ] {
        if let Ok(t) = std::fs::read_to_string(&log) {
            missing_in(&t, &mut found);
        }
    }
    let keys: BTreeSet<String> = listed
        .iter()
        .chain(&earlier)
        .chain(&found)
        .filter(|k| known.contains(*k))
        .cloned()
        .collect();
    if keys.is_empty() {
        return Err(format!(
            "none of the {} shaders in {} is in this version's shader map: build the whole map, without --seen-shaders",
            listed.len(),
            dir.join("shader_seen.tsv").display()
        ));
    }
    std::fs::create_dir_all(&sh).map_err(|e| format!("{}: {e}", sh.display()))?;
    let text: String = keys.iter().map(|k| format!("{k}\n")).collect();
    std::fs::write(&done, text).map_err(|e| format!("{}: {e}", done.display()))?;
    let extra = keys.iter().filter(|k| !listed.contains(*k)).count();
    p.say(&format!(
        "partial shader library: {} of {} shaders ({} from the profile's playtests{}); the rest will not draw, so areas the list does not cover draw incomplete until this runs again without --seen-shaders",
        keys.len(),
        known.len(),
        listed.iter().filter(|k| known.contains(*k)).count(),
        if extra > 0 {
            format!(", {extra} that earlier runs reported missing")
        } else {
            String::new()
        }
    ));
    let mut a2 = a.clone();
    a2.manifest = Some(done);
    a2.partial = true;
    Ok(a2)
}

/// `Apple metal version 32023.922 (metalfe-32023.922.4)` as numbers.
fn metal_key(s: &str) -> Option<(u32, u32, u32)> {
    let r = s.trim().strip_prefix("Apple metal version ")?;
    let (a, r) = r.split_once('.')?;
    let (b, r) = r.split_once(" (metalfe-")?;
    let r = r.strip_suffix(')')?;
    let c = r.rsplit('.').next()?;
    Some((a.parse().ok()?, b.parse().ok()?, c.parse().ok()?))
}

/// The compilers that stop the build: (lock key, tested, actual). A newer compiler of the recorded generation goes on.
fn metal_stops(found: &[(String, String, String)]) -> Vec<(String, String, String)> {
    found
        .iter()
        .filter(|(_, want, now)| match (metal_key(want), metal_key(now)) {
            (Some(w), Some(n)) => !(w.0 == n.0 && n > w),
            _ => true,
        })
        .cloned()
        .collect()
}

/// An NVN game's shaders are built only with the Metal compiler they were tested with (`dmsh_chain.lock`), unless
/// `--allow-other-metal` (`check_metal`): checked before the shader step.
pub(crate) fn check_metal(a: &Args, b: &Build, p: &Progress) -> Result<(), String> {
    if a.shaders.is_some() || crate::layer::gfx(b)? != crate::layer::Gfx::Nvn {
        return Ok(());
    }
    let lock = std::fs::read_to_string(b.engine.join("dm/gfx/tools/dmsh_chain.lock"))
        .map_err(|e| format!("dm/gfx/tools/dmsh_chain.lock: {e}"))?;
    let want = |k: &str| {
        lock.lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| l.split_once('\t'))
            .find(|(n, _)| *n == k)
            .map(|(_, v)| v.to_string())
    };
    let r = Runner::from_parent()?;
    let mut found = Vec::new();
    for sdk in ["macosx", "iphoneos"] {
        let now = r
            .stdout("xcrun", ["-sdk", sdk, "metal", "--version"], None)
            .ok()
            .and_then(|o| o.lines().next().map(str::to_string))
            .filter(|l| !l.is_empty());
        let (Some(now), Some(tested)) = (now, want(&format!("metal_{sdk}"))) else {
            continue;
        };
        if now != tested {
            found.push((format!("metal_{sdk}"), tested, now));
        }
    }
    let stops = metal_stops(&found);
    if stops.is_empty() {
        for (k, w, n) in found {
            p.say(&format!(
                "note: built with a newer Metal compiler than the shaders were tested with (untested): {k} {n} (tested: {w})"
            ));
        }
        return Ok(());
    }
    let list = stops
        .iter()
        .map(|(k, w, n)| format!("{k} {n} (tested: {w})"))
        .collect::<Vec<_>>()
        .join("; ");
    if a.allow_other_metal {
        p.say(&format!(
            "note: built with this Mac's compilers, not the ones the shaders were tested with (dmsh_chain.lock): {list}"
        ));
        return Ok(());
    }
    Err(format!(
        "this Mac's Metal compiler is not the one the shaders were tested with (dm/gfx/tools/dmsh_chain.lock): {list}. Untested compilers are unsupported. The Metal compiler is the selected Xcode's: select the tested Xcode (xcode-select -s) and download its Metal toolchain (xcodebuild -downloadComponent MetalToolchain), or pass --allow-other-metal to build with this one anyway"
    ))
}

/// `--mac-sign <identity>`: the inner host first, then the bundle (`codesign --force --sign`), without the hardened
/// runtime, as `pack_app.py` signs. Files macOS keeps beside a file on a volume without extended attributes (`._name`)
/// are removed first: `codesign` reads each as an unsigned code object.
pub(crate) fn sign_with_identity(app: &Path, identity: &str) -> Result<(), String> {
    fn strip(dir: &Path) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if e.file_name().to_string_lossy().starts_with("._") {
                let _ = std::fs::remove_file(&p);
            } else if p.is_dir() && !p.is_symlink() {
                strip(&p);
            }
        }
    }
    let r = Runner::from_parent()?;
    strip(app);
    for target in [app.join("Contents/MacOS/dmhost"), app.to_path_buf()] {
        r.run(
            "codesign",
            [
                "--force".into(),
                "--sign".into(),
                identity.into(),
                target.into_os_string(),
            ],
            None,
        )
        .map_err(|e| format!("signing with {identity}: {e}"))?;
    }
    strip(app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_file_with_no_motion() {
        let d = std::env::temp_dir().join(format!("nxb-envfile-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("e.txt");
        std::fs::write(&f, "# c\nA=1\n DM_MOTION=1 \nB = x y\nnoeq\n=v\n").unwrap();
        let mut a = Args {
            env_file: Some(f.clone()),
            ..Args::default()
        };
        // motion on: the file as given
        assert_eq!(env_file(&a, &d.join("w")).unwrap(), Some(f));
        a.motion = Some(false);
        let out = env_file(&a, &d.join("w")).unwrap().unwrap();
        assert_eq!(
            std::fs::read_to_string(out).unwrap(),
            "A=1\nDM_MOTION=0\nB= x y\n"
        );
        a.env_file = None;
        let out = env_file(&a, &d.join("w")).unwrap().unwrap();
        assert_eq!(std::fs::read_to_string(out).unwrap(), "DM_MOTION=0\n");
        a.env_file = Some(d.join("missing"));
        assert!(env_file(&a, &d.join("w")).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn missing_shaders_in_a_log() {
        let mut s = BTreeSet::new();
        missing_in(
            "x\n[RED] shader 0123456789abcdef (pixel) failed to load: no lib\n[RED] failed to load shader fedcba9876543210_b: gone\n[RED] shader zzzz failed to load:\n",
            &mut s,
        );
        assert_eq!(s.len(), 2);
        assert!(s.contains("0123456789abcdef") && s.contains("fedcba9876543210"));
    }

    #[test]
    fn newer_metal_of_the_same_generation_goes_on() {
        let t = "Apple metal version 32023.921 (metalfe-32023.921.6)".to_string();
        let newer = "Apple metal version 32023.922 (metalfe-32023.922.4)".to_string();
        let older = "Apple metal version 32023.900 (metalfe-32023.900.1)".to_string();
        let other = "Apple metal version 32024.1 (metalfe-32024.1.1)".to_string();
        let row = |n: &String| vec![("metal_macosx".to_string(), t.clone(), n.clone())];
        assert!(metal_stops(&row(&newer)).is_empty());
        assert_eq!(metal_stops(&row(&older)).len(), 1);
        assert_eq!(metal_stops(&row(&other)).len(), 1);
        assert_eq!(metal_stops(&row(&"garbage".to_string())).len(), 1);
    }
}
