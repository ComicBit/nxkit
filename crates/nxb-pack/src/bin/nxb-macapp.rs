//! `nxb-macapp <out.app> --host <dmhost> --companion-app <nxnative> --patched <dir> --romfs <img> --msl <dir> --tables <dir>
//! --font <dir> --title-id <id> --name <name> [--version v] [--category c] [--lang l] [--marker m] [--bundle-id id]
//! [--html <dir>] [--pso <dir>] [--pso-pairs <tsv>] [--env-file f] [--settings <app settings json>] [--icns <AppIcon.icns>]
//! [--fork-patches <fork_patches.txt>] [--commit c] [--msl-name n] [--previous <old.app>] [--threads N]
//! [--dest-digests <tsv>]`
//!
//! `pack_app.py`'s Mac app: the file set from the build outputs, staged with clones and signed ad hoc. With
//! `--previous`, the previous app is updated in place: only files whose digest changed are staged again.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let mut opt: BTreeMap<String, String> = BTreeMap::new();
    let mut program_rows: Vec<String> = Vec::new();
    let mut out = None;
    let mut i = 0;
    while i < a.len() {
        match a[i].strip_prefix("--") {
            Some("program") => {
                program_rows.push(a.get(i + 1).cloned().unwrap_or_default());
                i += 2;
            }
            Some(k) => {
                opt.insert(k.to_string(), a.get(i + 1).cloned().unwrap_or_default());
                i += 2;
            }
            None => {
                out = Some(PathBuf::from(&a[i]));
                i += 1;
            }
        }
    }
    let p = |k: &str| opt.get(k).map(PathBuf::from);
    let s = |k: &str| opt.get(k).cloned();
    let fork_patches = match p("fork-patches") {
        Some(f) => match std::fs::read_to_string(&f) {
            Ok(t) => t
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join(","),
            Err(e) => {
                eprintln!("nxb-macapp: {}: {e}", f.display());
                return ExitCode::from(1);
            }
        },
        None => String::new(),
    };
    let (
        Some(out),
        Some(host),
        Some(companion),
        Some(patched),
        Some(romfs),
        Some(msl),
        Some(tables),
        Some(font),
        Some(title_id),
        Some(name),
    ) = (
        out,
        p("host"),
        p("companion-app"),
        p("patched"),
        p("romfs"),
        p("msl"),
        p("tables"),
        p("font"),
        s("title-id"),
        s("name"),
    )
    else {
        eprintln!("usage: see the source header of nxb-macapp");
        return ExitCode::from(2);
    };
    let inputs = nxb_pack::macapp::MacAppInputs {
        host,
        companion,
        patched,
        romfs,
        msl,
        msl_name: s("msl-name"),
        tables,
        font,
        pso: p("pso"),
        pso_pairs: p("pso-pairs"),
        html: p("html"),
        env_file: p("env-file"),
        settings: p("settings"),
        icns: p("icns"),
        title_id,
        name,
        version: s("version"),
        bundle_id: s("bundle-id"),
        category: s("category"),
        lang: s("lang"),
        marker: s("marker"),
        fork_patches,
        commit: s("commit"),
        aoc: p("aoc"),
        programs: match program_rows
            .iter()
            .map(|r| nxb_pack::macapp::Program::parse(r))
            .collect()
        {
            Ok(v) => v,
            Err(e) => {
                eprintln!("nxb-macapp: {e}");
                return ExitCode::from(2);
            }
        },
    };
    let threads = s("threads")
        .and_then(|t| t.parse().ok())
        .unwrap_or_else(nxb_pack::digest::default_threads);
    let t = std::time::Instant::now();
    // digests the producers know (a store), by destination path relative to `Contents/` (`nxb-pack digests` form)
    let known = match p("dest-digests") {
        Some(f) => match nxb_pack::TsvDigests::load(&f) {
            Ok(t) => Some(t),
            Err(e) => {
                eprintln!("nxb-macapp: {e}");
                return ExitCode::from(1);
            }
        },
        None => None,
    };
    let with_known = |mut plan: nxb_pack::appstage::StagePlan| {
        use nxb_pack::DigestSource;
        if let Some(k) = &known {
            for e in &mut plan.entries {
                let size = match &e.content {
                    nxb_pack::appstage::Content::File(p) => {
                        std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
                    }
                    nxb_pack::appstage::Content::Bytes(b) => b.len() as u64,
                };
                if let Some(rel) = e.dest.strip_prefix("Contents/") {
                    e.digests = k.lookup(rel, size);
                }
            }
        }
        plan
    };
    // the reuse key (<app>.key): equal inputs and the app on disk, nothing to do, dropped before any change
    let mut key_file = out.as_os_str().to_owned();
    key_file.push(".key");
    let key_file = PathBuf::from(key_file);
    let plan = match nxb_pack::macapp::plan(&inputs) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("nxb-macapp: {e}");
            return ExitCode::from(1);
        }
    };
    let key = nxb_pack::macapp::reuse_key(&plan).unwrap_or_default();
    if out.is_dir()
        && !key.is_empty()
        && std::fs::read_to_string(&key_file).is_ok_and(|k| k.trim() == key)
    {
        println!("app: {}, reused (inputs unchanged)", out.display());
        return ExitCode::SUCCESS;
    }
    let _ = std::fs::remove_file(&key_file);
    let r = Ok::<_, nxb_pack::Error>(with_known(plan)).and_then(|plan| match p("previous") {
        Some(prev) => {
            nxb_pack::incremental::update(&plan, &prev, &out, threads).map(|r| r.to_string())
        }
        None => {
            let opts = nxb_pack::appstage::StageOptions {
                threads,
                ..Default::default()
            };
            nxb_pack::appstage::stage_and_sign(&plan, &out, &opts).map(|r| {
                format!(
                    "staged {} files ({} MB) in {:.2} s, signed in {:.2} s",
                    r.files,
                    r.bytes / 1_000_000,
                    r.stage_time.as_secs_f64(),
                    r.sign_time.as_secs_f64()
                )
            })
        }
    });
    match r {
        Ok(line) => {
            if !key.is_empty() {
                let _ = std::fs::write(&key_file, format!("{key}\n"));
            }
            println!(
                "app: {}: {line}; {:.2} s in all",
                out.display(),
                t.elapsed().as_secs_f64()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("nxb-macapp: {e}");
            ExitCode::from(if matches!(e, nxb_pack::Error::Unsupported(_)) {
                3
            } else {
                1
            })
        }
    }
}
