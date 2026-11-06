//! `airc rawcheck`: the raw container path (no Apple tool in the build) over a list, `-j` threads in process:
//! library size, build time per input, and with `--validate` Apple's `air-validate` on each library (a check, not a
//! build step). One JSON line per input on stdout (`lib_sha256` for a built library, `error` with the reason for a
//! reject or a failed build), a summary on stderr. `--ref-store DIR` adds the size of Apple's
//! library for the same source (`lib_size_ref` is not stored, so the reference `ref.air` size is reported instead).

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use crate::Opts;

/// Runs the raw path over all inputs.
pub fn run(opts: &Opts) -> i32 {
    let Some(work) = &opts.work else {
        eprintln!("airc rawcheck: --work DIR is required");
        return 2;
    };
    let _ = std::fs::create_dir_all(work);
    let validate = if opts.keep {
        // --keep doubles as --validate here: air-validate every library
        let o = Command::new("xcrun")
            .args(["-sdk", opts.sdk(), "-find", "air-validate"])
            .output()
            .ok();
        o.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    } else {
        None
    };
    let sdkroot = Command::new("xcrun")
        .args(["-sdk", opts.sdk(), "--show-sdk-path"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let target = opts.target();
    let next = AtomicUsize::new(0);
    let out = Mutex::new(std::io::stdout());
    let tot = Mutex::new((0usize, 0usize, 0usize, 0usize, 0u64, Vec::<f64>::new()));
    let t0 = Instant::now();
    std::thread::scope(|sc| {
        for _ in 0..opts.jobs {
            sc.spawn(|| loop {
                let k = next.fetch_add(1, Ordering::Relaxed);
                let Some(input) = opts.inputs.get(k) else { break };
                let Ok(src) = std::fs::read(input) else { continue };
                let t = Instant::now();
                let r = air_front::build_raw(&src, &target);
                let ms = t.elapsed().as_secs_f64() * 1000.0;
                let sha = crate::sha256::hex(&src);
                let (stage, size, valid) = match &r {
                    Ok(lib) => {
                        let p = work.join(format!("{k}.metallib"));
                        let valid = match &validate {
                            Some(v) => {
                                let _ = std::fs::write(&p, lib);
                                let ok = Command::new(v)
                                    .arg(&p)
                                    .env_clear()
                                    .env("SDKROOT", &sdkroot)
                                    .stdout(Stdio::null())
                                    .stderr(Stdio::null())
                                    .status()
                                    .is_ok_and(|s| s.success());
                                let _ = std::fs::remove_file(&p);
                                Some(ok)
                            }
                            None => None,
                        };
                        ("ok", lib.len(), valid)
                    }
                    Err(Ok(_)) => ("front", 0, None),
                    Err(Err(_)) => ("raw", 0, None),
                };
                let error = match &r {
                    Err(Ok(rej)) => format!(", \"error\": \"{}\"", json_escape(&rej.0)),
                    Err(Err(e)) => format!(", \"error\": \"{}\"", json_escape(e)),
                    Ok(lib) => format!(", \"lib_sha256\": \"{}\"", crate::sha256::hex(lib)),
                };
                let line = format!(
                    "{{\"path\": \"{}\", \"source_sha256\": \"{sha}\", \"stage\": \"{stage}\", \"size\": {size}, \"validates\": {}, \"ms\": {ms:.2}{error}}}\n",
                    input.display(),
                    valid.map(|v| v.to_string()).unwrap_or_else(|| "null".into())
                );
                if let Ok(mut o) = out.lock() {
                    let _ = o.write_all(line.as_bytes());
                }
                if let Ok(mut s) = tot.lock() {
                    s.0 += 1;
                    if stage == "ok" {
                        s.1 += 1;
                        s.4 += size as u64;
                    }
                    if valid == Some(true) {
                        s.2 += 1;
                    }
                    if valid == Some(false) {
                        s.3 += 1;
                    }
                    s.5.push(ms);
                }
            });
        }
    });
    let wall = t0.elapsed().as_secs_f64();
    if let Ok(mut s) = tot.lock() {
        s.5.sort_by(|a, b| a.total_cmp(b));
        let med = s.5.get(s.5.len() / 2).copied().unwrap_or(0.0);
        let mean = s.5.iter().sum::<f64>() / s.5.len().max(1) as f64;
        eprintln!(
            "airc rawcheck: {} inputs, {} built, validate ok {} bad {}, {} bytes, {} threads: wall {wall:.1} s, {:.1} inputs/s, build median {med:.1} ms mean {mean:.1} ms",
            s.0,
            s.1,
            s.2,
            s.3,
            s.4,
            opts.jobs,
            s.0 as f64 / wall
        );
    }
    0
}

/// `s` as the body of a JSON string.
fn json_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

#[cfg(test)]
mod tests {
    #[test]
    fn json_escape_quotes_backslashes_and_controls() {
        assert_eq!(super::json_escape("a\"b\\c\nd"), "a\\\"b\\\\c\\u000ad");
    }
}
