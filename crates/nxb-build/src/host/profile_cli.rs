//! `nxb profile guess|show|check|manifest`: the provisional profile of a game with no registered profile.
//!
//! ```text
//! nxb profile guess  --engine <engine> (--extract <dir> | --exefs <dir> --romfs <manifest|folder>) [--app <hex>]
//!                    [--name <text>] [--out <folder>] [--report <survey.md>] [--census <file>] [--no-heap-check] [--strict]
//! nxb profile show   <game.tsv | profile folder> [--toml]
//! nxb profile check  --engine <engine> <profile folder> (--extract <dir> | --exefs <dir> --romfs <manifest|folder>)
//! nxb profile manifest <RomFS.img> <out folder>
//! ```
//! Exit codes: 0 done, 1 failed, 2 usage, 4 `guess --strict` when a step that boots the game on the Mac is needed
//! (heap check, region check, shader capture: not ported, the reference script runs it).

use super::guess::{facts_of, guess};
use super::guess_dump::check_line;
use super::nsofacts::host_hooks;
use super::survey::{SurveyInput, survey};
use super::survey_extract::manifest_of_image;
use super::survey_profile::{runs_needed, write_profile};
use std::path::{Path, PathBuf};

/// The usage text.
pub const USAGE: &str = "usage: nxb profile guess --engine <engine> (--extract <dir> | --exefs <dir> --romfs <manifest|folder>) [--app <hex>] [--name <text>] [--out <folder>] [--report <survey.md>] [--census <file>] [--no-heap-check] [--strict]\n       nxb profile show <game.tsv | profile folder> [--toml]\n       nxb profile check --engine <engine> <profile folder> (--extract <dir> | --exefs <dir> --romfs <manifest|folder>)\n       nxb profile manifest <RomFS.img> <out folder>";

struct Args {
    flags: Vec<(String, String)>,
    switches: Vec<String>,
    rest: Vec<String>,
}

fn parse(a: &[String]) -> Args {
    let (mut flags, mut switches, mut rest) = (Vec::new(), Vec::new(), Vec::new());
    let mut i = 0;
    while i < a.len() {
        match a[i].as_str() {
            "--no-heap-check" | "--strict" | "--toml" => switches.push(a[i].clone()),
            s if s.starts_with("--") => {
                flags.push((s.to_string(), a.get(i + 1).cloned().unwrap_or_default()));
                i += 1;
            }
            s => rest.push(s.to_string()),
        }
        i += 1;
    }
    Args {
        flags,
        switches,
        rest,
    }
}

impl Args {
    fn get(&self, k: &str) -> Option<&str> {
        self.flags.iter().find(|f| f.0 == k).map(|f| f.1.as_str())
    }
    fn on(&self, k: &str) -> bool {
        self.switches.iter().any(|s| s == k)
    }
    fn engine(&self) -> Result<PathBuf, String> {
        self.get("--engine")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("NXB_ENGINE").map(PathBuf::from))
            .ok_or_else(|| "--engine <engine folder> (or NXB_ENGINE) is needed".to_string())
    }
    /// (ExeFS, RomFS manifest or folder).
    fn inputs(&self) -> Result<(PathBuf, PathBuf), String> {
        if let Some(d) = self.get("--extract") {
            return Ok((
                Path::new(d).join("ExeFS"),
                Path::new(d).join("romfs_manifest.tsv"),
            ));
        }
        match (self.get("--exefs"), self.get("--romfs")) {
            (Some(e), Some(r)) => Ok((e.into(), r.into())),
            _ => Err("--extract <dir> or --exefs <dir> --romfs <manifest|folder> is needed".into()),
        }
    }
}

/// `nxb profile ...`, returns the exit code.
pub fn main(args: &[String]) -> i32 {
    let Some((cmd, rest)) = args.split_first() else {
        println!("{USAGE}");
        return 2;
    };
    let a = parse(rest);
    let r = match cmd.as_str() {
        "guess" => guess_cmd(&a),
        "show" => show_cmd(&a),
        "check" => check_cmd(&a),
        "manifest" => match a.rest[..] {
            [ref img, ref out] => manifest_of_image(Path::new(img), Path::new(out)).map(|n| {
                println!("{n} files listed");
                0
            }),
            _ => Err(USAGE.to_string()),
        },
        "--help" | "help" => {
            println!("{USAGE}");
            Ok(0)
        }
        other => Err(format!("usage: unknown profile command {other:?}\n{USAGE}")),
    };
    match r {
        Ok(c) => c,
        Err(e) => {
            let usage = e.starts_with("usage") || e.contains("is needed");
            println!("{}", if usage { e } else { format!("[RED] {e}") });
            if usage { 2 } else { 1 }
        }
    }
}

