//! `nxb cache export|import|prune|list`: a title's shader and pipeline cache as a store export (design 2.8, 2.12).
//!
//! `nxb cache export <store> <out> [--device-family F] [--translator T] [--emitter E] [--variant V]`
//! `nxb cache import <store> <file> [--cap-mb N]`, `nxb cache prune <store> --cap-mb N`, `nxb cache list <store> [filters]`

use nxb_store::{Filter, Store};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

const USAGE: &str = "usage: nxb cache export <store> <out> [filters] | import <store> <file> [--cap-mb N] | prune <store> --cap-mb N | list <store> [filters]\nfilters: --device-family F --translator T --emitter E --variant V";

struct Args {
    pos: Vec<String>,
    filter: Filter,
    cap: Option<u64>,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut a = Args {
        pos: Vec::new(),
        filter: Filter::default(),
        cap: None,
    };
    let mut it = args.iter();
    while let Some(x) = it.next() {
        let mut val = |what: &str| it.next().cloned().ok_or(format!("{what} needs a value"));
        match x.as_str() {
            "--device-family" => a.filter.device_family = Some(val(x)?),
            "--translator" => a.filter.translator = Some(val(x)?),
            "--emitter" => a.filter.emitter = Some(val(x)?),
            "--variant" => a.filter.variant = Some(val(x)?),
            "--cap-mb" => {
                let mb: u64 = val(x)?
                    .parse()
                    .map_err(|_| "--cap-mb needs a number".to_string())?;
                a.cap = Some(mb.saturating_mul(1 << 20));
            }
            s if s.starts_with("--") => return Err(format!("unknown option {s}")),
            _ => a.pos.push(x.clone()),
        }
    }
    Ok(a)
}

/// Run the command, exit code 0, 1 for a failed operation, 2 for bad usage.
pub(crate) fn main(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    match run(args, out) {
        Ok(()) => 0,
        Err((code, msg)) => {
            let _ = writeln!(
                err,
                "{}",
                if code == 2 {
                    format!("{msg}\n{USAGE}")
                } else {
                    format!("[RED] {msg}")
                }
            );
            code
        }
    }
}

fn run(args: &[String], out: &mut dyn Write) -> Result<(), (i32, String)> {
    let bad = |m: String| (2, m);
    let fail = |m: String| (1, m);
    let sub = args.first().map(String::as_str).unwrap_or("");
    let a = parse(args.get(1..).unwrap_or(&[])).map_err(bad)?;
    let store_dir = PathBuf::from(
        a.pos
            .first()
            .ok_or_else(|| bad("store folder missing".into()))?,
    );
    let w = |e: std::io::Error| fail(e.to_string());
    match sub {
        "export" => {
            let dest = PathBuf::from(
                a.pos
                    .get(1)
                    .ok_or_else(|| bad("output file missing".into()))?,
            );
            let s = Store::open(&store_dir).map_err(|e| fail(e.to_string()))?;
            let tmp = dest.with_extension("partial");
            let mut f = BufWriter::new(File::create(&tmp).map_err(w)?);
            let r = s
                .export_cache(&a.filter, &mut f)
                .map_err(|e| fail(e.to_string()));
            let r = r.and_then(|r| {
                f.into_inner()
                    .map_err(|e| fail(e.to_string()))
                    .and_then(|f| f.sync_all().map(|_| r).map_err(w))
            });
            let r = match r {
                Ok(r) => std::fs::rename(&tmp, &dest).map(|_| r).map_err(w),
                Err(e) => Err(e),
            };
            if r.is_err() {
                let _ = std::fs::remove_file(&tmp);
            }
            let r = r?;
            writeln!(
                out,
                "exported {} entries, {} blobs, {} bytes: {}",
                r.entries,
                r.blobs,
                r.bytes,
                dest.display()
            )
            .map_err(w)
        }
        "import" => {
            let src = a
                .pos
                .get(1)
                .ok_or_else(|| bad("input file missing".into()))?;
            let f = File::open(Path::new(src)).map_err(|e| fail(format!("{src}: {e}")))?;
            let s = Store::open(&store_dir).map_err(|e| fail(e.to_string()))?;
            let r = s
                .import_cache(BufReader::new(f))
                .map_err(|e| fail(e.to_string()))?;
            writeln!(
                out,
                "imported {} entries ({} already there), {} blobs, {} bytes",
                r.added, r.present, r.blobs, r.bytes
            )
            .map_err(w)?;
            drop(s);
            match a.cap {
                Some(cap) => prune(&store_dir, cap, out),
                None => Ok(()),
            }
        }
        "prune" => prune(
            &store_dir,
            a.cap.ok_or_else(|| bad("--cap-mb missing".into()))?,
            out,
        ),
        "list" => {
            let s = Store::open(&store_dir).map_err(|e| fail(e.to_string()))?;
            for e in s.list_cache(&a.filter).map_err(|e| fail(e.to_string()))? {
                let k = &e.key;
                writeln!(
                    out,
                    "{} {} {} {} {} {}",
                    k.shader_key,
                    k.translator,
                    k.emitter,
                    k.device_family,
                    k.variant,
                    e.bytes()
                )
                .map_err(w)?;
            }
            Ok(())
        }
        _ => Err(bad(format!("unknown cache command {sub:?}"))),
    }
}

fn prune(dir: &Path, cap: u64, out: &mut dyn Write) -> Result<(), (i32, String)> {
    let s = Store::open_exclusive(dir, false).map_err(|e| (1, e.to_string()))?;
    let r = s.prune(cap).map_err(|e| (1, e.to_string()))?;
    writeln!(
        out,
        "pruned {} entries, {} blobs, {} bytes, {} bytes kept",
        r.entries, r.blobs, r.bytes, r.kept_bytes
    )
    .map_err(|e| (1, e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nxb_store::CacheKey;

    #[test]
    fn export_import_prune_through_the_command() {
        let base = std::env::temp_dir().join(format!("nxb-cli-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (a, b, f) = (base.join("a"), base.join("b"), base.join("x.nxbcache"));
        {
            let s = Store::open(&a).unwrap();
            for i in 0..3 {
                let k = CacheKey {
                    shader_key: format!("k{i}"),
                    translator: "t1".into(),
                    emitter: "e1".into(),
                    device_family: "apple9".into(),
                    variant: "base".into(),
                };
                s.put_cache(&k, &[("lib", &[i as u8; 1000])]).unwrap();
            }
        }
        let run = |args: &[&str]| {
            let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let c = main(&args, &mut o, &mut e);
            (
                c,
                String::from_utf8(o).unwrap(),
                String::from_utf8(e).unwrap(),
            )
        };
        let (c, o, _) = run(&[
            "export",
            a.to_str().unwrap(),
            f.to_str().unwrap(),
            "--device-family",
            "apple9",
        ]);
        assert_eq!(c, 0);
        assert!(o.starts_with("exported 3 entries"), "{o}");
        let (c, o, _) = run(&["import", b.to_str().unwrap(), f.to_str().unwrap()]);
        assert_eq!(c, 0);
        assert!(o.starts_with("imported 3 entries"), "{o}");
        assert_eq!(run(&["list", b.to_str().unwrap()]).1.lines().count(), 3);
        let (c, o, _) = run(&["prune", b.to_str().unwrap(), "--cap-mb", "0"]);
        assert_eq!(c, 0);
        assert!(o.contains("pruned 3 entries"), "{o}");
        assert_eq!(run(&["export", a.to_str().unwrap()]).0, 2);
        assert_eq!(run(&["frob", a.to_str().unwrap()]).0, 2);
        let _ = std::fs::remove_dir_all(&base);
    }
}
