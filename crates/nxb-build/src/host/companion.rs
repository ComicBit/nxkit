//! The companion app (`Contents/MacOS/nxnative`): `pack_app.py`'s `build_app_binary`. `launcher.c` and the host's
//! `save_sync.c` compiled by `clang`, linked by `swiftc` with the settings window (`Launcher.swift` and the iOS
//! model and rows), for the host's deployment target. The toolchain writes the same bytes on every run (the
//! temporary folder's path does not reach the output), so the result is byte-identical to the reference script's.

use super::{Runner, TempDir};
use nxb_model::ContentKey;
use std::path::{Path, PathBuf};

/// The compilers and the SDK as `pack_app.py` finds them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toolchain {
    /// `xcrun --find clang`, resolved.
    pub clang: PathBuf,
    /// `xcrun --find swiftc`, not resolved (the driver acts by its name).
    pub swiftc: PathBuf,
    /// `xcrun --sdk macosx --show-sdk-path`, resolved.
    pub sdk: PathBuf,
}

fn first_line(s: &str) -> PathBuf {
    PathBuf::from(s.trim())
}

fn real(p: PathBuf) -> PathBuf {
    std::fs::canonicalize(&p).unwrap_or(p)
}

impl Toolchain {
    /// Find the tools through `xcrun`.
    pub fn find(r: &Runner) -> Result<Self, String> {
        let t = Toolchain {
            clang: real(first_line(&r.stdout("xcrun", ["--find", "clang"], None)?)),
            swiftc: first_line(&r.stdout("xcrun", ["--find", "swiftc"], None)?),
            sdk: real(first_line(&r.stdout(
                "xcrun",
                ["--sdk", "macosx", "--show-sdk-path"],
                None,
            )?)),
        };
        if !t.clang.is_file() || !t.swiftc.is_file() || !t.sdk.is_dir() {
            return Err(format!(
                "no compiler or SDK at {}, {}, {}",
                t.clang.display(),
                t.swiftc.display(),
                t.sdk.display()
            ));
        }
        Ok(t)
    }
}

/// The host's deployment target (`LC_BUILD_VERSION` `minos` in `otool -l`), else `14.0`.
pub fn min_os(r: &Runner, host: &Path) -> Result<String, String> {
    let text = r.stdout("otool", ["-l".as_ref(), host.as_os_str()], None)?;
    Ok(text
        .find("cmd LC_BUILD_VERSION")
        .and_then(|i| {
            let rest = &text[i..];
            let j = rest.find("minos ")?;
            let v = rest[j + 6..].split(char::is_whitespace).next()?;
            (!v.is_empty()).then(|| v.to_string())
        })
        .unwrap_or_else(|| "14.0".into()))
}

/// The Swift sources (the settings window with the iOS model and rows).
pub fn swift_sources(engine: &Path) -> [PathBuf; 3] {
    let dm = engine.join("dm");
    [
        dm.join("mac/Launcher.swift"),
        dm.join("ios/Sources/LauncherSettings.swift"),
        dm.join("ios/Sources/LauncherRow.swift"),
    ]
}

/// Every source file of the companion app.
pub fn sources(engine: &Path) -> Vec<PathBuf> {
    let dm = engine.join("dm");
    let mut v = vec![
        dm.join("mac/launcher.c"),
        dm.join("host/save_sync.c"),
        dm.join("host/save_sync.h"),
        dm.join("host/romfs_image.h"),
    ];
    v.extend(swift_sources(engine));
    v
}