fn guess_cmd(a: &Args) -> Result<i32, String> {
    let engine = a.engine()?;
    let (exefs, romfs) = a.inputs()?;
    let extract = exefs
        .parent()
        .ok_or("--exefs has no parent folder")?
        .to_path_buf();
    let f = facts_of(&exefs, &romfs)?;
    let app = match a.get("--app") {
        Some(h) => {
            u64::from_str_radix(h, 16).map_err(|_| format!("--app takes 16 hex digits: {h}"))?
        }
        None => f.program.ok_or("main.npdm has no program ID: pass --app")?,
    };
    let census = match a.get("--census") {
        Some(p) => Some(std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?),
        None => None,
    };
    let s = survey(&SurveyInput {
        extract,
        app,
        fallback_name: a.get("--name").unwrap_or("").to_string(),
        package_lines: Vec::new(),
        update: None,
        dlc: false,
        engine,
        census,
    })
    .map_err(|e| e.to_string())?;
    if let Some(r) = a.get("--report") {
        std::fs::write(r, &s.report).map_err(|e| format!("{r}: {e}"))?;
        println!("survey report: {r}");
    }
    for n in &s.notes {
        println!("{n}");
    }
    for (k, (v, why)) in &s.guess.0 {
        println!(
            "{k}\t{v}{}",
            if why.is_empty() {
                String::new()
            } else {
                format!("\t# {why}")
            }
        );
    }
    if let Some(out) = a.get("--out") {
        let pdir = Path::new(out).join(format!("{app:016X}"));
        let report = a.get("--report").unwrap_or("survey.md");
        println!(
            "{}",
            if write_profile(&pdir, app, &s, report)? {
                format!(
                    "wrote a provisional game profile: {}",
                    pdir.join("game.tsv").display()
                )
            } else {
                format!(
                    "reusing the provisional profile: {} (delete it to regenerate from the survey)",
                    pdir.join("game.tsv").display()
                )
            }
        );
    }
    let runs = runs_needed(&s, &exefs, a.on("--no-heap-check"), None, "")?;
    for r in &runs {
        println!("needs a run of the game ({}): {}", r.step, r.why);
    }
    Ok(if a.on("--strict") && !runs.is_empty() {
        4
    } else {
        0
    })
}

fn show_cmd(a: &Args) -> Result<i32, String> {
    let p = a.rest.first().ok_or(USAGE)?;
    let path = if Path::new(p).is_dir() {
        Path::new(p).join("game.tsv")
    } else {
        PathBuf::from(p)
    };
    let t = nxb_profile::tsv::TsvFile::load(&path).map_err(|e| e.to_string())?;
    if a.on("--toml") {
        print!("{}", nxb_profile::Profile::from_game_tsv(&t).to_toml());
    } else {
        for r in t.rows() {
            println!("{}\t{}", r.key, r.joined());
        }
    }
    Ok(0)
}

fn check_cmd(a: &Args) -> Result<i32, String> {
    let engine = a.engine()?;
    let dir = PathBuf::from(a.rest.first().ok_or(USAGE)?);
    let (exefs, romfs) = a.inputs()?;
    let g = nxb_profile::tsv::TsvFile::load(&dir.join("game.tsv")).map_err(|e| e.to_string())?;
    let id = std::fs::canonicalize(&dir)
        .ok()
        .and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let f = facts_of(&exefs, &romfs)?;
    let hooks = host_hooks(&engine.join("dm"), false);
    let (line, bad) = check_line(&id, &g, &f, &guess(&f, &hooks)?);
    println!("{line}");
    Ok(i32::from(bad))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn usage_and_show() {
        assert_eq!(main(&[]), 2);
        assert_eq!(main(&s(&["nope"])), 2);
        assert_eq!(main(&s(&["guess"])), 2);
        let d = std::env::temp_dir().join(format!("nxb-profile-cli-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("game.tsv"),
            "# c\ntitle_id\t0100000000010000\nname\tG\napp_pool\t0xCD500000\n",
        )
        .unwrap();
        let p = d.to_string_lossy().into_owned();
        assert_eq!(main(&s(&["show", &p])), 0);
        assert_eq!(main(&s(&["show", &p, "--toml"])), 0);
        assert_eq!(main(&s(&["show", "/nonexistent/game.tsv"])), 1);
        let _ = std::fs::remove_dir_all(&d);
    }
}
