//! The shader step and the pipeline archive in one process graph: `nxb_stages::build::build` is called as a library
//! (no `shaderbuild` process) and reports every finished shader, the archive's pair groups (`nxb_archive::stream`) start
//! their `metal-tt` batches as soon as their shaders are done, on worker threads beside the shader step, and the shader
//! step translates the archive's shaders first so the groups complete one after another.

use crate::Args;
use crate::events::{Progress, Update};
use crate::graph::Build;
use nxb_pyjson::Value;
use nxb_stages::build::{BuildConfig, OnDone};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// `DOTNET_ROOT` as the NSP script finds it: the option, the environment, else `dotnet --list-runtimes` on `PATH`.
fn dotnet_root(a: &Args) -> Result<PathBuf, String> {
    if let Some(d) = &a.dotnet_root {
        return Ok(d.clone());
    }
    if let Some(d) = std::env::var_os("DOTNET_ROOT") {
        return Ok(d.into());
    }
    // `dotnet` is searched in the script's tool folders first (an app started from the Finder has no Homebrew in its PATH)
    let dotnet = crate::host::Runner::from_parent()
        .and_then(|r| r.which("dotnet"))
        .unwrap_or_else(|_| PathBuf::from("dotnet"));
    let out = std::process::Command::new(dotnet)
        .arg("--list-runtimes")
        .output()
        .map_err(|e| {
            format!("no .NET runtime for the shader translator (dotnet: {e}); pass --dotnet-root")
        })?;
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some(rest) = line.trim().strip_prefix("Microsoft.NETCore.App ")
            && let Some(i) = rest.find('[')
            && let Some(root) = rest[i + 1..]
                .trim_end_matches(']')
                .strip_suffix("/shared/Microsoft.NETCore.App")
        {
            return Ok(root.into());
        }
    }
    Err("no .NET runtime for the shader translator; pass --dotnet-root".into())
}

/// The shader step's configuration from the build's plan: the engine's translator and spirv-cross, the chain version
/// and the profile's inputs digest computed as `dmsh_chain.py` does, outputs in `<work>/shader/cache/b-<V>`.
pub fn shader_config(a: &Args, b: &Build) -> Result<BuildConfig, String> {
    let tools = b.engine.join("dm/gfx/tools");
    let chain = nxb_shader::chain::version(&b.engine)?;
    let inputs = nxb_shader::chain::inputs_of(&b.engine, &b.profile.0)?;
    let sh = b.work.join("shader");
    let manifest = match &a.manifest {
        Some(f) => nxb_sched::UseManifest::read(f)?.keys().to_vec(),
        None => Vec::new(),
    };
    let xlate = a
        .xlate
        .clone()
        .unwrap_or_else(|| tools.join("dmsh_xlate_bin/dmsh_xlate"));
    Ok(BuildConfig {
        pkg: sh.join("pkg"),
        out: sh.join("cache"),
        profile: b.profile.0.clone(),
        chain,
        game: b.title.clone(),
        inputs,
        xlate: xlate.clone(),
        dotnet_root: dotnet_root(a)?,
        spirv_cross: tools.join("spirv_cross_bin/spirv-cross"),
        store: sh.join("store"),
        work: sh.join("work"),
        scratch: std::env::temp_dir().join(format!("nxb-build-{}", std::process::id())),
        jobs: a.threads.max(1) as u32,
        batch: 128,
        batch_first: 16,
        keys: Vec::new(),
        own_front: (a.own_front != "off").then(|| {
            let mut c = nxb_stages::own_front::default_config();
            if !a.own_front.is_empty() && a.own_front != "on" {
                c.verdicts = PathBuf::from(&a.own_front);
            }
            c
        }),
        on_done: None,
        on_start: None,
        order: Vec::new(),
        manifest,
        partial: a.partial,
        ios_out: a.also_ios.then(|| sh.join("ios")),
        xlate_host: nxb_stages::xlatehost::resolve(a.xlate_host.as_deref(), &xlate),
        xlate_rust: nxb_stages::xlatehost::resolve_rust(None),
    })
}

