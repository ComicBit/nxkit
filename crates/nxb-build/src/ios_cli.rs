//! `nxb-ios`: the engine's iOS scripts as one binary, with their command lines and environment unchanged.
//!
//! - `nxb-ios ipa [<out.ipa> | --prepare-xcode]` is `dm/ios/tools/build_ipa.sh` (environment `GAME`, `W`, `PATCHED`,
//!   `ROMFS`, `MSL_IOS`, `AOC`, `GFXAPI`, `DM_*`, as the script reads them).
//! - `nxb-ios shell --prepare-xcode | --build` is `dm/ios/tools/build_shell.sh` (`SHELL_DIR`, `DM_SHELL_FRAMEWORKS`, ...).
//!
//! The engine is `NXB_ENGINE` (the folder holding `dm/`), else the folder three levels above this binary's
//! `dm/ios/tools` when it is installed there. Step lines are printed as the scripts print them (`[HH:MM:SS] ...`).
//! Exit codes: 0 done, 1 failed, 2 bad arguments, 4 unsupported here (a game of several programs, a route C framework
//! build): the caller runs the script.

use crate::host::Runner;
use crate::ios::{ipa, shell};
use std::path::PathBuf;

fn var(k: &str) -> Option<String> {
    std::env::var(k).ok()
}

fn stamp(line: &str) {
    // local wall clock as `date +%T` prints it
    let t = std::process::Command::new("date")
        .arg("+%T")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    println!("[{t}] {line}");
}

fn engine() -> Option<PathBuf> {
    if let Some(e) = var("NXB_ENGINE") {
        return Some(PathBuf::from(e));
    }
    let exe = std::env::current_exe().ok()?;
    let e = exe.parent()?.parent()?.parent()?.parent()?.to_path_buf();
    e.join("dm/ios/project.yml").is_file().then_some(e)
}

fn ipa_inputs(engine: PathBuf) -> ipa::Inputs {
    ipa::Inputs {
        engine,
        game: var("GAME").unwrap_or_default(),
        w: var("W").unwrap_or_default(),
        patched: var("PATCHED"),
        romfs: var("ROMFS"),
        msl_ios: var("MSL_IOS"),
        guard_tables: None,
        aoc: var("AOC"),
        gfxapi: var("GFXAPI"),
        font_py: var("FONT_PY"),
        std_font: var("STD_FONT"),
        display_name: var("DM_DISPLAY_NAME"),
        app_version: var("DM_APP_VERSION"),
        bundle_id: var("DM_BUNDLE_ID"),
        team_id: var("DM_TEAM_ID"),
        lang: var("DM_LANG"),
        control: var("CONTROL"),
        xcode_project_dir: var("DM_XCODE_PROJECT_DIR"),
        xcode_dd: var("DM_XCODE_DERIVED_DATA"),
        copy_romfs: var("DM_COPY_ROMFS"),
        full_romfs: var("DM_FULL_ROMFS"),
        entitlements: var("DM_ENTITLEMENTS"),
        icon: var("DM_ICON"),
        env_file: var("DM_ENV_FILE"),
        show_fps: var("DM_SHOW_FPS"),
        metal_capture: var("DM_METAL_CAPTURE"),
        pso_gpus: var("DM_PSO_GPUS"),
        pso_ios_min: var("DM_PSO_IOS_MIN"),
        ios_pull: var("IOS_PULL"),
        v: var("V"),
        framework: var("DM_FRAMEWORK"),
        framework_only: var("DM_FRAMEWORK_ONLY"),
        programs_tsv: var("PROGRAMS_TSV"),
        games_dir: var("DM_GAMES_DIR"),
        games_first: var("DM_GAMES_FIRST"),
        user: var("USER").unwrap_or_default(),
    }
}

fn shell_inputs(engine: PathBuf) -> shell::Inputs {
    shell::Inputs {
        engine,
        shell_dir: var("SHELL_DIR").map(PathBuf::from).unwrap_or_default(),
        frameworks: var("DM_SHELL_FRAMEWORKS").unwrap_or_default(),
        bundle_id: var("DM_BUNDLE_ID"),
        team_id: var("DM_TEAM_ID"),
        shell_name: var("DM_SHELL_NAME"),
        app_version: var("DM_APP_VERSION"),
        package: var("DM_SHELL_PACKAGE").map(PathBuf::from),
        derived: var("DM_XCODE_DERIVED_DATA").map(PathBuf::from),
        std_font: var("STD_FONT").map(PathBuf::from),
        font_py: var("FONT_PY"),
        entitlements: var("DM_ENTITLEMENTS"),
    }
}

/// `nxb ios` / `nxb-ios`: returns the exit code.
pub fn main(args: &[String]) -> i32 {
    let a = args;
    let usage = "usage: nxb-ios ipa [<out.ipa> | --prepare-xcode] | nxb-ios shell --prepare-xcode | --build";
    let Some(e) = engine() else {
        eprintln!("[RED] nxb-ios: no engine (set NXB_ENGINE to the folder holding dm/)");
        return 2;
    };
    let r = match Runner::from_parent() {
        Ok(r) => r,
        Err(err) => {
            eprintln!("[RED] nxb-ios: {err}");
            return 1;
        }
    };
    let res = match a.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["ipa"] => ipa::run(&r, ipa::Target::Ipa(None), &ipa_inputs(e), &mut |l| {
            stamp(l)
        }),
        ["ipa", "--prepare-xcode"] => {
            ipa::run(&r, ipa::Target::Prepare, &ipa_inputs(e), &mut |l| stamp(l))
        }
        ["ipa", out] if !out.starts_with('-') => {
            let out = PathBuf::from(out);
            ipa::run(&r, ipa::Target::Ipa(Some(&out)), &ipa_inputs(e), &mut |l| {
                stamp(l)
            })
        }
        ["shell", m @ ("--prepare-xcode" | "--build")] => {
            let mode = if m == "--build" {
                shell::Mode::Build
            } else {
                shell::Mode::Prepare
            };
            shell::run(&r, mode, &shell_inputs(e), &mut |l| stamp(l))
        }
        _ => {
            eprintln!("{usage}");
            return 2;
        }
    };
    match res {
        Ok(_) => 0,
        Err(m) if m.starts_with("unsupported") => {
            eprintln!("[RED] nxb-ios: {m}");
            4
        }
        Err(m) => {
            stamp(&m);
            1
        }
    }
}
