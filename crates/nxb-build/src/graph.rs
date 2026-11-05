//! The stage graph of one build: inputs discovered as the NSP script finds them, one node per stage with its
//! dependencies and cache key, run level by level (nodes of one level at once).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use nxb_model::ContentKey;

use crate::Args;
use crate::events::Progress;

/// One NSP/NSZ of the folder: (title ID, version, path).
type Package = (String, u32, PathBuf);

fn packages(dir: &Path) -> Result<Vec<Package>, String> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
    {
        let name = e.file_name().to_string_lossy().into_owned();
        let lower = name.to_lowercase();
        if !(lower.ends_with(".nsp") || lower.ends_with(".nsz")) || name.starts_with("._") {
            continue;
        }
        let tags: Vec<&str> = name
            .split('[')
            .filter_map(|t| t.split(']').next())
            .collect();
        let title = tags
            .iter()
            .find(|t| t.len() == 16 && t.chars().all(|c| c.is_ascii_hexdigit()))
            .map(|t| t.to_uppercase());
        let version = tags
            .iter()
            .find_map(|t| t.strip_prefix('v').and_then(|v| v.parse::<u32>().ok()))
            .unwrap_or(0);
        if let Some(t) = title {
            out.push((t, version, e.path()));
        }
    }
    out.sort();
    Ok(out)
}

/// The game profile (`dm/games/<title>/game.tsv`): key -> values.
fn profile(engine: &Path, title: &str) -> Result<(PathBuf, BTreeMap<String, String>), String> {
    let dir = engine.join("dm/games").join(title);
    let text = std::fs::read_to_string(dir.join("game.tsv"))
        .map_err(|e| format!("no profile for {title}: {e}"))?;
    let mut m = BTreeMap::new();
    for l in text.lines().filter(|l| !l.starts_with('#')) {
        if let Some((k, v)) = l.split_once('\t') {
            m.entry(k.to_string()).or_insert_with(|| v.to_string());
        }
    }
    Ok((dir, m))
}

fn identity(p: &Path) -> String {
    std::fs::metadata(p)
        .map(|m| format!("{}:{}:{}", p.display(), m.len(), m.mtime()))
        .unwrap_or_default()
}

/// One stage of the graph.
#[derive(Debug)]
pub struct Node {
    /// Stage name as the NSP script names it.
    pub name: String,
    /// Indices of the stages it waits for.
    pub deps: Vec<usize>,
    /// Its cache key (inputs).
    pub key: ContentKey,
    /// Done already (reused outputs), and how.
    pub reused: Option<String>,
}

/// Everything a build needs to know before it starts.
#[derive(Debug)]
pub struct Build {
    /// Base title ID.
    pub title: String,
    /// Base NSP, updates (version, path).
    pub base: PathBuf,
    /// Updates.
    pub updates: Vec<(u32, PathBuf)>,
    /// The packed version (`v327680`).
    pub package: String,
    /// The versions the split reads.
    pub split_versions: Vec<String>,
    /// Work folder.
    pub work: PathBuf,
    /// The profile folder and its keys.
    pub profile: (PathBuf, BTreeMap<String, String>),
    /// The shader libraries (`b-<chain>`).
    pub shaders: PathBuf,
    /// The app.
    pub out: PathBuf,
    /// GPU architectures of the pipeline archive (empty: none).
    pub gpus: Vec<String>,
    /// The engine folder.
    pub engine: PathBuf,
    /// The stages.
    pub nodes: Vec<Node>,
}

