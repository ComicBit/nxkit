//! `shaderbuild recompile <b-dir>`: the P1a milestone of design-v2. Rebuilds the Mac metallibs of an existing output
//! folder from the final MSL the Python chain left there (`<key>_b.metal`) through the new runtime (tool adapters with the
//! exact environment and the engine's precompiled header, the store, the work-stealing executor with single-flight) and
//! compares each result byte for byte with Python's `<key>_b.metallib` (and `<key>_bx.metallib`).
//!
//! A pixel shader with side entry points (`nxb_shader::side::unit`) is compiled as its unit and the library cut in two,
//! as `dmsh_build.ps_libraries` does, when that fails it falls back to the plain source, as Python does.

use crate::recompile_build::{Built, build, compare};
use nxb_error::{Fatal, ToolError};
use nxb_model::ContentKey;
use nxb_obs::{ProgressCounter, Sink};
use nxb_sched::{Budget, Claim, Executor, Report, SingleFlight, Task};
use nxb_shader::{pch, side, target::MAC};
use nxb_store::Store;
use nxb_tool::{Io, MetalTools, ProcessRegistry, Resolver, SdkRoots, StepCost};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// What to rebuild and how.
#[derive(Clone, Debug)]
pub struct RecompileConfig {
    /// A `b-<V>/` folder of the Python chain.
    pub b_dir: PathBuf,
    /// The content-addressed store root.
    pub store_root: PathBuf,
    /// Private work folder (temp files, header, report).
    pub work_dir: PathBuf,
    /// Concurrent compile processes (tokens).
    pub jobs: u32,
    /// Take every n-th shader of the sorted list (1 = all).
    pub every: usize,
    /// At most this many shaders.
    pub limit: Option<usize>,
    /// AIR via files or pipes.
    pub io: Io,
    /// Use the precompiled header (as the engine does unless `DMSH_PCH=0`).
    pub use_pch: bool,
    /// Store durability (`Full` fsyncs every write).
    pub durability: nxb_store::Durability,
    /// Build pixel shaders' side entry points (`dmsh_side`), off compiles every shader plain.
    pub side: bool,
    /// Also build and compare the variant libraries `_bp`, `_bv`, `_bf` and their reflection fields.
    pub variants: bool,
    /// The own front end (off when `None`).
    pub own_front: Option<crate::own_front::OwnFrontConfig>,
}

/// How one shader came out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Same bytes as Python's library (both libraries for a side shader).
    Identical,
    /// Different bytes, or a library one side has and the other has not.
    Different,
    /// No library: the tool failed (message).
    Failed(String),
    /// Python has no library for this key.
    NoReference,
}

/// One shader's line in the report.
#[derive(Clone, Debug)]
pub struct Row {
    /// Shader key.
    pub key: String,
    /// Result.
    pub outcome: Outcome,
    /// How the library was obtained: `compiled`, `store` (earlier run) or `joined` (single-flight follower).
    pub source: &'static str,
    /// `plain` or `side` (unit compile and cut) for the main library.`bp`, `bv`, `bf` for a variant.
    pub kind: &'static str,
    /// Source size (the unit's for a side shader).
    pub msl_bytes: u64,
    /// `metal` step (zero unless compiled).
    pub compile: StepCost,
    /// `metallib` step (zero unless compiled).
    pub link: StepCost,
    /// Building the unit and cutting the library.
    pub side: Duration,
    /// Writing the libraries into the store.
    pub store: Duration,
    /// Hashing Python's libraries and comparing.
    pub compare: Duration,
    /// Whole task wall time.
    pub task_wall: Duration,
}

/// The run's result.
#[derive(Debug)]
pub struct RecompileReport {
    /// One row per shader, sorted by key.
    pub rows: Vec<Row>,
    /// Executor statistics.
    pub exec: Report,
    /// Wall time of the compile phase.
    pub wall: Duration,
    /// Setup time (tool resolution, header).
    pub setup: Duration,
    /// Whether the header was used.
    pub pch: bool,
    /// Own front end: libraries made, fallbacks to Apple, back-half CPU, verdict summary.
    pub own: Option<(u64, u64, Duration, String)>,
    /// Why the own front end is on or off.
    pub own_note: String,
}

