//! `airc build`: the own front end as a library in one process, Apple's back half as piped child
//! processes (no temp files): `opt` = air-opt -O1 | air-opt --discard-value-names | metallib,
//! `direct` = air-as | metallib (no optimiser, metallib runs its own per-function air:: passes).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use crate::Opts;

struct Tools {
    air_as: String,
    air_opt: String,
    metallib: String,
    sdk: String,
}

fn find(sdk: &str, t: &str) -> String {
    let o = Command::new("xcrun")
        .args(["-sdk", sdk, "-find", t])
        .output()
        .ok();
    o.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn spawn(
    t: &Tools,
    tool: &str,
    args: &[&str],
    stdin: Stdio,
    stdout: Stdio,
) -> std::io::Result<Child> {
    Command::new(tool)
        .args(args)
        .env_clear()
        .env("SDKROOT", &t.sdk)
        .env("PATH", "/usr/bin:/bin")
        .stdin(stdin)
        .stdout(stdout)
        .stderr(Stdio::null())
        .spawn()
}

/// Pipes bitcode through one air-opt call and metallib.
fn pipe(t: &Tools, bc: &[u8], out: &Path, opt_args: &[&str]) -> Result<(), String> {
    let lib = out.to_string_lossy().to_string();
    let mut a = spawn(t, &t.air_opt, opt_args, Stdio::piped(), Stdio::piped())
        .map_err(|e| e.to_string())?;
    let feed = a.stdin.take();
    let prev = a.stdout.take().ok_or("no stdout")?;
    let mut m = spawn(
        t,
        &t.metallib,
        &["-", "-o", &lib],
        Stdio::from(prev),
        Stdio::null(),
    )
    .map_err(|e| e.to_string())?;
    if let Some(mut w) = feed {
        w.write_all(bc).map_err(|e| e.to_string())?;
    }
    let ok = a.wait().map_err(|e| e.to_string())?.success();
    let ok2 = m.wait().map_err(|e| e.to_string())?.success();
    if ok && ok2 {
        Ok(())
    } else {
        Err("tool failed".into())
    }
}

/// Runs one input, returns (front-end ms, total ms) or an error.
fn one(
    t: &Tools,
    target: &air_emit::Target,
    input: &Path,
    out: &Path,
    direct: bool,
) -> Result<(f64, f64), String> {
    let t0 = Instant::now();
    let src = msl_parse::read_source(input).map_err(|e| e.to_string())?;
    let ir = air_emit::compile_source(&src, target, "-").map_err(|e| e.key())?;
    let fe = t0.elapsed().as_secs_f64() * 1000.0;
    if !direct && crate::check::bc_enabled() && std::env::var_os("AIRC_KEEP_NAMES").is_none() {
        if let Ok(bc) = air_bc::to_bitcode(&ir) {
            if pipe(
                t,
                &bc,
                out,
                &["-O1", "--discard-value-names", "-", "-o", "-"],
            )
            .is_ok()
            {
                return Ok((fe, t0.elapsed().as_secs_f64() * 1000.0));
            }
        }
    }
    let lib = out.to_string_lossy().to_string();
    let mut chain: Vec<Child> = Vec::new();
    let first = if direct {
        spawn(
            t,
            &t.air_as,
            &["-", "-o", "-"],
            Stdio::piped(),
            Stdio::piped(),
        )
    } else {
        spawn(
            t,
            &t.air_opt,
            &["-O1", "-", "-o", "-"],
            Stdio::piped(),
            Stdio::piped(),
        )
    }
    .map_err(|e| e.to_string())?;
    let mut first = first;
    let feed = first.stdin.take();
    let mut prev = first.stdout.take().ok_or("no stdout")?;
    chain.push(first);
    if !direct && std::env::var_os("AIRC_KEEP_NAMES").is_none() {
        let mut c = spawn(
            t,
            &t.air_opt,
            &["--discard-value-names", "-", "-o", "-"],
            Stdio::from(prev),
            Stdio::piped(),
        )
        .map_err(|e| e.to_string())?;
        prev = c.stdout.take().ok_or("no stdout")?;
        chain.push(c);
    }
    let last = spawn(
        t,
        &t.metallib,
        &["-", "-o", &lib],
        Stdio::from(prev),
        Stdio::null(),
    )
    .map_err(|e| e.to_string())?;
    chain.push(last);
    if let Some(mut w) = feed {
        w.write_all(ir.as_bytes()).map_err(|e| e.to_string())?;
    }
    for mut c in chain {
        let st = c.wait().map_err(|e| e.to_string())?;
        if !st.success() {
            return Err("tool failed".into());
        }
    }
    Ok((fe, t0.elapsed().as_secs_f64() * 1000.0))
}

/// Builds every input, prints a summary line (inputs/s, medians).
pub fn run(opts: &Opts) -> ExitCode {
    let Some(work) = &opts.work else {
        eprintln!("airc build: --work DIR is required");
        return ExitCode::from(2);
    };
    let _ = std::fs::create_dir_all(work);
    let t = Tools {
        air_as: find(opts.sdk(), "air-as"),
        air_opt: find(opts.sdk(), "air-opt"),
        metallib: find(opts.sdk(), "metallib"),
        sdk: {
            let o = Command::new("xcrun")
                .args(["-sdk", opts.sdk(), "--show-sdk-path"])
                .output()
                .ok();
            o.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default()
        },
    };
    let direct = opts.direct;
    let target = opts.target();
    let next = AtomicUsize::new(0);
    let res: Mutex<Vec<(f64, f64)>> = Mutex::new(Vec::new());
    let fails = AtomicUsize::new(0);
    let t0 = Instant::now();
    std::thread::scope(|sc| {
        for _ in 0..opts.jobs {
            sc.spawn(|| loop {
                let k = next.fetch_add(1, Ordering::Relaxed);
                let Some(input) = opts.inputs.get(k) else {
                    break;
                };
                let out: PathBuf = work.join(format!("{k}.metallib"));
                match one(&t, &target, input, &out, direct) {
                    Ok(r) => {
                        if let Ok(mut v) = res.lock() {
                            v.push(r);
                        }
                    }
                    Err(_) => {
                        fails.fetch_add(1, Ordering::Relaxed);
                    }
                }
                if !opts.keep {
                    let _ = std::fs::remove_file(&out);
                }
            });
        }
    });
    let wall = t0.elapsed().as_secs_f64();
    let mut v = res.into_inner().unwrap_or_default();
    let n = v.len();
    v.sort_by(|a, b| a.1.total_cmp(&b.1));
    let med = |i: usize| -> f64 {
        if n == 0 {
            0.0
        } else {
            let mut x: Vec<f64> = v.iter().map(|r| if i == 0 { r.0 } else { r.1 }).collect();
            x.sort_by(|a, b| a.total_cmp(b));
            x[n / 2]
        }
    };
    let p90 = if n == 0 { 0.0 } else { v[n * 9 / 10].1 };
    println!(
        "airc build {}: {} ok, {} failed, {} jobs: wall {:.1} s, {:.1} inputs/s, front end median {:.1} ms, total median {:.0} ms, p90 {:.0} ms",
        if direct { "direct" } else { "opt" },
        n,
        fails.load(Ordering::Relaxed),
        opts.jobs,
        wall,
        n as f64 / wall,
        med(0),
        med(1),
        p90
    );
    ExitCode::SUCCESS
}