/// Discovers the inputs and builds the graph.
pub fn plan(a: &Args) -> Result<Build, String> {
    let pk = packages(&a.nsp_dir)?;
    let base = pk
        .iter()
        .filter(|p| {
            p.0.ends_with("000")
                && a.title_id
                    .as_ref()
                    .is_none_or(|t| t.eq_ignore_ascii_case(&p.0))
        })
        .min_by_key(|p| p.1)
        .ok_or("no base game NSP in the folder")?;
    let title = base.0.clone();
    let upd_title = format!("{}800", &title[..13]);
    let updates: Vec<(u32, PathBuf)> = pk
        .iter()
        .filter(|p| p.0 == upd_title)
        .map(|p| (p.1, p.2.clone()))
        .collect();
    let engine = a
        .engine
        .clone()
        .ok_or("--engine <engine folder> is required (profiles under dm/games)")?;
    let prof = profile(&engine, &title)?;
    let newest = updates.iter().map(|u| u.0).max().unwrap_or(0);
    let package = a
        .package
        .clone()
        .or_else(|| prof.1.get("package").cloned())
        .unwrap_or_else(|| format!("v{newest}"));
    let split_versions: Vec<String> = prof
        .1
        .get("shader_versions")
        .map(|v| v.split(' ').map(str::to_string).collect())
        .unwrap_or_else(|| vec![package.clone()]);
    let work = a
        .work_dir
        .clone()
        .unwrap_or_else(|| a.nsp_dir.join(format!("{title}_build")));
    // the shader step's library folder: given (--shaders: an existing folder, the step is skipped) or the one this
    // build writes, `<work>/shader/cache/b-<V>` with V from the engine (dmsh_chain.py)
    let shaders = match &a.shaders {
        Some(s) => s.clone(),
        None => work
            .join("shader/cache")
            .join(format!("b-{}", nxb_shader::chain::version(&engine)?)),
    };
    let name = a
        .app_name
        .clone()
        .or_else(|| prof.1.get("name").cloned())
        .unwrap_or_else(|| title.clone());
    let out = a
        .out
        .clone()
        .unwrap_or_else(|| a.nsp_dir.join(format!("{name}.app")));
    let gpus: Vec<String> = match a.gpu.as_deref() {
        None | Some("none") => Vec::new(),
        Some(g) => g.split(',').map(nxb_archive::plan::arch_name).collect(),
    };
    let key = |name: &str, fields: &[(&str, String)]| {
        let mut b = ContentKey::builder(&format!("nxb-build {name} 1"));
        for (k, v) in fields {
            b = b.field_str(k, v);
        }
        b.finish()
    };
    let nsps: String = std::iter::once(identity(&base.2))
        .chain(updates.iter().map(|u| identity(&u.1)))
        .collect::<Vec<_>>()
        .join(";");
    let ex = |v: &str| work.join("extract").join(v).join("RomFS.img");
    let have_extract = split_versions
        .iter()
        .chain([&package])
        .all(|v| ex(v).is_file());
    let have_split =
        std::fs::read_dir(work.join("shader/pkg")).is_ok_and(|mut d| d.next().is_some());
    let mut nodes = vec![
        Node {
            name: "extract".into(),
            deps: vec![],
            key: key(
                "extract",
                &[
                    ("nsps", nsps.clone()),
                    ("versions", split_versions.join(" ")),
                ],
            ),
            reused: have_extract
                .then(|| "the work folder's images (kept for the next build)".into()),
        },
        Node {
            name: "shader split".into(),
            deps: vec![0],
            key: key(
                "split",
                &[
                    ("nsps", nsps),
                    ("profile", identity(&prof.0.join("game.tsv"))),
                ],
            ),
            reused: have_split.then(|| "the work folder's package files".into()),
        },
        Node {
            name: "Mac shaders".into(),
            deps: vec![1],
            key: key("shaders", &[("libraries", identity(&shaders))]),
            reused: a
                .shaders
                .as_ref()
                .map(|s| format!("{} (an existing library folder, --shaders)", s.display())),
        },
    ];
    for g in &gpus {
        nodes.push(Node {
            name: format!("Pipeline archive {g}"),
            deps: vec![2],
            key: key(
                "pso",
                &[("arch", g.clone()), ("libraries", identity(&shaders))],
            ),
            reused: None,
        });
    }
    nodes.push(Node {
        name: "Mac host".into(),
        deps: vec![],
        key: key("host", &[("work", work.display().to_string())]),
        reused: None,
    });
    let app_deps: Vec<usize> = (2..nodes.len()).collect();
    nodes.push(Node {
        name: "Mac app".into(),
        deps: app_deps,
        key: key(
            "app",
            &[
                ("out", out.display().to_string()),
                ("package", package.clone()),
            ],
        ),
        reused: None,
    });
    let mut b = Build {
        title,
        base: base.2.clone(),
        updates,
        package,
        split_versions,
        work,
        profile: prof,
        shaders,
        out,
        gpus,
        engine,
        nodes,
    };
    if a.platform == "ios" {
        // the IPA is named by the profile, beside the build folder, as the script names it
        if a.out.is_none() {
            let ipa = b
                .profile
                .1
                .get("ipa")
                .cloned()
                .unwrap_or_else(|| format!("{name}.ipa"));
            b.out = b.work.parent().unwrap_or(&a.nsp_dir).join(ipa);
        }
        b.gpus = Vec::new();
        b.nodes = crate::iosbuild::nodes(a, &b);
    }
    Ok(b)
}