pub(crate) struct Shared {
    pub(crate) cfg: RecompileConfig,
    pub(crate) tools: Arc<MetalTools>,
    pub(crate) own: Option<crate::own_front::OwnFront>,
    pub(crate) pch: Option<PathBuf>,
    pub(crate) store: Store,
    pub(crate) compiler_id: String,
    pub(crate) flights: SingleFlight<ContentKey, Built>,
    rows: Mutex<Vec<Row>>,
    progress: ProgressCounter,
}

/// The `<key>_b.metal` inputs of a folder, sorted, sampled.
pub fn inputs(b_dir: &Path, every: usize, limit: Option<usize>) -> std::io::Result<Vec<String>> {
    let mut keys: Vec<String> = fs::read_dir(b_dir)?
        .flatten()
        .filter_map(|e| {
            e.file_name()
                .to_str()
                .and_then(|n| n.strip_suffix("_b.metal"))
                .map(str::to_string)
        })
        .collect();
    keys.sort();
    let mut keys: Vec<String> = keys.into_iter().step_by(every.max(1)).collect();
    if let Some(l) = limit {
        keys.truncate(l);
    }
    Ok(keys)
}

fn fatal_tool(e: ToolError) -> Fatal {
    match e.env.clone() {
        Some(env) => Fatal::Env(env),
        None => Fatal::Internal(e.to_string()),
    }
}

fn fatal_io(what: &str, e: impl std::fmt::Display) -> Fatal {
    Fatal::Internal(format!("{what}: {e}"))
}

/// Run the recompile. Fatal errors: toolchain or store not usable.
pub fn run(cfg: RecompileConfig, sink: Arc<dyn Sink>) -> Result<RecompileReport, Fatal> {
    let t0 = Instant::now();
    let keys = inputs(&cfg.b_dir, cfg.every, cfg.limit).map_err(|e| fatal_io("list inputs", e))?;
    let tmp = cfg.work_dir.join("tmp");
    let pch_dir = cfg.work_dir.join("pch");
    for d in [&tmp, &pch_dir] {
        fs::create_dir_all(d).map_err(|e| fatal_io("create work folder", e))?;
    }
    let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from);
    let registry = Arc::new(ProcessRegistry::new());
    let resolver = Resolver {
        xcrun: PathBuf::from("/usr/bin/xcrun"),
        developer_dir: std::env::var_os("DEVELOPER_DIR"),
        home: home.clone(),
    };
    let roots = SdkRoots::resolve(&resolver, &[MAC.target], &registry).map_err(fatal_tool)?;
    let env = roots
        .apple_env(MAC.target, &tmp, &home)
        .map_err(|e| fatal_io("environment", e))?;
    let tools = Arc::new(
        MetalTools::resolve(&resolver, MAC.target, env, tmp, registry.clone())
            .map_err(fatal_tool)?,
    );
    let (ver, build_v) = resolver
        .sdk_version(MAC.target, &registry)
        .map_err(fatal_tool)?;
    let compiler_id = tools.compiler_id(&ver, &build_v).map_err(fatal_tool)?;
    let (own_cfg, own_note) = crate::own_front::resolve(cfg.own_front.as_ref(), &compiler_id);
    let own = match &own_cfg {
        Some(o) => Some(
            crate::own_front::OwnFront::open(o, &tools, &resolver)
                .map_err(|e| fatal_io("own front end", e))?,
        ),
        None => None,
    };
    let pch = if cfg.use_pch {
        tools
            .build_pch(
                &MAC.pch_flags(),
                MAC.compile_flags,
                &pch::header(),
                pch::PROBE,
                &pch_dir,
            )
            .map_err(fatal_tool)?
    } else {
        None
    };
    let mut store = Store::open(&cfg.store_root).map_err(|e| fatal_io("open store", e))?;
    store.set_durability(cfg.durability);
    let setup = t0.elapsed();
    let progress = ProgressCounter::new(
        sink,
        "Mac shaders",
        "shaders",
        keys.len() as u64,
        Duration::from_secs(10),
    );
    let jobs = cfg.jobs;
    let shared = Arc::new(Shared {
        cfg,
        tools,
        own,
        pch,
        store,
        compiler_id,
        flights: SingleFlight::new(),
        rows: Mutex::new(Vec::with_capacity(keys.len())),
        progress,
    });
    let mut tasks: Vec<Task> = Vec::with_capacity(keys.len() * 2);
    for k in keys {
        let s = shared.clone();
        let k2 = k.clone();
        tasks.push(Task::new(move |_| shader_task(&s, k)).mem(150 << 20));
        if shared.cfg.variants {
            let s = shared.clone();
            tasks.push(
                Task::new(move |_| crate::recompile_variants::variant_tasks(&s, k2)).mem(150 << 20),
            );
        }
    }
    let budget = Budget::fixed(jobs).with_memory(Budget::default_memory());
    let t1 = Instant::now();
    let exec = Executor::new(budget).run(tasks);
    let wall = t1.elapsed();
    shared.progress.finish();
    let mut rows = std::mem::take(&mut *shared.rows.lock().unwrap_or_else(PoisonError::into_inner));
    rows.sort_by(|a, b| a.key.cmp(&b.key));
    let own = shared.own.as_ref().map(|o| {
        (
            o.own.load(Ordering::Relaxed),
            o.fallback.load(Ordering::Relaxed),
            Duration::from_nanos(o.cpu_ns.load(Ordering::Relaxed)),
            o.describe(),
        )
    });
    Ok(RecompileReport {
        own,
        own_note,
        rows,
        exec,
        wall,
        setup,
        pch: shared.pch.is_some(),
    })
}

