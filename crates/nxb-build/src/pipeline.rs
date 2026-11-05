//! The shader step and the pipeline archive in one process graph: `nxb_stages::build::build` is called as a library
//! (no `shaderbuild` process) and reports every finished shader, the archive's pair groups (`nxb_archive::stream`) start
//! their `metal-tt` batches as soon as their shaders are done, on worker threads beside the shader step, and the shader
//! step translates the archive's shaders first so the groups complete one after another.

use crate::Args;
use crate::events::{Progress, Update};
use crate::graph::Build;
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
    let out = std::process::Command::new("dotnet")
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
    let inputs = nxb_shader::chain::inputs(&b.profile.0)?;
    let sh = b.work.join("shader");
    Ok(BuildConfig {
        pkg: sh.join("pkg"),
        out: sh.join("cache"),
        profile: b.profile.0.clone(),
        chain,
        game: b.title.clone(),
        inputs,
        xlate: a
            .xlate
            .clone()
            .unwrap_or_else(|| tools.join("dmsh_xlate_bin/dmsh_xlate")),
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
        ios_out: a.also_ios.then(|| sh.join("ios")),
        xlate_host: None,
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
        &crate::graph::osmin()?,
        &work.join("bases.mtl4-json"),
        &abs(&out.join(format!("{arch}.mtl4a")))?,
        &abs(&work)?,
    )?;
    c.store = abs(&c.store)?;
    let _ = lib;
    Ok(c)
}

/// What the combined step did.
pub struct Outcome {
    /// The shader step's summary.
    pub shaders: String,
    /// The archive's summary (when one was built).
    pub archive: Option<String>,
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
    let sinp = nxb_archive::stream::StreamInputs {
        lib_dir: std::path::absolute(&lib).map_err(|e| e.to_string())?,
        chain: cfg.chain.clone(),
        pairs: nxb_archive::plan::read_pairs(&text),
        zero,
        group_target: 400,
        workers: 3,
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
        let s = s.clone();
        let total = p.clone();
        cfg.on_start = Some(nxb_stages::build::OnStart(Arc::new(
            move |keys: &[String]| {
                s.building(&keys.iter().cloned().collect());
                total.set(Update {
                    total: Some(keys.len() as u64),
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
        |scope| -> Result<(String, Option<String>), String> {
            let job = match (&stream, &arch) {
                (Some(s), Some(arch)) => {
                    let c = archive_config(b, &lib, arch)?;
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
            let shaders = format!(
                "{ok} of {} shaders, {}; {} translator processes",
                r.results.len(),
                r.own_note,
                r.xlate_runs
            );
            p.say(&format!(
                "shaders done in {:.1} s ({shaders})",
                r.wall.as_secs_f64()
            ));
            let archive = match job {
                Some(j) => {
                    p.set(Update {
                        step: Some("archive batches still running"),
                        ..Default::default()
                    });
                    let ((rep, bases), c) = j.join().map_err(|_| "archive thread panicked")??;
                    let arch = arch.clone().unwrap_or_default();
                    crate::graph::write_archive_stamps(b, &lib, &c, &arch, &text, zero, bases)?;
                    Some(format!(
                        "{bases} bases, {} MB, {} batches ({} reused), streamed beside the shader step",
                        rep.size >> 20,
                        rep.batches.len(),
                        rep.batches.iter().filter(|x| x.reused).count()
                    ))
                }
                None => None,
            };
            Ok((shaders, archive))
        },
    )?;
    Ok(Outcome {
        shaders,
        archive,
        lib,
    })
}