/// The archive's inputs for one architecture over the library folder the shader step writes.
fn archive_config(b: &Build, lib: &Path, arch: &str) -> Result<nxb_archive::Config, String> {
    let (out, work) = (b.work.join("pso/mac"), b.work.join("pso/mac-work"));
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let abs = |p: &Path| std::path::absolute(p).map_err(|e| e.to_string());
    let mut c = nxb_archive::Config::for_tools(
        "macos",
        arch,
        &osmin()?,
        &work.join("bases.mtl4-json"),
        &abs(&out.join(format!("{arch}.mtl4a")))?,
        &abs(&work)?,
    )?;
    c.store = abs(&c.store)?;
    let _ = lib;
    Ok(c)
}

/// Pairs per streamed group (about).
const GROUP_TARGET: usize = 400;

/// The streamed archive's workers once the shader step ended: one compiler run per slot, as many as the memory budget
/// holds for a group of twice the average size (a group is at most 4x, PROGRESS X8: about 1.9 GiB per 1,000 bases).
fn wide_workers(c: &nxb_archive::Config) -> usize {
    let slots = (c.tokens / c.cost.max(1)).max(1) as u64;
    let per = c.mem_per_batch * (2 * GROUP_TARGET as u64) / 1000 + (512 << 20);
    slots.min(c.mem_budget / per.max(1)).max(3) as usize
}

/// What the combined step did.
pub struct Outcome {
    /// The shader step's summary.
    pub shaders: String,
    /// The archive's summary (when one was built).
    pub archive: Option<String>,
    /// Why the archive failed: the script treats that as no archive for the app, not as a failed build.
    pub archive_error: Option<String>,
    /// The library folder.
    pub lib: PathBuf,
}

