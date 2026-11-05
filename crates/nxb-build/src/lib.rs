//! `nxb build`: the NSP build script (`scripts/build_ipa_from_nsp.py`) as one stage graph. Same command line (the
//! options it does not implement yet are refused by name, exit code 1 with a `[RED]` line), same console events
//! (`--progress json`: the front ends' NDJSON, [`events`]), same exit codes (0 done, 1 error, 2 bad arguments, 130
//! interrupted).
//!
//! The graph (`--platform mac-app`): extract + split (fused, one pass for all versions) -> shaders -> pipeline archive
//! (when `--gpu` names an architecture.`nxb-archive`, not run on a full game from here yet) -> app (stage + sign, an in-place update of the previous app).
//! Stages overlap where the graph allows ([`order`]): the Mac host from the start, the app's companion app, icon and
//! settings beside the shader step, the first archive's batches as their libraries appear (widening to every core when
//! the shader step ends), the console events stay those of a run one after another. Not overlapped yet: the shader
//! step with the extract (it reads the split's packages when they are all written) and the staging of the app's
//! non-shader files with the shader step. `--dry-run` prints the graph and the keys.
//!
//! The rewrite (`patch_nso.py`) is made by [`rewrite`] when the work folder has no `patched_<package>` and the engine
//! is the one the port was checked against (otherwise the work folder's output is taken, as before). Not ported yet,
//! taken from the work folder (the Python build's outputs) and named in the plan: the survey and profile steps. The
//! shader step is an existing library folder (`--shaders`, or the work folder's `shader/cache/b-<chain>`), running the
//! lead's `shaderbuild build` as a process from here is not wired yet.

pub mod events;
pub mod graph;
pub mod host;
pub mod ios;
mod iosbuild;
pub mod order;
pub mod pipeline;
pub mod rewrite;

use std::path::PathBuf;

/// The command line (the NSP script's names, extras: `--dry-run`, `--engine`, `--shaders`, `--companion-app`, `--threads`).
#[derive(Clone, Debug, Default)]
pub struct Args {
    /// The folder holding one game's NSPs.
    pub nsp_dir: PathBuf,
    /// `-k/--keys`.
    pub keys: Option<PathBuf>,
    /// `--title-id`.
    pub title_id: Option<String>,
    /// `-p/--package`.
    pub package: Option<String>,
    /// `-o/--out`.
    pub out: Option<PathBuf>,
    /// `--app-name`.
    pub app_name: Option<String>,
    /// `--bundle-id`.
    pub bundle_id: Option<String>,
    /// `--platform` (`mac-app` or `ios`).
    pub platform: String,
    /// `--mac-sign` (only ad hoc `-`).
    pub mac_sign: String,
    /// `-l/--language`.
    pub language: Option<String>,
    /// `--work-dir`.
    pub work_dir: Option<PathBuf>,
    /// `--gpu` (`none` or architectures).
    pub gpu: Option<String>,
    /// `--progress`.
    pub progress: String,
    /// `--dry-run`: print the graph and the keys.
    pub dry_run: bool,
    /// `--engine`: the engine folder (profiles under `dm/games`).
    pub engine: Option<PathBuf>,
    /// `--shaders`: an existing `b-<chain>` library folder.
    pub shaders: Option<PathBuf>,
    /// `--companion-app`: the companion app binary embedded next to the built app (`Contents/MacOS/nxnative`), default:
    /// built from the engine's sources by clang + swiftc (`host::companion`) into `<work>/app/nxnative`.
    pub companion: Option<PathBuf>,
    /// Worker threads.
    pub threads: usize,
    /// `--xlate`: the shader translator (default the engine's `dmsh_xlate_bin/dmsh_xlate`).
    pub xlate: Option<PathBuf>,
    /// `--xlate-host`: the translator host program and arguments, or `off` (default: found next to `--xlate`, or
    /// env `NXB_XLATE_HOST`).
    pub xlate_host: Option<String>,
    /// `--dotnet-root` (default `DOTNET_ROOT`, else `dotnet --list-runtimes`).
    pub dotnet_root: Option<PathBuf>,
    /// `--own-front on|off|<verdicts>`: the own front end (default on: the shipped own-identical verdicts.
    /// `NXB_OWN_FRONT=0` turns it off).
    pub own_front: String,
    /// `--also-ios`: the iOS shader libraries too, from the same translation (`<work>/shader/ios/b-<V>`, the NSP
    /// script's layout), in the same shader step.
    pub also_ios: bool,
    /// `--shader-manifest <file>`: the keys the title used, one per line. Their shaders are built first.
    pub manifest: Option<PathBuf>,
    /// `--partial`: build only the manifest's shaders and flag the output as partial (needs `--shader-manifest`).
    pub partial: bool,
}

