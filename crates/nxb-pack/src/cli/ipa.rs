//! `nxb-ipa <out.ipa> [--store .a:.b] [--skip <path>]... [--skip-suffix <s>]... <archive path>=<source>...`: `pack_ipa.py`'s
//! zip (see `crate::ipa`, files are stored until a zlib-exact deflate is available).

use std::collections::HashSet;
use std::path::PathBuf;

/// `nxb ipa` / `nxb-ipa`: returns the exit code.
pub fn main(args: &[String]) -> i32 {
    let a = args;
    let mut out = None;
    let (mut store, mut skip, mut skip_suffix, mut maps) =
        (Vec::new(), HashSet::new(), Vec::new(), Vec::new());
    let mut i = 0;
    while i < a.len() {
        let v = a.get(i + 1).cloned().unwrap_or_default();
        match a[i].as_str() {
            "--store" => {
                store = v
                    .split(':')
                    .filter(|s| !s.is_empty())
                    .map(str::to_lowercase)
                    .collect();
                i += 2;
            }
            "--skip" => {
                skip.insert(v.trim_matches('/').to_string());
                i += 2;
            }
            "--skip-suffix" => {
                skip_suffix.push(v);
                i += 2;
            }
            m => {
                match m.split_once('=') {
                    Some((arc, src)) if out.is_some() => {
                        maps.push((arc.trim_matches('/').to_string(), PathBuf::from(src)))
                    }
                    _ if out.is_none() => out = Some(PathBuf::from(m)),
                    _ => {
                        eprintln!("[RED] pack_ipa: not <archive path>=<file or folder>: {m}");
                        return 2;
                    }
                }
                i += 1;
            }
        }
    }
    let Some(out) = out else {
        eprintln!(
            "usage: nxb-ipa <out.ipa> [--store .a:.b] [--skip p] [--skip-suffix s] <archive path>=<source>..."
        );
        return 2;
    };
    let o = crate::ipa::PackOptions {
        store,
        skip,
        skip_suffix,
        reuse: std::env::var_os("NXB_IPA_NO_REUSE").is_none(),
        compress: if std::env::var_os("NXB_IPA_STORE_ONLY").is_some() {
            crate::ipa::STORE_ONLY
        } else {
            crate::ipa::ZLIB
        },
    };
    match crate::ipa::pack(&out, &maps, &o) {
        Ok(n) => {
            println!("packed: {n} entries");
            0
        }
        Err(e) => {
            eprintln!("[RED] pack_ipa: {e}");
            1
        }
    }
}