/// Run the shader step and, for the first `--gpu` architecture, the streamed archive beside it.
pub fn shaders_and_archive(a: &Args, b: &Build, p: &Arc<Progress>) -> Result<Outcome, String> {
    let mut cfg = shader_config(a, b)?;
    let lib = cfg.out.join(format!("b-{}", cfg.chain));
    let arch = b.gpus.first().cloned();
    let zero = b.profile.1.get("depth_mode").map(String::as_str) == Some("zero");
    let pair_files = [
        b.profile.0.join("shader_pairs.tsv"),
        b.work.join("shader/pso_pairs.tsv"),
    ];
    let text = nxb_archive::plan::pair_text(&pair_files);
    let acfg = match &arch {
        Some(arch) => Some(archive_config(b, &lib, arch)?),
        None => None,
    };
    let sinp = nxb_archive::stream::StreamInputs {
        lib_dir: std::path::absolute(&lib).map_err(|e| e.to_string())?,
        chain: cfg.chain.clone(),
        pairs: nxb_archive::plan::read_pairs(&text),
        zero,
        group_target: GROUP_TARGET,
        workers: 3,
        workers_after: acfg.as_ref().map_or(3, wide_workers),
    };
    let stream = arch
        .as_ref()
        .map(|_| Arc::new(nxb_archive::stream::Stream::new(&sinp)));
    let done = Arc::new(AtomicU64::new(0));
    {
        let (st, done, p) = (stream.clone(), done.clone(), p.clone());
        cfg.on_done = Some(OnDone(Arc::new(move |key: &str, _status: &str| {
            let n = done.fetch_add(1, Ordering::Relaxed) + 1;
            p.set(Update {
                done: Some(n),
                ..Default::default()
            });
            if let Some(s) = &st {
                s.done(key);
            }
        })));
    }
    if let Some(s) = &stream {
        cfg.order = s.key_order();
    }
    {
        // the total is reported with or without an archive: the app binary shows a percent bar and an ETA from it
        let (s, total) = (stream.clone(), p.clone());
        cfg.on_start = Some(nxb_stages::build::OnStart(Arc::new(
            move |keys: &[String]| {
                if let Some(s) = &s {
                    s.building(&keys.iter().cloned().collect());
                }
                total.set(Update {
                    total: Some(keys.len() as u64),
                    done: Some(0),
                    ..Default::default()
                });
            },
        )));
    }
    p.set(Update {
        step: Some("translating and compiling"),
        unit: Some("shaders"),
        ..Default::default()
    });
    let (shaders, archive) = std::thread::scope(
        |scope| -> Result<(String, Option<Result<String, String>>), String> {
            let job = match (&stream, acfg) {
                (Some(s), Some(c)) => {
                    let (s, sinp) = (s.clone(), sinp.clone());
                    Some(scope.spawn(move || s.run(&c, &sinp).map(|r| (r, c))))
                }
                _ => None,
            };
            let r = nxb_stages::build::build(cfg).map_err(|e| e.to_string());
            if let Some(s) = &stream {
                s.finish();
            }
            let r = r?;
            let ok = r.results.iter().filter(|x| x.1 == "ok").count();
            let mut shaders = format!(
                "{ok} of {} shaders, {}; {} translator processes",
                r.results.len(),
                r.own_note,
                r.xlate_runs
            );
            let partial = nxb_sched::Partial::read(&lib);
            if let (true, Some(st)) = (a.partial, &partial) {
                shaders = format!("{shaders}; {}", st.note());
            }
            p.say_with(
                &format!("shaders done in {:.1} s ({shaders})", r.wall.as_secs_f64()),
                vec![
                    ("xlate_starts".into(), Value::Int(r.xlate_runs as i64)),
                    (
                        "xlate_restarts".into(),
                        Value::Int(i64::from(r.xlate_restarts)),
                    ),
                    (
                        "xlate_fallbacks".into(),
                        Value::Object(
                            r.xlate_fallbacks
                                .iter()
                                .map(|(k, n)| (k.clone(), Value::Int(*n as i64)))
                                .collect(),
                        ),
                    ),
                ],
            );
            let archive = match job {
                Some(j) => {
                    p.set(Update {
                        step: Some("archive batches still running"),
                        ..Default::default()
                    });
                    let done = j
                        .join()
                        .map_err(|_| "archive thread panicked".to_string())
                        .and_then(|r| r)
                        .and_then(|((rep, bases), c)| {
                            let arch = arch.clone().unwrap_or_default();
                            write_archive_stamps(b, &lib, &c, &arch, &text, zero, bases)?;
                            Ok((rep, bases))
                        });
                    match done {
                        Ok((rep, bases)) => {
                            let tail = partial
                                .filter(|_| a.partial)
                                .map_or(String::new(), |st| format!("; {}", st.note()));
                            Some(Ok(format!(
                                "{bases} bases, {} MB, {} batches ({} reused), streamed beside the shader step{tail}",
                                rep.size >> 20,
                                rep.batches.len(),
                                rep.batches.iter().filter(|x| x.reused).count()
                            )))
                        }
                        Err(e) => Some(Err(archive_failure(&e))),
                    }
                }
                None => None,
            };
            Ok((shaders, archive))
        },
    )?;
    let (archive, archive_error) = match archive {
        Some(Ok(s)) => (Some(s), None),
        Some(Err(e)) => (None, Some(e)),
        None => (None, None),
    };
    Ok(Outcome {
        shaders,
        archive,
        archive_error,
        lib,
    })
}

/// An archive failure in words: the common one, no pair with libraries of the chain, says what it means.
pub(crate) fn archive_failure(e: &str) -> String {
    if e.contains("not enough input files") {
        "the pipeline archive has no base to pack: no shader pair has libraries of this build's chain (their `_b.json` names another chain than the folder `b-<chain>`) or the profile has no usable pairs".into()
    } else {
        e.to_string()
    }
}

/// The minimum OS of the Mac archive: this Mac's major version (`sw_vers`), as `pso_offline.py`.
pub(crate) fn osmin() -> Result<String, String> {
    std::process::Command::new("/usr/bin/sw_vers")
        .arg("-productVersion")
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .split('.')
                .next()
                .unwrap_or("27")
                .to_string()
                + ".0"
        })
        .map_err(|e| e.to_string())
}