/// Build the companion app into `out` for `minos` (a temporary folder beside `out` holds the objects).
pub fn build(
    r: &Runner,
    engine: &Path,
    out: &Path,
    minos: &str,
    t: &Toolchain,
) -> Result<(), String> {
    let parent = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let tmp = TempDir::new_in(parent)?;
    let dm = engine.join("dm");
    let objs = [
        tmp.path().join("launcher.o"),
        tmp.path().join("save_sync.o"),
    ];
    let clang = t.clang.to_string_lossy().into_owned();
    let swiftc = t.swiftc.to_string_lossy().into_owned();
    let min = format!("-mmacosx-version-min={minos}");
    for (obj, src) in objs
        .iter()
        .zip([dm.join("mac/launcher.c"), dm.join("host/save_sync.c")])
    {
        r.run(
            &clang,
            [
                "-isysroot".as_ref(),
                t.sdk.as_os_str(),
                "-arch".as_ref(),
                "arm64".as_ref(),
                min.as_ref(),
                "-O2".as_ref(),
                "-Wall".as_ref(),
                "-I".as_ref(),
                dm.join("host").as_os_str(),
                "-c".as_ref(),
                "-o".as_ref(),
                obj.as_os_str(),
                src.as_os_str(),
            ],
            None,
        )?;
    }
    let target = format!("arm64-apple-macos{minos}");
    let cache = tmp.path().join("module-cache");
    let mut args: Vec<std::ffi::OsString> = [
        "-sdk".into(),
        t.sdk.clone().into_os_string(),
        "-target".into(),
        target.into(),
        "-O".into(),
        "-swift-version".into(),
        "5".into(),
        "-parse-as-library".into(),
        "-module-cache-path".into(),
        cache.into_os_string(),
        "-o".into(),
        out.as_os_str().to_owned(),
    ]
    .into();
    args.extend(
        swift_sources(engine)
            .into_iter()
            .map(PathBuf::into_os_string),
    );
    args.extend(objs.iter().map(|o| o.as_os_str().to_owned()));
    for f in ["CoreFoundation", "AppKit", "SwiftUI"] {
        args.push("-framework".into());
        args.push(f.into());
    }
    r.run(&swiftc, args, None)?;
    Ok(())
}

/// The companion app at `out`, built when its inputs (sources, compilers and their versions, SDK, `minos`) differ from
/// the last build into `out` (`<out>.key`). Returns whether it was built.
pub fn ensure(r: &Runner, engine: &Path, out: &Path, host: &Path) -> Result<bool, String> {
    let minos = min_os(r, host)?;
    let t = Toolchain::find(r)?;
    let mut k = ContentKey::builder("nxb-build companion 1")
        .field_str("minos", &minos)
        .field_str("sdk", &t.sdk.to_string_lossy())
        .field_str("clang", &t.clang.to_string_lossy())
        .field_str("swiftc", &t.swiftc.to_string_lossy());
    for c in [&t.clang, &t.swiftc] {
        let v = r.stdout(&c.to_string_lossy(), ["--version"], None)?;
        k = k.field_str("version", &v);
    }
    for s in sources(engine) {
        let bytes = std::fs::read(&s).map_err(|e| format!("{}: {e}", s.display()))?;
        k = k
            .field_str("source", &s.to_string_lossy())
            .field("bytes", &bytes);
    }
    let key = format!("{}\n", k.finish().to_hex());
    let kf = PathBuf::from(format!("{}.key", out.display()));
    if out.is_file() && std::fs::read_to_string(&kf).is_ok_and(|s| s == key) {
        return Ok(false);
    }
    let _ = std::fs::remove_file(&kf);
    build(r, engine, out, &minos, &t)?;
    std::fs::write(&kf, key).map_err(|e| format!("{}: {e}", kf.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Oracle: `NXB_ORACLE_ENGINE` (the engine), `NXB_ORACLE_HOST` (a Mac host, for `minos`) and
    /// `NXB_ORACLE_COMPANION` (`pack_app.build_app_binary`'s output for them) give the same bytes.
    #[test]
    fn companion_equals_the_reference_script() {
        let (Some(engine), Some(host), Some(want)) = (
            std::env::var_os("NXB_ORACLE_ENGINE"),
            std::env::var_os("NXB_ORACLE_HOST"),
            std::env::var_os("NXB_ORACLE_COMPANION"),
        ) else {
            return;
        };
        let r = Runner::from_parent().unwrap();
        let d = std::env::temp_dir().join(format!("nxb-companion-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let out = d.join("nxnative");
        assert!(ensure(&r, Path::new(&engine), &out, Path::new(&host)).unwrap());
        assert!(std::fs::read(&out).unwrap() == std::fs::read(&want).unwrap());
        // unchanged inputs: reused
        assert!(!ensure(&r, Path::new(&engine), &out, Path::new(&host)).unwrap());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn min_os_of_a_binary() {
        let Ok(r) = Runner::from_parent() else { return };
        if r.which("otool").is_err() {
            return;
        }
        let v = min_os(&r, Path::new("/bin/ls")).unwrap();
        assert!(v.split('.').all(|p| p.parse::<u32>().is_ok()), "{v}");
        assert!(min_os(&r, Path::new("/nonexistent/nxb")).is_err());
    }
}