/// The plan as text (`--dry-run`).
pub fn describe(b: &Build) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "game {} package {} work {}",
        b.title,
        b.package,
        b.work.display()
    );
    let _ = writeln!(s, "base {}", b.base.display());
    for (v, p) in &b.updates {
        let _ = writeln!(s, "update v{v} {}", p.display());
    }
    let _ = writeln!(s, "app {}", b.out.display());
    for (i, n) in b.nodes.iter().enumerate() {
        let deps: Vec<String> = n.deps.iter().map(|d| b.nodes[*d].name.clone()).collect();
        let _ = writeln!(
            s,
            "stage {i} {:<28} after [{}] key {}{}",
            n.name,
            deps.join(", "),
            &n.key.to_hex()[..16],
            n.reused
                .as_ref()
                .map(|r| format!(" (reused: {r})"))
                .unwrap_or_default()
        );
    }
    let _ = writeln!(
        s,
        "rewrite: Rust when the work folder has no patched_{} and the engine matches, else taken from the work folder",
        b.package
    );
    s
}

pub(crate) fn stage_extract(a: &Args, b: &Build, p: &Progress) -> Result<String, String> {
    let keys = a.keys.clone().ok_or("-k/--keys is required to extract")?;
    let out = |v: &str| b.work.join("extract").join(v);
    let pkg = b.work.join("shader/pkg");
    let req = nxb_extract::Request {
        keys,
        base: b.base.clone(),
        out_base: out("v0"),
        updates: b
            .updates
            .iter()
            .map(|(v, path)| (path.clone(), out(&format!("v{v}"))))
            .collect(),
        threads: a.threads,
        mem_limit: 1 << 30,
        cache_chunks: 512,
        digests: None,
        html: true,
        base_outputs: true,
        app: None,
        discard_images: false,
        allow_damaged: false,
        manifest: false,
        split: Some((
            b.profile.0.join("game.tsv"),
            nxb_extract::split::SplitOutputs {
                pkg,
                map: b.work.join("shader/shader_map.tsv"),
                facts: b.work.join("shader/shader_facts.tsv"),
                pairs: b.work.join("shader/shader_pairs.tsv"),
                pso_pairs: b.work.join("shader/pso_pairs.tsv"),
            },
        )),
    };
    p.say("extract and shader split in one pass (nxb-extract)");
    let s = nxb_extract::extract(&req).map_err(|e| e.to_string())?;
    let note = crate::rewrite::ensure(&b.engine, &b.profile.0, &b.work, &b.package)?;
    Ok(format!(
        "{} images{}",
        s.images.len(),
        note.map(|n| format!(", {n}")).unwrap_or_default()
    ))
}

/// The app stage: stage and sign (an in-place update of `previous`, the app as it was when the build started) with the
/// parts made ahead (`host::app::prepare`).
fn stage_app(
    a: &Args,
    b: &Build,
    previous: Option<PathBuf>,
    parts: crate::host::app::AppParts,
) -> Result<String, String> {
    let w = &b.work;
    let prof = &b.profile.1;
    let ex = w.join("extract").join(&b.package);
    let (companion, icns, lang) = (parts.companion, parts.icns, parts.lang);
    let nacp = std::fs::read(ex.join("control.nacp")).unwrap_or_default();
    let version = nxb_extract::version::display_version(&nacp);
    let name = a
        .app_name
        .clone()
        .or_else(|| nxb_extract::version::title_name(&nacp).ok())
        .unwrap_or_else(|| b.title.clone());
    let patched = w.join(format!("patched_{}", b.package));
    let fork = std::fs::read_to_string(b.engine.join("dm/fork_patches.txt")).unwrap_or_default();
    let tables = b.profile.0.clone();
    let pso_pairs = w.join("shader/pso_pairs.tsv");
    let inputs = nxb_pack::macapp::MacAppInputs {
        host: w.join("tools/dmhost"),
        companion: companion.clone(),
        patched: patched.clone(),
        romfs: ex.join("RomFS.img"),
        msl: b.shaders.clone(),
        msl_name: None,
        tables,
        font: w.join("font"),
        pso: (!b.gpus.is_empty()).then(|| w.join("pso/mac")),
        pso_pairs: pso_pairs.is_file().then_some(pso_pairs),
        html: Some(ex.join("html")),
        env_file: None,
        settings: Some(parts.settings),
        icns,
        title_id: b.title.clone(),
        name,
        version: (!version.is_empty()).then_some(version),
        bundle_id: a.bundle_id.clone(),
        category: prof.get("app_category").cloned(),
        lang: Some(lang.clone()),
        marker: prof.get("romfs_marker").cloned(),
        fork_patches: fork
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(","),
        commit: None,
        aoc: None,
        programs: Vec::new(),
    };
    let plan = nxb_pack::macapp::plan(&inputs).map_err(|e| e.to_string())?;
    let done = match previous {
        Some(prev) => {
            // the companion app comes from the previous app: keep a copy, the update rewrites it in place
            let keep = w.join("tmp/nxnative");
            if companion.starts_with(&prev) {
                std::fs::create_dir_all(w.join("tmp")).map_err(|e| e.to_string())?;
                std::fs::copy(&companion, &keep).map_err(|e| e.to_string())?;
            }
            let mut plan = plan;
            for e in &mut plan.entries {
                if e.dest == "Contents/MacOS/nxnative" && companion.starts_with(&prev) {
                    e.content = nxb_pack::appstage::Content::File(keep.clone());
                }
            }
            nxb_pack::incremental::update(&plan, &prev, &b.out, a.threads)
                .map(|r| r.to_string())
                .map_err(|e| e.to_string())
        }
        None => {
            let o = nxb_pack::appstage::StageOptions {
                threads: a.threads,
                ..Default::default()
            };
            nxb_pack::appstage::stage_and_sign(&plan, &b.out, &o)
                .map(|r| format!("{} files staged and signed", r.files))
                .map_err(|e| e.to_string())
        }
    }?;
    // a partial library folder travels into the app as `msl/<b-V>/_b_partial.json`, the summary says so too
    Ok(match nxb_sched::Partial::read(&b.shaders) {
        Some(st) => format!("{done}; {}", st.note()),
        None => done,
    })
}

