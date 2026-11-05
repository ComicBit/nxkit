//! `shaderbuild build`: the whole Mac shader step from the split's package records, streaming on the executor:
//! translation batches → per-shader finish (spirv-cross, passes, entry points, reflection) → libraries and variants
//! through the store → outputs, then `_b_build.json`, `inputs.tsv` and the guard tables. Paired vertex shaders run in a
//! second phase because `vs_pad` reads their pixel shaders' final sources (as `dmsh_build.py`).

use crate::build_finish::Shared;
use crate::inputs::ShaderInputs;
use nxb_error::Fatal;
use nxb_pyjson::{Value, dumps};
use nxb_sched::{Budget, Executor, Report, SingleFlight, Task};
use nxb_shader::{pch, target::MAC};
use nxb_store::{Durability, Store};
use nxb_tool::{
    EnvPolicy, Io, MetalTools, ProcessRegistry, ProcessTool, ProcessTranslator, Resolver, SdkRoots,
    ToolId, XlateConfig,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

pub use crate::build_hooks::{DoneFn, OnDone, OnStart, StartFn};

/// Inputs and tools of a build.
#[derive(Clone, Debug)]
pub struct BuildConfig {
    /// The split's package folder (`<stage>_<group>.bin`).
    pub pkg: PathBuf,
    /// The output cache folder, outputs go to `<out>/b-<chain>/`.
    pub out: PathBuf,
    /// The game profile folder (`shader_map.tsv`, `shader_facts.tsv`, `shader_pairs.tsv`).
    pub profile: PathBuf,
    /// Chain version V (taken as given until the Rust chain-files list exists).
    pub chain: String,
    /// Game id and profile inputs digest for `inputs.tsv`.
    pub game: String,
    /// Profile inputs digest.
    pub inputs: String,
    /// `dmsh_xlate`.
    pub xlate: PathBuf,
    /// .NET root for the translator.
    pub dotnet_root: PathBuf,
    /// `spirv-cross`.
    pub spirv_cross: PathBuf,
    /// Store root.
    pub store: PathBuf,
    /// Private work folder (header, guard tables, timeline).
    pub work: PathBuf,
    /// Folder for the short-lived files (translator in/out, spirv-cross input, temp AIR): best on the internal disk.
    /// Its `tmp/` and `x/` are removed at the end.
    pub scratch: PathBuf,
    /// Tokens.
    pub jobs: u32,
    /// Translation batch size after the first ones.
    pub batch: usize,
    /// Size of the first `jobs` translation batches (small, so every token starts at once).
    pub batch_first: usize,
    /// Only these keys (empty: all).
    pub keys: Vec<String>,
    /// The own front end (off when `None`).
    pub own_front: Option<crate::own_front::OwnFrontConfig>,
    /// Per-shader completion callback.
    pub on_done: Option<OnDone>,
    /// The key list, once known.
    pub on_start: Option<OnStart>,
    /// Drive the translator as a persistent host with these arguments (`HostTranslator`).`None`: a process per batch.
    pub xlate_host: Option<Vec<String>>,
    /// Also build the iOS libraries (same sources, `ios_metal.py` convention) into `<ios_out>/b-<chain>/`.
    pub ios_out: Option<PathBuf>,
    /// Keys to translate first, in this order (empty: pixel and compute shaders, then paired vertex shaders).
    pub order: Vec<String>,
}

/// One line of the timeline: what ran, when, how long.
#[derive(Clone, Debug)]
pub struct Span {
    /// `translate` or `finish`.
    pub what: &'static str,
    /// Key (first key of a batch).
    pub key: String,
    /// Start since the build began.
    pub start: Duration,
    /// Wall time.
    pub wall: Duration,
}

/// The build's result.
#[derive(Debug)]
pub struct BuildReport {
    /// `(key, st, err)` in `_b_build.json` order.
    pub results: Vec<(String, String, String)>,
    /// Executor reports of both phases.
    pub phases: Vec<Report>,
    /// Timeline.
    pub spans: Vec<Span>,
    /// Wall time.
    pub wall: Duration,
    /// Own front end: libraries made, fallbacks to Apple.
    pub own: Option<(u64, u64)>,
    /// Why the own front end is on or off.
    pub own_note: String,
    /// Translator processes started (.NET launches).
    pub xlate_runs: u64,
    /// Translations answered from the store (early cutoff).
    pub xlate_hits: u64,
}

pub(crate) fn fatal(what: &str, e: impl std::fmt::Display) -> Fatal {
    Fatal::Internal(format!("{what}: {e}"))
}

/// Run the build.
pub fn build(cfg: BuildConfig) -> Result<BuildReport, Fatal> {
    let t0 = Instant::now();
    let out = cfg.out.join(format!("b-{}", cfg.chain));
    let tmp = cfg.scratch.join("tmp");
    for d in [&out, &tmp, &cfg.work.join("pch"), &cfg.scratch.join("x")] {
        fs::create_dir_all(d).map_err(|e| fatal("create folder", e))?;
    }
    if cfg.keys.is_empty() {
        let _ = fs::remove_file(out.join("_b_cas.txt"));
    }
    let inputs =
        ShaderInputs::load(&cfg.pkg, &cfg.profile).map_err(|e| fatal("profile tables", e))?;
    let pairs = crate::build_shared::pairs(&cfg);
    let keys: Vec<String> = if cfg.keys.is_empty() {
        inputs.loc.keys().cloned().collect()
    } else {
        let want: BTreeSet<String> = cfg.keys.iter().cloned().collect();
        let mut k: BTreeSet<String> = want.clone();
        for v in &want {
            for p in pairs.get(v).into_iter().flatten() {
                if inputs.loc.contains_key(p) {
                    k.insert(p.clone());
                }
            }
        }
        k.into_iter().collect()
    };
    // A full build starts from no shader outputs (dmsh_build.py removes them first).
    if let Ok(rd) = fs::read_dir(&out) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            let is_out = n.len() > 18
                && n.as_bytes()[..16].iter().all(|c| c.is_ascii_hexdigit())
                && n[16..].starts_with("_b");
            if is_out && (cfg.keys.is_empty() || keys.iter().any(|k| n.starts_with(k.as_str()))) {
                let _ = fs::remove_file(e.path());
            }
        }
    }
    let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from);
    let registry = Arc::new(ProcessRegistry::new());
    let resolver = Resolver {
        xcrun: "/usr/bin/xcrun".into(),
        developer_dir: std::env::var_os("DEVELOPER_DIR"),
        home: home.clone(),
    };
    let roots =
        SdkRoots::resolve(&resolver, &[MAC.target], &registry).map_err(|e| fatal("SDK", e))?;
    let env = roots
        .apple_env(MAC.target, &tmp, &home)
        .map_err(|e| fatal("environment", e))?;
    let tools = Arc::new(
        MetalTools::resolve(&resolver, MAC.target, env, tmp.clone(), registry.clone())
            .map_err(|e| fatal("metal", e))?,
    );
    let (ver, bld) = resolver
        .sdk_version(MAC.target, &registry)
        .map_err(|e| fatal("SDK version", e))?;
    let cid = tools
        .compiler_id(&ver, &bld)
        .map_err(|e| fatal("compiler id", e))?;
    let (own_cfg, own_note) = crate::own_front::resolve(cfg.own_front.as_ref(), &cid);
    let own = match &own_cfg {
        Some(o) => Some(
            crate::own_front::OwnFront::open(o, &tools, &resolver)
                .map_err(|e| fatal("own front end", e))?,
        ),
        None => None,
    };
    let mut ident = b"dmsh_mtlcache 2\0".to_vec();
    ident.extend_from_slice(MAC.record().as_bytes());
    ident.push(0);
    ident.extend_from_slice(cid.as_bytes());
    let pchp = tools
        .build_pch(
            &MAC.pch_flags(),
            MAC.compile_flags,
            &pch::header(),
            pch::PROBE,
            &cfg.work.join("pch"),
        )
        .map_err(|e| fatal("header", e))?;
    let ios = match &cfg.ios_out {
        Some(d) => Some(crate::build_shared::ios_side(
            &cfg, d, &resolver, &registry, &home, &tmp,
        )?),
        None => None,
    };
    let xenv = EnvPolicy::translator(&home, &tmp, Some(cfg.dotnet_root.as_os_str()), None, None)
        .and_then(|e| {
            e.set(
                "PATH",
                format!("{}:/usr/bin:/bin", cfg.dotnet_root.display()),
            )
        })
        .map_err(|e| fatal("translator environment", e))?;
    let xid = ToolId {
        name: "dmsh_xlate".into(),
        path: cfg.xlate.clone(),
        version: String::new(),
    };
    // a persistent host when asked for (`xlate_host`: its arguments), else a process per batch
    let translator = match &cfg.xlate_host {
        Some(args) => crate::build_finish::Xlate::Host(nxb_tool::HostTranslator::new(
            xid,
            nxb_tool::HostConfig {
                program: cfg.xlate.clone(),
                args: args.clone(),
                env: xenv,
                per_job: Duration::from_secs(60),
            },
        )),
        None => crate::build_finish::Xlate::Batch(ProcessTranslator::new(
            xid,
            registry.clone(),
            XlateConfig::new(cfg.scratch.join("x"), xenv),
        )),
    };
    let sc = ProcessTool::new(
        ToolId {
            name: "spirv-cross".into(),
            path: cfg.spirv_cross.clone(),
            version: String::new(),
        },
        registry.clone(),
    );
    let mut store = Store::open(&cfg.store).map_err(|e| fatal("store", e))?;
    store.set_durability(Durability::Relaxed);
    let (translator, sc_id) =
        crate::xlate_cache::open(translator, &cfg).map_err(|e| fatal("translation cache", e))?;
    let shared = Arc::new(Shared {
        out: out.clone(),
        work: cfg.work.clone(),
        scratch: cfg.scratch.clone(),
        chain: cfg.chain.clone(),
        ident: nxb_model::sha256(&ident).to_hex(),
        compiler_id: cid,
        tools,
        own,
        pch: pchp,
        io: Io::Pipes,
        store,
        sc,
        translator,
        sc_id,
        inputs,
        pairs,
        flights: SingleFlight::new(),
        ios,
        gates: Mutex::new(BTreeMap::new()),
        dependents: Mutex::new(BTreeMap::new()),
        results: Mutex::new(BTreeMap::new()),
        on_done: cfg.on_done.clone(),
        used: Mutex::new(BTreeSet::new()),
        spans: Mutex::new(Vec::new()),
        t0,
        n: Default::default(),
    });
    if let Some(f) = &cfg.on_start {
        (f.0)(&keys);
    }
    let (phase_a, phase_b): (Vec<String>, Vec<String>) = keys
        .iter()
        .cloned()
        .partition(|k| !shared.pairs.contains_key(k));
    // One streaming run (design-v2 4): a paired vertex shader finishes as soon as it is translated and its pixel
    // shaders are done (vs_pad reads their final sources), not after a global barrier as in dmsh_build.py.
    {
        let building: BTreeSet<&String> = keys.iter().collect();
        let mut gates = shared.gates.lock().unwrap_or_else(PoisonError::into_inner);
        let mut deps = shared
            .dependents
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        for v in &phase_b {
            let partners: Vec<&String> = shared.pairs[v]
                .iter()
                .filter(|p| building.contains(p) && !shared.pairs.contains_key(*p))
                .collect();
            gates.insert(
                v.clone(),
                crate::build_finish::Gate {
                    remaining: 1 + partners.len(),
                    out: None,
                },
            );
            for p in partners {
                deps.entry(p.clone()).or_default().push(v.clone());
            }
        }
    }
    // Batches: small ones first so every token gets work at once, then large ones (one .NET start per batch).
    let mut tasks = Vec::new();
    let mut order: Vec<&String> = phase_a.iter().chain(&phase_b).collect();
    if !cfg.order.is_empty() {
        // a consumer's order first (the archive's pair groups), the rest after, each part stable
        let pos: BTreeMap<&String, usize> =
            cfg.order.iter().enumerate().map(|(i, k)| (k, i)).collect();
        order.sort_by_key(|k| pos.get(k).copied().unwrap_or(usize::MAX));
    }
    let (small, large) = (cfg.batch_first.max(1), cfg.batch.max(1));
    let mut at = 0;
    let mut n = 0usize;
    while at < order.len() {
        let size = if n < cfg.jobs as usize { small } else { large };
        let ks: Vec<String> = order[at..(at + size).min(order.len())]
            .iter()
            .map(|k| (*k).clone())
            .collect();
        at += ks.len();
        let s = shared.clone();
        let prio = -(n as i64);
        tasks.push(
            Task::new(move |ctx| crate::build_xlate::translate_batch(&s, ks, ctx))
                .prio(prio)
                .mem(500 << 20),
        );
        n += 1;
    }
    let phases = vec![
        Executor::new(Budget::fixed(cfg.jobs).with_memory(Budget::default_memory())).run(tasks),
    ];
    let results_map = std::mem::take(
        &mut *shared
            .results
            .lock()
            .unwrap_or_else(PoisonError::into_inner),
    );
    let mut results = Vec::new();
    for k in phase_a.iter().chain(&phase_b) {
        let (st, err) = results_map
            .get(k)
            .cloned()
            .unwrap_or_else(|| ("internal error".into(), "no result".into()));
        results.push((k.clone(), st, err));
    }
    let bb = Value::List(
        results
            .iter()
            .map(|(k, st, e)| {
                Value::obj([
                    ("key", Value::str(k.clone())),
                    ("st", Value::str(st.clone())),
                    ("err", Value::str(e.clone())),
                ])
            })
            .collect(),
    );
    fs::write(out.join("_b_build.json"), dumps(&bb)).map_err(|e| fatal("_b_build.json", e))?;
    if cfg.keys.is_empty() {
        let stamp = format!(
            "# the game and the profile inputs this library was built from (dm/gfx/tools/dmsh_chain.py inputs)\ngame\t{}\ninputs\t{}\n",
            cfg.game, cfg.inputs
        );
        fs::write(out.join("inputs.tsv"), stamp).map_err(|e| fatal("inputs.tsv", e))?;
        // `_b_cas.txt`: the cache entries this full build used, one per line, sorted. Python lists its CAS digests there
        // (its pruner keeps those), ours are store keys, so a Python pruner sees none of its entries in use, which is
        // true for a library built here.
        let used = shared.used.lock().unwrap_or_else(PoisonError::into_inner);
        let text: String = used.iter().map(|k| format!("{k}\n")).collect();
        fs::write(out.join("_b_cas.txt"), text).map_err(|e| fatal("_b_cas.txt", e))?;
        crate::guard_tables::write(&out, &cfg.work.join("tables"), cfg.jobs)
            .map_err(|e| fatal("guard tables", e))?;
    }
    for d in ["tmp", "x"] {
        let _ = fs::remove_dir_all(cfg.scratch.join(d));
    }
    let _ = fs::remove_dir(&cfg.scratch);
    let spans = std::mem::take(&mut *shared.spans.lock().unwrap_or_else(PoisonError::into_inner));
    let own = shared.own.as_ref().map(|o| {
        use std::sync::atomic::Ordering::Relaxed;
        (o.own.load(Relaxed), o.fallback.load(Relaxed))
    });
    Ok(BuildReport {
        own,
        own_note,
        xlate_runs: shared.translator.inner().runs(),
        xlate_hits: shared
            .translator
            .hits
            .load(std::sync::atomic::Ordering::Relaxed),
        results,
        phases,
        spans,
        wall: t0.elapsed(),
    })
}
