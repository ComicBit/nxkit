//! The command line of the `nxb` and `shaderbuild` binaries (layer L3).
//!
//! Both binaries share this code. `nxb` is the whole build and the stage commands (design-pipeline-rest 11).
//! `shaderbuild` is the shader-step CLI (design-v2 7.2). Implemented: `version`, `help`, `shaderbuild recompile`.
//! every other command answers "not implemented yet" with exit code 2.

use std::io::Write;

mod build;
mod recompile;

/// Which binary is running.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Binary {
    /// `nxb`
    Nxb,
    /// `shaderbuild`
    Shaderbuild,
}

impl Binary {
    /// The program name.
    pub fn name(self) -> &'static str {
        match self {
            Binary::Nxb => "nxb",
            Binary::Shaderbuild => "shaderbuild",
        }
    }

    /// The planned commands, in help order.
    pub fn commands(self) -> &'static [&'static str] {
        match self {
            Binary::Nxb => &[
                "build", "extract", "rewrite", "split", "app", "ipa", "pso", "sign", "parity",
                "bench", "explain", "version", "help",
            ],
            Binary::Shaderbuild => &[
                "build",
                "recompile",
                "guards",
                "own-verdicts",
                "parity",
                "explain",
                "bench",
                "prune",
                "regressions",
                "fe-verify",
                "self-check",
                "version",
                "help",
            ],
        }
    }
}

/// Run a binary with its arguments (without the program name) and return the exit code.
pub fn run(bin: Binary, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let cmd = args.first().map(String::as_str).unwrap_or("help");
    let res = match cmd {
        "version" | "--version" | "-V" => {
            writeln!(out, "{} {}", bin.name(), env!("CARGO_PKG_VERSION")).map(|_| 0)
        }
        "help" | "--help" | "-h" => {
            let list = bin.commands().join(", ");
            writeln!(
                out,
                "usage: {} <command> [args]\ncommands: {list}",
                bin.name()
            )
            .map(|_| 0)
        }
        "recompile" if bin == Binary::Shaderbuild => {
            return recompile::main(&args[1..], out, err);
        }
        "guards" if bin == Binary::Shaderbuild => return guards(&args[1..], out, err),
        "own-verdicts" if bin == Binary::Shaderbuild => return own_verdicts(&args[1..], out, err),
        "build" if bin == Binary::Shaderbuild => return build::main(&args[1..], out, err),
        // the whole build (ADR W19: nxb-build is the orchestrator crate)
        "build" if bin == Binary::Nxb => return nxb_build::main(&args[1..]),
        c if bin.commands().contains(&c) => {
            writeln!(err, "{}: {c}: not implemented yet", bin.name()).map(|_| 2)
        }
        c => writeln!(err, "{}: unknown command {c:?} (try help)", bin.name()).map(|_| 2),
    };
    res.unwrap_or(1)
}

/// `shaderbuild own-verdicts <validation.jsonl> [data-dir]`: write the shipped verdict file of this front end and this
/// Mac toolchain (own-identical shaders only).
fn own_verdicts(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let Some(jsonl) = args.first() else {
        let _ = writeln!(
            err,
            "usage: shaderbuild own-verdicts <validation.jsonl> [data-dir]"
        );
        return 2;
    };
    let data = args.get(1).map_or_else(
        || nxb_stages::own_front::default_config().verdicts,
        std::path::PathBuf::from,
    );
    let tmp = std::env::temp_dir().join(format!("nxb-own-verdicts-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let r = nxb_stages::own_front::mac_compiler_id(&tmp)
        .and_then(|cid| nxb_stages::own_front::ship(std::path::Path::new(jsonl), &data, &cid));
    let _ = std::fs::remove_dir_all(&tmp);
    match r {
        Ok((f, n)) => writeln!(out, "{n} own-identical shaders: {}", f.display()).map_or(1, |_| 0),
        Err(e) => {
            let _ = writeln!(err, "[RED] {e}");
            1
        }
    }
}

/// `shaderbuild guards <b-dir> <out-dir> [jobs]`: write `c1_full.tsv` and `cb_extent.tsv`.
fn guards(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let (Some(b), Some(o)) = (args.first(), args.get(1)) else {
        let _ = writeln!(err, "usage: shaderbuild guards <b-dir> <out-dir> [jobs]");
        return 2;
    };
    let jobs = args.get(2).and_then(|j| j.parse().ok()).unwrap_or(6);
    match nxb_stages::guard_tables::write(std::path::Path::new(b), std::path::Path::new(o), jobs) {
        Ok((c1, cb)) => {
            let _ = writeln!(
                out,
                "not proven bounded: {c1} shaders; uniform extent rows: {cb}"
            );
            0
        }
        Err(e) => {
            let _ = writeln!(err, "[RED] guards: {e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(bin: Binary, args: &[&str]) -> (i32, String, String) {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let code = run(bin, &args, &mut o, &mut e);
        (
            code,
            String::from_utf8_lossy(&o).into(),
            String::from_utf8_lossy(&e).into(),
        )
    }

    #[test]
    fn version_and_help() {
        let (c, o, _) = call(Binary::Shaderbuild, &["version"]);
        assert_eq!(c, 0);
        assert!(o.starts_with("shaderbuild "));
        let (c, o, _) = call(Binary::Nxb, &[]);
        assert_eq!(c, 0);
        assert!(o.contains("extract"));
    }

    #[test]
    fn unknown_and_planned() {
        assert_eq!(call(Binary::Nxb, &["frobnicate"]).0, 2);
        let (c, _, e) = call(Binary::Shaderbuild, &["prune"]);
        assert_eq!(c, 2);
        assert!(e.contains("not implemented"));
    }
}
