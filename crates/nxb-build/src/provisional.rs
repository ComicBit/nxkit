//! A game with no registered profile (or `--provisional`): the NSP script's survey, guess and provisional profile
//! (`survey`, `write_profile`, `refine_after_rewrite`), as stages of the build.
//!
//! The order is the script's: [`prepare`] makes the survey input (a package: the code modules and a RomFS manifest,
//! no game files, a NUP or dump: the images, then a listing of the RomFS image), surveys, guesses and writes the
//! profile into `<work>/profile/<title ID>/` (`--profile-out`: `<folder>/<title ID>/`) with `survey.md` in
//! `<work>/<title ID>_survey/`. The build then plans again with the profile in place: the extract stage runs the
//! checks that need the game on the Mac ([`runs_check`]) and the rewrite, [`refine`] sets `mem_used` from the
//! rewritten modules, and the split's tables go into the profile ([`take_tables`]), as the script does.
//! Nothing here starts the game: a heap check, a region check or a shader capture is refused with exit code 4 unless
//! `--no-heap-check` is given.

use std::path::{Path, PathBuf};

use crate::Args;
use crate::events::Progress;
use crate::graph::Build;
use crate::host::survey::{Survey, SurveyInput, survey};
use crate::host::survey_extract::manifest_of_image;
use crate::host::survey_profile::{
    EMPTY_TABLES, has_va, refine_after_rewrite, runs_needed, write_profile,
};
use crate::inputs::Source;

fn survey_dir(b: &Build) -> PathBuf {
    b.work.join(format!("{}_survey", b.title))
}

fn game_tsv(b: &Build) -> PathBuf {
    b.profile.0.join("game.tsv")
}

fn version_dir(b: &Build) -> PathBuf {
    b.work.join("extract").join(&b.package)
}

fn io(p: &Path, e: std::io::Error) -> String {
    format!("{}: {e}", p.display())
}

fn newest_update(b: &Build) -> Option<&(u32, PathBuf)> {
    b.updates.iter().max_by_key(|u| u.0)
}

/// The survey's input folder from the images of the version (`ExeFS` and `control.nacp` linked, the RomFS listed).
fn input_from_image(b: &Build, dst: &Path) -> Result<(), String> {
    let (src, tmp) = (version_dir(b), dst.with_extension("partial"));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| io(&tmp, e))?;
    for n in ["ExeFS", "control.nacp"] {
        if src.join(n).exists() {
            std::os::unix::fs::symlink(src.join(n), tmp.join(n)).map_err(|e| io(&tmp, e))?;
        }
    }
    manifest_of_image(&src.join("RomFS.img"), &tmp)?;
    let _ = std::fs::remove_dir_all(dst);
    std::fs::rename(&tmp, dst).map_err(|e| io(dst, e))
}

/// A package's code modules and RomFS manifest without game files (`nx_extract --manifest`, the newest update
/// merged over the base).
fn input_from_package(a: &Args, b: &Build, p: &Progress, dst: &Path) -> Result<(), String> {
    let tmp = dst.with_extension("partial");
    let _ = std::fs::remove_dir_all(&tmp);
    let (out_base, updates, base_outputs) = match newest_update(b) {
        Some((_, path)) => (tmp.join("base"), vec![(path.clone(), tmp.clone())], false),
        None => (tmp.clone(), Vec::new(), true),
    };
    let req = nxb_extract::Request {
        keys: crate::extract_stage::keys(a)?,
        base: b.base.clone(),
        out_base,
        updates,
        threads: a.threads,
        base_outputs,
        app: b.src.app_flag,
        program: (b.program != 0).then_some(b.src.app),
        allow_damaged: a.allow_damaged,
        manifest: true,
        ..nxb_extract::Request::default()
    };
    let s = nxb_extract::extract(&req).map_err(|e| crate::extract_stage::damaged(p, &e))?;
    s.closing.wait();
    let _ = std::fs::remove_dir_all(tmp.join("base"));
    let _ = std::fs::remove_dir_all(dst);
    std::fs::rename(&tmp, dst).map_err(|e| io(dst, e))
}