/// `--companion-app` under its old name: compatibility alias for the reference build scripts.
const COMPAT_ALIAS: &str = "--launcher";
const VALUE: &[&str] = &[
    "-k",
    "--keys",
    "--title-id",
    "-p",
    "--package",
    "-o",
    "--out",
    "--app-name",
    "--bundle-id",
    "--platform",
    "--mac-sign",
    "-l",
    "--language",
    "--work-dir",
    "--gpu",
    "--progress",
    "--engine",
    "--shaders",
    "--companion-app",
    COMPAT_ALIAS,
    "--threads",
    "--xlate",
    "--xlate-host",
    "--dotnet-root",
    "--own-front",
    "--shader-manifest",
];
/// Options of the NSP script this port refuses by name (not implemented yet).
const NOT_YET: &[&str] = &[
    "--romfs",
    "--prepare-xcode",
    "--framework",
    "--shell-framework",
    "--team-id",
    "--xcode-workspace",
    "--env-file",
    "--seen-shaders",
    "--show-fps",
    "--motion",
    "--survey-only",
    "--program",
    "--allow-damaged",
    "--allow-shader-regressions",
    "--allow-other-metal",
    "--profile-out",
    "--provisional",
    "--no-heap-check",
    "--keep-extracted",
    "--no-keep-extracted",
];

/// Parses the command line.`Err((code, message))`.
pub fn parse(argv: &[String]) -> Result<Args, (i32, String)> {
    let mut a = Args {
        platform: "ios".into(),
        mac_sign: "-".into(),
        progress: "auto".into(),
        threads: 10,
        ..Args::default()
    };
    let mut i = 0;
    while i < argv.len() {
        let s = argv[i].as_str();
        let (k, inline) = match s.split_once('=') {
            Some((k, v)) if k.starts_with("--") => (k, Some(v.to_string())),
            _ => (s, None),
        };
        if NOT_YET.contains(&k) {
            return Err((
                1,
                format!("{k} is not implemented in nxb build yet (run the NSP script)"),
            ));
        }
        if k == "--also-ios" {
            a.also_ios = true;
            i += 1;
            continue;
        }
        if k == "--partial" {
            a.partial = true;
            i += 1;
            continue;
        }
        if k == "--dry-run" {
            a.dry_run = true;
            i += 1;
            continue;
        }
        if VALUE.contains(&k) {
            let v = match inline {
                Some(v) => v,
                None => {
                    i += 1;
                    argv.get(i)
                        .cloned()
                        .ok_or((2, format!("argument {k}: expected one argument")))?
                }
            };
            match k {
                "-k" | "--keys" => a.keys = Some(v.into()),
                "--title-id" => a.title_id = Some(v),
                "-p" | "--package" => a.package = Some(v),
                "-o" | "--out" => a.out = Some(v.into()),
                "--app-name" => a.app_name = Some(v),
                "--bundle-id" => a.bundle_id = Some(v),
                "--platform" => a.platform = v,
                "--mac-sign" => a.mac_sign = v,
                "-l" | "--language" => a.language = Some(v),
                "--work-dir" => a.work_dir = Some(v.into()),
                "--gpu" => a.gpu = Some(v),
                "--progress" => a.progress = v,
                "--engine" => a.engine = Some(v.into()),
                "--shaders" => a.shaders = Some(v.into()),
                "--xlate" => a.xlate = Some(v.into()),
                "--xlate-host" => a.xlate_host = Some(v),
                "--dotnet-root" => a.dotnet_root = Some(v.into()),
                "--own-front" => a.own_front = v,
                "--shader-manifest" => a.manifest = Some(v.into()),
                _ if k == "--companion-app" || k == COMPAT_ALIAS => a.companion = Some(v.into()),
                _ => a.threads = v.parse().map_err(|_| (2, format!("--threads: {v}")))?,
            }
        } else if s.starts_with('-') {
            return Err((2, format!("unrecognized arguments: {s}")));
        } else if a.nsp_dir.as_os_str().is_empty() {
            a.nsp_dir = s.into();
        } else {
            return Err((2, format!("unrecognized arguments: {s}")));
        }
        i += 1;
    }
    if a.platform != "mac-app" && a.platform != "ios" {
        return Err((
            1,
            format!(
                "--platform {}: only mac-app and ios are implemented in nxb build",
                a.platform
            ),
        ));
    }
    if a.mac_sign != "-" {
        return Err((
            1,
            "--mac-sign: only ad-hoc signing (-) is implemented".into(),
        ));
    }
    if a.partial && a.manifest.is_none() {
        return Err((2, "--partial needs --shader-manifest <file>".into()));
    }
    if a.nsp_dir.as_os_str().is_empty() {
        return Err((2, "the NSP folder is required".into()));
    }
    Ok(a)
}