pub(crate) fn push(s: &Shared, row: Row) {
    s.rows
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(row);
    s.progress.add(1);
}

/// A row with no step costs yet.
pub(crate) fn row(key: String, outcome: Outcome, source: &'static str, t: Instant) -> Row {
    Row {
        key,
        outcome,
        source,
        kind: "plain",
        msl_bytes: 0,
        compile: StepCost::default(),
        link: StepCost::default(),
        side: Duration::ZERO,
        store: Duration::ZERO,
        compare: Duration::ZERO,
        task_wall: t.elapsed(),
    }
}

fn shader_task(s: &Arc<Shared>, key: String) {
    let t = Instant::now();
    let msl = match fs::read_to_string(s.cfg.b_dir.join(format!("{key}_b.metal"))) {
        Ok(m) => m,
        Err(e) => return push(s, row(key, Outcome::Failed(e.to_string()), "compiled", t)),
    };
    let tu = Instant::now();
    let unit = if s.cfg.side {
        side::unit(&msl).ok()
    } else {
        None
    };
    let unit_time = tu.elapsed();
    let mut kb = ContentKey::builder("nxb.metallib.v2")
        .field_str("record", &MAC.record())
        .field_str("compiler", &s.compiler_id)
        .field("msl", msl.as_bytes());
    if let Some(o) = &s.own {
        // a side shader compiles its unit source first and the plain MSL as fallback: both decide the tier
        let mut srcs = vec![msl.as_bytes()];
        if let Some(u) = &unit {
            srcs.insert(0, u.source.as_bytes());
        }
        kb = kb.field_str("frontend", &o.key_field_of(&srcs));
    }
    if let Some(u) = &unit {
        kb = kb.field_str("side", &u.side().join(","));
    }
    let ckey = kb.finish();
    let follower = {
        let (s2, key2) = (s.clone(), key.clone());
        move |b: Option<&Built>| {
            let out = match b {
                Some(b) => compare(&s2, &key2, b),
                None => Outcome::Failed("leader abandoned".into()),
            };
            let mut r = row(key2, out, "joined", t);
            if matches!(b, Some(Ok(l)) if l.bx.is_some()) {
                r.kind = "side";
            }
            push(&s2, r);
        }
    };
    let Claim::Leader(leader) = s.flights.claim(ckey, follower) else {
        return;
    };
    let (built, mut r) = build(s, ckey, &msl, unit.as_ref(), &key, t);
    r.side += unit_time;
    let tc = Instant::now();
    r.outcome = compare(s, &key, &built);
    r.compare = tc.elapsed();
    leader.complete(built);
    r.task_wall = t.elapsed();
    push(s, r);
}