/// The survey of the game: `fresh` makes the input again, else an earlier one is used.
fn survey_of(a: &Args, b: &Build, p: &Progress, fresh: bool) -> Result<Survey, String> {
    let dir = survey_dir(b).join("extract");
    if fresh || !dir.join("romfs_manifest.tsv").is_file() {
        std::fs::create_dir_all(survey_dir(b)).map_err(|e| io(&survey_dir(b), e))?;
        if b.src.source == Source::Packages && !version_dir(b).join("RomFS.img").is_file() {
            input_from_package(a, b, p, &dir)?;
        } else {
            input_from_image(b, &dir)?;
        }
    }
    let name = |p: &Path| {
        p.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let mut lines = vec![if b.src.has_base {
        format!("- base: `{}`", name(&b.base))
    } else {
        "- base: none (the update is the whole game)".to_string()
    }];
    let newest = newest_update(b).map(|u| u.0);
    let mut ups: Vec<&(u32, PathBuf)> = b.updates.iter().collect();
    ups.sort_by_key(|u| u.0);
    for u in ups {
        lines.push(format!(
            "- update: `{}` (v{}){}",
            name(&u.1),
            u.0,
            if Some(u.0) == newest {
                " (used for this survey)"
            } else {
                ""
            }
        ));
    }
    for (_, _, path) in &b.src.dlc {
        lines.push(format!("- DLC: `{}`", name(path)));
    }
    survey(&SurveyInput {
        extract: dir,
        app: b.src.app,
        fallback_name: if b.src.source == Source::Dump {
            name(&b.base)
        } else {
            String::new()
        },
        package_lines: lines,
        update: newest.map(u64::from),
        dlc: !b.src.dlc.is_empty(),
        engine: b.engine.clone(),
        census: None,
    })
    .map_err(|e| e.to_string().trim_start_matches("[RED] ").to_string())
}

/// The survey, the guess and the provisional profile. `Ok(true)`: the build stops here (`--survey-only`).
pub(crate) fn prepare(a: &Args, b: &Build, p: &Progress) -> Result<bool, String> {
    let game = game_tsv(b);
    if game.is_file() && !a.survey_only {
        p.say(&format!(
            "no registered game profile for {}: reusing the provisional profile from the last run: {} (delete it to survey again)",
            b.title,
            game.display()
        ));
        return Ok(false);
    }
    p.say(&format!("no game profile for {}: surveying first", b.title));
    p.plan_more(2);
    p.stage("survey: RomFS manifest", "");
    let packages = b.src.source == Source::Packages;
    if !packages {
        crate::extract_stage::container(a, b, p)?;
    }
    p.end(
        if packages {
            "code modules and a RomFS manifest (no game files written)"
        } else {
            "images unpacked"
        },
        true,
    );
    p.stage("survey: guess", "");
    let s = survey_of(a, b, p, true)?;
    let report = survey_dir(b).join("survey.md");
    std::fs::write(&report, &s.report).map_err(|e| io(&report, e))?;
    for n in &s.notes {
        p.say(n);
    }
    p.say(&format!("survey: {}", report.display()));
    if a.survey_only {
        p.end("survey only: no provisional profile", true);
        return Ok(true);
    }
    let wrote = write_profile(&b.profile.0, b.src.app, &s, &report.to_string_lossy())?;
    p.say(&format!(
        "{}: {}",
        if wrote {
            "wrote a provisional game profile"
        } else {
            "reusing the provisional profile"
        },
        game.display()
    ));
    for k in ["modules", "mem_reserve", "op_mode"] {
        if let Some((v, why)) = s.guess.0.iter().find(|g| g.0 == k).map(|g| &g.1)
            && !why.is_empty()
        {
            p.say(&format!("  {k} {v}: {why}"));
        }
    }
    p.say("[RED] provisional game profile: values guessed from the survey, the game may not boot");
    p.end(
        &format!("provisional profile {}", b.profile.0.display()),
        true,
    );
    Ok(false)
}

/// The steps that start the game on the Mac (heap check, region check, shader captures) are not done here: a game
/// that needs one stops with exit code 4 (which captures it needs follows from the files of the RomFS image, as
/// `build_sources` of the reference script reads them), unless `--no-heap-check` says to go on with the guessed layout (the
/// captures are then left out).
pub(crate) fn runs_check(a: &Args, b: &Build, p: &Progress) -> Result<(), String> {
    let exefs = version_dir(b).join("ExeFS");
    if !exefs.is_dir() {
        return Ok(());
    }
    let text = std::fs::read_to_string(game_tsv(b)).map_err(|e| io(&game_tsv(b), e))?;
    let s = survey_of(a, b, p, false)?;
    let img = version_dir(b).join("RomFS.img");
    let rom = if img.is_file() {
        Some(crate::host::rom::Rom::open(&img)?)
    } else {
        None
    };
    let runs = runs_needed(&s, &exefs, a.no_heap_check, Some(&text), "", rom.as_ref())?;
    let boots: Vec<_> = runs.iter().filter(|r| r.step != "capture").collect();
    let caps: Vec<_> = runs.iter().filter(|r| r.step == "capture").collect();
    let list = |v: &[&crate::host::survey_profile::RunNeeded]| {
        v.iter()
            .map(|r| format!("{} ({})", r.step, r.why))
            .collect::<Vec<_>>()
            .join("; ")
    };
    if !boots.is_empty() || (!caps.is_empty() && !a.no_heap_check) {
        let all: Vec<_> = runs.iter().collect();
        return Err(crate::exit::fail(
            4,
            format!(
                "unsupported: the provisional profile of {} needs the game to run on the Mac: {}. nxb build does not start the game. Use --no-heap-check to go on with the guessed layout (a shader capture is then left out)",
                b.title,
                list(&all)
            ),
        ));
    }
    if !caps.is_empty() {
        p.say(&format!(
            "[RED] not done, the game is not started here: {}: the shaders it would capture are not in the app",
            list(&caps)
        ));
    }
    if a.no_heap_check {
        p.say("--no-heap-check: no heap check and no region check, the guessed heap and address layout stays");
    }
    Ok(())
}

/// After the rewrite: the tables a Mac host needs exist, `mem_used` follows the rewritten modules.
pub(crate) fn refine(b: &Build, p: &Progress) -> Result<(), String> {
    for (n, text) in EMPTY_TABLES {
        let f = b.profile.0.join(n);
        if !f.exists() {
            std::fs::write(&f, text).map_err(|e| io(&f, e))?;
        }
    }
    let text = std::fs::read_to_string(game_tsv(b)).map_err(|e| io(&game_tsv(b), e))?;
    let va = has_va(&version_dir(b).join("ExeFS"))?;
    let patched = b.work.join(format!("patched_{}", b.package));
    if let Some(t) = refine_after_rewrite(&text, va, &patched)? {
        std::fs::write(game_tsv(b), t).map_err(|e| io(&game_tsv(b), e))?;
        p.say("provisional profile: mem_used set from the rewritten modules");
    }
    Ok(())
}

/// A provisional profile takes the split's tables as they come.
pub(crate) fn take_tables(b: &Build) -> Result<(), String> {
    if !b.prov {
        return Ok(());
    }
    for n in [
        "shader_map.tsv",
        "shader_facts.tsv",
        "shader_pairs.tsv",
        "pso_pairs.tsv",
    ] {
        let src = b.work.join("shader").join(n);
        if src.is_file() {
            std::fs::copy(&src, b.profile.0.join(n)).map_err(|e| io(&src, e))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_of_the_split_go_into_a_provisional_profile_only() {
        let d = std::env::temp_dir().join(format!("nxb-prov-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("w/shader")).unwrap();
        std::fs::create_dir_all(d.join("p")).unwrap();
        std::fs::write(d.join("w/shader/shader_map.tsv"), "map\n").unwrap();
        std::fs::write(d.join("w/shader/pso_pairs.tsv"), "pso\n").unwrap();
        let mut b = Build::for_test(d.join("w"), d.join("p"));
        take_tables(&b).unwrap();
        assert_eq!(
            std::fs::read_to_string(d.join("p/shader_map.tsv")).unwrap(),
            "map\n"
        );
        assert!(!d.join("p/shader_facts.tsv").exists());
        std::fs::remove_file(d.join("p/shader_map.tsv")).unwrap();
        b.prov = false;
        take_tables(&b).unwrap();
        assert!(!d.join("p/shader_map.tsv").exists());
        let _ = std::fs::remove_dir_all(&d);
    }
}