/// `prebuilt.tsv`, `archive.tsv` and the script `bases.mtl4-json` of a streamed archive, from the whole plan as
/// `pso_offline.py` (and the non-streamed [`stage_pso`]) writes them; the streamed groups must have compiled exactly the
/// plan's bases.
pub(crate) fn write_archive_stamps(
    b: &Build,
    lib: &Path,
    c: &nxb_archive::Config,
    arch: &str,
    text: &[u8],
    zero: bool,
    bases: usize,
) -> Result<(), String> {
    use nxb_archive::plan;
    let chain = lib
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_prefix("b-"))
        .ok_or("library folder is not b-<chain>")?;
    let p = plan::plan(lib, chain, &plan::read_pairs(text), zero);
    if p.bases.len() != bases {
        return Err(format!(
            "streamed archive: {bases} bases compiled, the plan has {}",
            p.bases.len()
        ));
    }
    let out = b.work.join("pso/mac");
    let js = b.work.join("pso/mac-work/bases.mtl4-json");
    std::fs::write(&js, plan::script_text(lib, &p.bases)).map_err(|e| e.to_string())?;
    std::fs::write(out.join("prebuilt.tsv"), plan::stamp(chain, zero, text))
        .map_err(|e| e.to_string())?;
    let tsv = plan::archive_tsv(
        &[arch.to_string()],
        "macos",
        &c.osmin,
        &c.toolchain,
        bases,
        p.rest.len(),
    );
    std::fs::write(out.join("archive.tsv"), tsv).map_err(|e| e.to_string())
}

/// One architecture's pipeline archive: the plan of `pso_offline.py` (profile pairs, then the split's pso_pairs.tsv)
/// and `nxb_archive::build` (content-cut batches in the store). Output in `<work>/pso/mac`, as the NSP script's.
pub(crate) fn stage_pso(b: &Build, arch: &str) -> Result<String, String> {
    use nxb_archive::plan;
    let pairs = [
        b.profile.0.join("shader_pairs.tsv"),
        b.work.join("shader/pso_pairs.tsv"),
    ];
    let text = plan::pair_text(&pairs);
    let chain = b
        .shaders
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_prefix("b-"))
        .ok_or("library folder is not b-<chain>")?;
    let zero = b.profile.1.get("depth_mode").map(String::as_str) == Some("zero");
    let p = plan::plan(&b.shaders, chain, &plan::read_pairs(&text), zero);
    let (out, work) = (b.work.join("pso/mac"), b.work.join("pso/mac-work"));
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let js = work.join("bases.mtl4-json");
    std::fs::write(&js, plan::script_text(&b.shaders, &p.bases)).map_err(|e| e.to_string())?;
    let osmin = osmin()?;
    let c = nxb_archive::Config::for_tools(
        "macos",
        arch,
        &osmin,
        &js,
        &out.join(format!("{arch}.mtl4a")),
        &work,
    )?;
    let r = nxb_archive::build(&c)?;
    std::fs::write(out.join("prebuilt.tsv"), plan::stamp(chain, zero, &text))
        .map_err(|e| e.to_string())?;
    std::fs::write(
        out.join("archive.tsv"),
        plan::archive_tsv(
            &[arch.to_string()],
            "macos",
            &osmin,
            &c.toolchain,
            p.bases.len(),
            p.rest.len(),
        ),
    )
    .map_err(|e| e.to_string())?;
    let tail =
        nxb_sched::Partial::read(&b.shaders).map_or(String::new(), |st| format!("; {}", st.note()));
    Ok(format!(
        "{} bases, {} MB, {} batches{tail}",
        p.bases.len(),
        r.size >> 20,
        r.batches.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::archive_failure;

    #[test]
    fn archive_failures_in_words() {
        let raw = "metal-pack: ToolError { stderr_head: \"metal-pack: error: not enough input files\\n\" }";
        assert!(archive_failure(raw).starts_with("the pipeline archive has no base to pack"));
        assert_eq!(archive_failure("disk full"), "disk full");
    }
}