/// `nxb build <args>`: returns the exit code.
pub fn main(argv: &[String]) -> i32 {
    let a = match parse(argv) {
        Ok(a) => a,
        Err((code, msg)) => {
            println!("[RED] {msg}");
            return code;
        }
    };
    let mode = match a.progress.as_str() {
        "json" => events::Mode::Json,
        "off" => events::Mode::Off,
        _ => events::Mode::Lines,
    };
    let prog = std::sync::Arc::new(events::Progress::new(mode, Box::new(std::io::stdout())));
    match graph::run(&a, &prog) {
        Ok(()) => 0,
        Err(e) => {
            prog.end("", false);
            prog.say(&format!("[RED] {e}"));
            // Unsupported by contract: the caller runs the reference script (docs/integration-requests.md)
            if e.starts_with("unsupported") { 4 } else { 1 }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn command_line_as_the_nsp_script() {
        let a = parse(&v(&[
            "dir",
            "--platform",
            "mac-app",
            "-k",
            "k.keys",
            "--gpu=none",
            "--progress",
            "json",
        ]));
        assert!(a.as_ref().is_ok_and(|a| a.keys.is_some()
            && a.gpu.as_deref() == Some("none")
            && a.progress == "json"));
        assert_eq!(
            parse(&v(&["dir", "--platform", "mac-app", "--romfs", "bundle"]))
                .err()
                .map(|e| e.0),
            Some(1)
        );
        assert_eq!(
            parse(&v(&["dir", "--platform", "mac-app", "--nope"]))
                .err()
                .map(|e| e.0),
            Some(2)
        );
        assert_eq!(
            parse(&v(&["dir", "--platform", "mac"])).err().map(|e| e.0),
            Some(1)
        );
        // manifest-first and partial
        let a = parse(&v(&["dir", "--shader-manifest", "m.txt", "--partial"])).unwrap();
        assert!(a.partial && a.manifest.as_deref() == Some(std::path::Path::new("m.txt")));
        assert_eq!(parse(&v(&["dir", "--partial"])).err().map(|e| e.0), Some(2));
        assert!(parse(&v(&["dir"])).is_ok_and(|a| !a.partial && a.manifest.is_none()));
        // the script's default platform is ios
        assert!(parse(&v(&["dir"])).is_ok_and(|a| a.platform == "ios"));
    }
}
