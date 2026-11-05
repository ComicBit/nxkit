//! `airc check`: the whole own-front-end pipeline per input, with Apple's assembler, linker and
//! validator: MSL -> AIR text -> `air-as` -> `metallib` -> `air-validate`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use crate::Opts;

/// Paths of Apple's tools and the SDK.
struct Tools {
    air_as: String,
    air_opt: String,
    metallib: String,
    validate: String,
    sdk: String,
}

fn xcrun(args: &[&str]) -> Result<String, String> {
    let out = Command::new("xcrun")
        .args(args)
        .output()
        .map_err(|e| format!("xcrun: {e}"))?;
    if !out.status.success() {
        return Err(format!("xcrun {args:?} failed"));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

impl Tools {
    fn find(sdk: &str) -> Result<Tools, String> {
        Ok(Tools {
            air_as: xcrun(&["-sdk", sdk, "-find", "air-as"])?,
            air_opt: xcrun(&["-sdk", sdk, "-find", "air-opt"])?,
            metallib: xcrun(&["-sdk", sdk, "-find", "metallib"])?,
            validate: xcrun(&["-sdk", sdk, "-find", "air-validate"])?,
            sdk: xcrun(&["-sdk", sdk, "--show-sdk-path"])?,
        })
    }

    /// Runs a tool with only `SDKROOT` and a minimal `PATH`, returns the error tail on failure.
    fn run(&self, tool: &str, args: &[&Path]) -> Result<(), String> {
        let out = Command::new(tool)
            .args(args)
            .env_clear()
            .env("SDKROOT", &self.sdk)
            .env("PATH", "/usr/bin:/bin")
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("spawn {tool}: {e}"))?;
        if out.status.success() {
            return Ok(());
        }
        let mut msg = String::from_utf8_lossy(&out.stderr).to_string();
        msg.push_str(&String::from_utf8_lossy(&out.stdout));
        Err(first_error(&msg))
    }
}

/// The first line that says "error", else the first non-empty line, trimmed of the file path.
fn first_error(msg: &str) -> String {
    let line = msg
        .lines()
        .find(|l| l.contains("error"))
        .or_else(|| msg.lines().find(|l| !l.trim().is_empty()))
        .unwrap_or("failed");
    let line = match line.find("error:") {
        Some(k) => &line[k..],
        None => line,
    };
    line.chars().take(300).collect()
}

/// JSON string literal.
pub fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Result of one input.
pub struct Outcome {
    /// `ok`, `front`, `air-as`, `metallib`, `validate`, `read`.
    pub stage: &'static str,
    /// Error key or message.
    pub key: String,
    /// Front-end time in milliseconds.
    pub fe_ms: f64,
    /// SHA-256 of the library when it validated.
    pub lib_sha: Option<String>,
    /// Whether the back half took the bitcode path.
    pub bc: bool,
}

/// Whether the bitcode path is on (`AIRC_NO_BC` turns it off).
pub(crate) fn bc_enabled() -> bool {
    std::env::var_os("AIRC_NO_BC").is_none()
}

#[allow(clippy::too_many_arguments)]
fn one(
    tools: &Tools,
    target: &air_emit::Target,
    work: &Path,
    idx: usize,
    input: &Path,
    keep: bool,
    opt: bool,
) -> Outcome {
    let src = match msl_parse::read_source(input) {
        Ok(s) => s,
        Err(e) => {
            return Outcome {
                stage: "read",
                key: e.to_string(),
                fe_ms: 0.0,
                lib_sha: None,
                bc: false,
            }
        }
    };
    let t0 = Instant::now();
    let name = "-".to_string();
    let ir = air_emit::compile_source(&src, target, &name);
    let fe_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let ir = match ir {
        Ok(ir) => ir,
        Err(e) => {
            return Outcome {
                stage: "front",
                key: e.key(),
                fe_ms,
                lib_sha: None,
                bc: false,
            }
        }
    };
    let ll: PathBuf = work.join(format!("{idx}.ll"));
    let air: PathBuf = work.join(format!("{idx}.air"));
    let lib: PathBuf = work.join(format!("{idx}.metallib"));
    let ir_text = ir;
    if let Err(e) = std::fs::write(&ll, &ir_text) {
        return Outcome {
            stage: "read",
            key: e.to_string(),
            fe_ms,
            lib_sha: None,
            bc: false,
        };
    }
    let o = Path::new("-o");
    let o1: PathBuf = work.join(format!("{idx}.o1"));
    let mut used_bc = false;
    let bc: PathBuf = work.join(format!("{idx}.bc"));
    let res = (|| {
        if opt {
            // A3's back half: Apple's O1 pipeline on our IR, value names dropped
            let lvl = Path::new("-O1");
            let dvn = Path::new("--discard-value-names");
            // bitcode input lets one air-opt call drop the names, text needs two
            let via_bc = bc_enabled()
                && air_bc::to_bitcode(&ir_text).is_ok_and(|b| std::fs::write(&bc, b).is_ok())
                && tools.run(&tools.air_opt, &[lvl, dvn, &bc, o, &air]).is_ok();
            used_bc = via_bc;
            if !via_bc {
                tools
                    .run(&tools.air_opt, &[lvl, &ll, o, &o1])
                    .map_err(|e| ("air-opt", e))?;
                tools
                    .run(&tools.air_opt, &[dvn, &o1, o, &air])
                    .map_err(|e| ("air-opt", e))?;
            }
        } else {
            tools
                .run(&tools.air_as, &[&ll, o, &air])
                .map_err(|e| ("air-as", e))?;
        }
        tools
            .run(&tools.metallib, &[&air, o, &lib])
            .map_err(|e| ("metallib", e))?;
        tools
            .run(&tools.validate, &[&lib])
            .map_err(|e| ("validate", e))?;
        Ok::<(), (&'static str, String)>(())
    })();
    let ok = res.is_ok();
    let lib_sha = if ok {
        std::fs::read(&lib).ok().map(|b| crate::sha256::hex(&b))
    } else {
        None
    };
    let _ = std::fs::remove_file(&o1);
    let _ = std::fs::remove_file(&bc);
    if !keep {
        let _ = std::fs::remove_file(&air);
        if ok {
            let _ = std::fs::remove_file(&ll);
            let _ = std::fs::remove_file(&lib);
        }
    }
    match res {
        Ok(()) => Outcome {
            stage: "ok",
            key: String::new(),
            fe_ms,
            lib_sha,
            bc: used_bc,
        },
        Err((stage, key)) => Outcome {
            stage,
            key,
            fe_ms,
            lib_sha: None,
            bc: false,
        },
    }
}

/// `lib_sha_ref` of a source in A3's store (`<store>/<k[..2]>/<k>/meta.json`).
fn ref_lib_sha(store: &Path, key: &str) -> Option<String> {
    let meta = std::fs::read_to_string(store.join(&key[..2]).join(key).join("meta.json")).ok()?;
    let pat = "\"lib_sha_ref\": \"";
    let k = meta.find(pat)? + pat.len();
    Some(
        meta[k..]
            .chars()
            .take_while(|c| c.is_ascii_hexdigit())
            .collect(),
    )
}

/// Runs the pipeline over all inputs, one JSON line per input on stdout.
pub fn run(opts: &Opts) -> i32 {
    let Some(work) = &opts.work else {
        eprintln!("airc check: --work DIR is required");
        return 2;
    };
    if let Err(e) = std::fs::create_dir_all(work) {
        eprintln!("airc check: {}: {e}", work.display());
        return 1;
    }
    let target = opts.target();
    let tools = match Tools::find(opts.sdk()) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("airc check: {e}");
            return 1;
        }
    };
    let next = AtomicUsize::new(0);
    let out = Mutex::new(std::io::stdout());
    let counts = Mutex::new(std::collections::BTreeMap::<&'static str, usize>::new());
    std::thread::scope(|sc| {
        for _ in 0..opts.jobs {
            sc.spawn(|| loop {
                let k = next.fetch_add(1, Ordering::Relaxed);
                let Some(input) = opts.inputs.get(k) else { break };
                let t0 = Instant::now();
                let r = one(&tools, &target, work, k, input, opts.keep, opts.opt);
                let ms = t0.elapsed().as_secs_f64() * 1000.0;
                let src_sha = std::fs::read(input).map(|b| crate::sha256::hex(&b)).unwrap_or_default();
                let lowers = !matches!(r.stage, "front" | "read");
                let validates = r.stage == "ok";
                let bytes_equal = match (&r.lib_sha, &opts.ref_store) {
                    (Some(got), Some(store)) => ref_lib_sha(store, &src_sha).map(|want| *got == want),
                    _ => None,
                };
                // the deny-list verdict: own bytes only when validated, identical when the store says so
                let verdict = match (validates, bytes_equal) {
                    (true, Some(true)) => "own-identical",
                    (true, _) => "own-validated",
                    _ => "apple",
                };
                let be = match bytes_equal {
                    Some(b) => b.to_string(),
                    None => "null".to_string(),
                };
                let line = format!(
                    "{{\"i\": {k}, \"path\": {}, \"source_sha256\": \"{src_sha}\", \"lowers\": {lowers}, \"validates\": {validates}, \"bytes_equal\": {be}, \"verdict\": \"{verdict}\", \"stage\": \"{}\", \"key\": {}, \"fe_ms\": {:.3}, \"ms\": {:.1}, \"bc\": {}}}\n",
                    json_str(&input.display().to_string()),
                    r.stage,
                    json_str(&r.key),
                    r.fe_ms,
                    ms,
                    r.bc
                );
                if let Ok(mut o) = out.lock() {
                    let _ = o.write_all(line.as_bytes());
                }
                if let Ok(mut c) = counts.lock() {
                    *c.entry(r.stage).or_default() += 1;
                }
            });
        }
    });
    if let Ok(c) = counts.lock() {
        let parts: Vec<String> = c.iter().map(|(k, v)| format!("{k} {v}")).collect();
        eprintln!(
            "airc check: {} inputs: {}",
            opts.inputs.len(),
            parts.join(", ")
        );
    }
    0
}
