//! `shaderbuild recompile <b-dir> --store <dir> --work <dir> [--jobs N] [--every N] [--limit N] [--io files|pipes]
//! [--no-pch] [--no-side] [--no-variants] [--relaxed]
//! [--own-front <verdicts.jsonl> [--own-allow own-identical,own-validated]]`

use nxb_obs::{NdjsonSink, Sink};
use nxb_stages::recompile::{RecompileConfig, run};
use nxb_stages::recompile_summary::{summary, tsv};
use nxb_tool::Io;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

const USAGE: &str = "usage: shaderbuild recompile <b-dir> --store <dir> --work <dir> [--jobs N] [--every N] [--limit N] [--io files|pipes] [--no-pch] [--no-side] [--no-variants] [--relaxed] [--own-front <verdicts.jsonl> [--own-allow v1,v2]]";

fn parse(args: &[String]) -> Result<RecompileConfig, String> {
    let mut b_dir = None;
    let mut cfg = RecompileConfig {
        b_dir: PathBuf::new(),
        store_root: PathBuf::new(),
        work_dir: PathBuf::new(),
        jobs: 6,
        every: 1,
        limit: None,
        io: Io::Files,
        use_pch: true,
        side: true,
        variants: true,
        durability: nxb_store::Durability::Full,
        own_front: None,
    };
    let mut allow = vec!["own-identical".to_string()];
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().cloned().ok_or(format!("{a} needs a value"));
        let num = |v: String| {
            v.parse::<usize>()
                .map_err(|_| format!("{a}: not a number: {v}"))
        };
        match a.as_str() {
            "--store" => cfg.store_root = PathBuf::from(val()?),
            "--work" => cfg.work_dir = PathBuf::from(val()?),
            "--jobs" => cfg.jobs = u32::try_from(num(val()?)?).map_err(|e| e.to_string())?,
            "--every" => cfg.every = num(val()?)?,
            "--limit" => cfg.limit = Some(num(val()?)?),
            "--io" => {
                cfg.io = match val()?.as_str() {
                    "files" => Io::Files,
                    "pipes" => Io::Pipes,
                    v => return Err(format!("--io: files or pipes, not {v}")),
                }
            }
            "--no-pch" => cfg.use_pch = false,
            "--no-side" => cfg.side = false,
            "--no-variants" => cfg.variants = false,
            "--relaxed" => cfg.durability = nxb_store::Durability::Relaxed,
            "--own-front" => {
                cfg.own_front = Some(nxb_stages::own_front::OwnFrontConfig {
                    verdicts: PathBuf::from(val()?),
                    allow: Vec::new(),
                })
            }
            "--own-allow" => allow = val()?.split(',').map(str::to_string).collect(),
            s if !s.starts_with("--") && b_dir.is_none() => b_dir = Some(PathBuf::from(s)),
            s => return Err(format!("unknown argument {s}")),
        }
    }
    if let Some(o) = &mut cfg.own_front {
        o.allow = allow;
    }
    cfg.b_dir = b_dir.ok_or("missing <b-dir>")?;
    if cfg.store_root.as_os_str().is_empty() || cfg.work_dir.as_os_str().is_empty() {
        return Err("--store and --work are required".into());
    }
    Ok(cfg)
}

pub(crate) fn main(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let cfg = match parse(args) {
        Ok(c) => c,
        Err(e) => {
            let _ = writeln!(err, "shaderbuild recompile: {e}\n{USAGE}");
            return 2;
        }
    };
    let jobs = cfg.jobs;
    let work = cfg.work_dir.clone();
    let sink: Arc<dyn Sink> = Arc::new(NdjsonSink::new(std::io::stderr()));
    match run(cfg, sink) {
        Ok(r) => {
            let _ = std::fs::write(work.join("recompile.tsv"), tsv(&r));
            let _ = write!(out, "{}", summary(&r, jobs));
            let _ = writeln!(out, "{}", r.own_note);
            if let Some((own, fb, cpu, what)) = &r.own {
                let _ = writeln!(
                    out,
                    "own front end: {own} libraries, {fb} fallbacks to Apple, back-half CPU {:.1} s ({what})",
                    cpu.as_secs_f64()
                );
            }
            0
        }
        Err(f) => {
            let _ = writeln!(err, "[RED] {f}");
            f.exit_code()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses() {
        let c = parse(&v(&[
            "/b", "--store", "/s", "--work", "/w", "--jobs", "4", "--every", "32", "--io", "pipes",
            "--no-pch",
        ]))
        .unwrap();
        assert_eq!(c.jobs, 4);
        assert_eq!(c.every, 32);
        assert_eq!(c.io, Io::Pipes);
        assert!(!c.use_pch);
        assert!(c.own_front.is_none());
        let o = parse(&v(&[
            "/b",
            "--store",
            "/s",
            "--work",
            "/w",
            "--own-front",
            "/v.jsonl",
        ]))
        .unwrap();
        assert_eq!(o.own_front.unwrap().allow, v(&["own-identical"]));
        assert!(parse(&v(&["/b"])).is_err());
        assert!(parse(&v(&["/b", "--store", "/s", "--work", "/w", "--io", "x"])).is_err());
    }
}