/// Runs the build. Stages overlap where the graph allows ([`crate::order`]): the Mac host from the start, the app's
/// companion app, icon and settings once the extract and the host are done (beside the shader step), the first
/// archive streamed beside the shader step ([`crate::pipeline`]); the events stay those of a run one after another.
pub fn run(a: &Args, p: &std::sync::Arc<Progress>) -> Result<(), String> {
    use crate::order::{Stage, Work};
    use std::sync::Mutex;
    let b = plan(a)?;
    if a.dry_run {
        print!("{}", describe(&b));
        return Ok(());
    }
    if a.platform == "ios" {
        return crate::iosbuild::run(a, &b, p);
    }
    p.plan(b.nodes.len());
    let b = &b;
    let streamed: Mutex<Option<String>> = Mutex::new(None);
    let parts: Mutex<Option<crate::host::app::AppParts>> = Mutex::new(None);
    let (streamed, parts) = (&streamed, &parts);
    let previous = b.out.is_dir().then(|| b.out.clone());
    let host = b.nodes.iter().position(|n| n.name == "Mac host");
    let mut stages: Vec<Stage<'_>> = Vec::new();
    for (i, n) in b.nodes.iter().enumerate() {
        let mut deps = n.deps.clone();
        let work = match (&n.reused, i, n.name.as_str()) {
            (Some(r), _, _) => Work::Reused(format!("reused: {r}")),
            (_, 0, _) => Work::Turn(Box::new(move || stage_extract(a, b, p))),
            (_, 1, _) => Work::Reused("in the extract's pass".into()),
            (_, 2, _) => Work::Turn(Box::new(move || {
                // the shader step and the first architecture's archive together (crate::pipeline)
                let o = crate::pipeline::shaders_and_archive(a, b, p)?;
                *streamed.lock().map_err(|_| "lock")? = o.archive;
                Ok(o.shaders)
            })),
            (_, _, name) if name.starts_with("Pipeline archive") => {
                let arch = name.trim_start_matches("Pipeline archive ");
                Work::Turn(Box::new(move || {
                    match streamed.lock().map_err(|_| "lock")?.take() {
                        Some(s) => Ok(s),
                        None => crate::pipeline::stage_pso(b, arch),
                    }
                }))
            }
            (_, _, "Mac host") => Work::Early(Box::new(move || {
                crate::host::machost::stage(&b.engine, &b.work, &b.title)
            })),
            _ => {
                // the app's parts (companion app, icon, settings) need the extract and the host only: made ahead
                let prev = previous.clone();
                stages.push(Stage {
                    name: None,
                    deps: std::iter::once(0).chain(host).collect(),
                    work: Work::Early(Box::new(move || {
                        let made = crate::host::app::prepare(a, b, prev.as_deref())?;
                        *parts.lock().map_err(|_| "lock")? = Some(made);
                        Ok(String::new())
                    })),
                });
                deps.push(stages.len() - 1);
                let prev = previous.clone();
                Work::Turn(Box::new(move || {
                    let made = parts.lock().map_err(|_| "lock")?.take();
                    stage_app(a, b, prev, made.ok_or("the app's parts were not made")?)
                }))
            }
        };
        stages.push(Stage {
            name: Some(n.name.clone()),
            deps,
            work,
        });
    }
    crate::order::run(p, stages)?;
    p.say(&format!("done: {}", b.out.display()));
    Ok(())
}
