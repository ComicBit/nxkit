//! `nxb-extract --keys <prod.keys> --base <nsp> --out <dir> [--update <nsp> <dir>]... [--threads N] [--digests <tsv>]`

use std::path::PathBuf;
use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!(
        "usage: nxb-extract --keys <prod.keys> --base <base.nsp> --out <dir> [--update <update.nsp> <dir>]... [--threads N] [--mem-mib N] [--cache-mib N] [--digests <tsv>]"
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() == 3 && a[1] == "--contents" {
        return match nxb_extract::run::contents(
            std::path::Path::new(&a[0]),
            std::path::Path::new(&a[2]),
        ) {
            Ok(t) => {
                print!("{t}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                println!("[RED] {e}");
                ExitCode::from(e.exit_code() as u8)
            }
        };
    }
    let (mut keys, mut base, mut out, mut digests) = (None, None, None, None);
    let mut updates = Vec::new();
    let mut threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let (mut mem, mut cache) = (1024u64, 512usize);
    let mut html = false;
    let mut base_outputs = true;
    let mut split: std::collections::BTreeMap<String, PathBuf> = std::collections::BTreeMap::new();
    let mut i = 0;
    while i < a.len() {
        let next = |k: usize| a.get(k).cloned();
        match a[i].as_str() {
            "--keys" => keys = next(i + 1).map(PathBuf::from),
            "--base" => base = next(i + 1).map(PathBuf::from),
            "--out" => out = next(i + 1).map(PathBuf::from),
            "--digests" => digests = next(i + 1).map(PathBuf::from),
            "--threads" => threads = next(i + 1).and_then(|v| v.parse().ok()).unwrap_or(threads),
            "--mem-mib" => mem = next(i + 1).and_then(|v| v.parse().ok()).unwrap_or(mem),
            "--cache-mib" => cache = next(i + 1).and_then(|v| v.parse().ok()).unwrap_or(cache),
            k @ ("--split-game" | "--split-pkg" | "--split-map" | "--split-facts"
            | "--split-pairs" | "--split-pso-pairs") => {
                if let Some(v) = next(i + 1) {
                    split.insert(
                        k.trim_start_matches("--split-").to_string(),
                        PathBuf::from(v),
                    );
                }
            }
            "--updates-only" => {
                base_outputs = false;
                i += 1;
                continue;
            }
            "--html" => {
                html = true;
                i += 1;
                continue;
            }
            "--update" => match (next(i + 1), next(i + 2)) {
                (Some(u), Some(d)) => {
                    updates.push((PathBuf::from(u), PathBuf::from(d)));
                    i += 1;
                }
                _ => return usage(),
            },
            _ => return usage(),
        }
        i += 2;
    }
    let (Some(keys), Some(base), Some(out_base)) = (keys, base, out) else {
        return usage();
    };
    let g = |k: &str| split.get(k).cloned();
    let split = match (
        g("game"),
        g("pkg"),
        g("map"),
        g("facts"),
        g("pairs"),
        g("pso-pairs"),
    ) {
        (Some(game), Some(pkg), Some(map), Some(facts), Some(pairs), Some(pso_pairs)) => Some((
            game,
            nxb_extract::split::SplitOutputs {
                pkg,
                map,
                facts,
                pairs,
                pso_pairs,
            },
        )),
        _ if split.is_empty() => None,
        _ => return usage(),
    };
    let req = nxb_extract::Request {
        keys,
        base,
        out_base,
        updates,
        threads,
        mem_limit: mem << 20,
        cache_chunks: cache,
        digests,
        html,
        base_outputs,
        split,
    };
    match nxb_extract::extract(&req) {
        Ok(mut s) => {
            let closing = std::mem::replace(&mut s.closing, nxb_extract::write::Closing::done());
            eprintln!("nxb-extract: {}", nxb_extract::prof::summary());
            eprintln!(
                "nxb-extract: open + small files {:.2} s, images {:.2} s; decrypted {:.1} MB in {} chunks, {} cache hits",
                s.seconds.0,
                s.seconds.1,
                s.cache.produced_bytes as f64 / 1e6,
                s.cache.produced,
                s.cache.hits
            );
            let c = closing.wait();
            if c > 0.5 {
                eprintln!(
                    "nxb-extract: closing the images took {c:.1} s more (the disk taking the data)"
                );
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            if let nxb_extract::ExtractError::Damaged(p) = &e {
                for x in p {
                    println!("[RED] hash check failed: {x}");
                }
            }
            println!("[RED] {e}");
            ExitCode::from(e.exit_code() as u8)
        }
    }
}
