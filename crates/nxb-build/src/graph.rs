//! The stage graph of one build: inputs discovered as the NSP script finds them, one node per stage with its
//! dependencies and cache key, run level by level (nodes of one level at once).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use nxb_model::ContentKey;

use crate::Args;
use crate::events::Progress;

/// The game profile `dir/game.tsv`: key -> values. A provisional profile that the survey has not written yet reads
/// as no keys (`missing_ok`).
fn profile_at(dir: &Path, missing_ok: bool) -> Result<(PathBuf, BTreeMap<String, String>), String> {
    let text = match std::fs::read_to_string(dir.join("game.tsv")) {
        Ok(t) => t,
        Err(_) if missing_ok => String::new(),
        Err(e) => return Err(format!("{}: {e}", dir.join("game.tsv").display())),
    };
    let mut m = BTreeMap::new();
    for l in text.lines().filter(|l| !l.starts_with('#')) {
        if let Some((k, v)) = l.split_once('\t') {
            m.entry(k.to_string()).or_insert_with(|| v.to_string());
        }
    }
    Ok((dir.to_path_buf(), m))
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
    /// What the game is read from.
    pub src: crate::inputs::Inputs,
    /// The profile is a provisional one (`<work>/profile/<title>`, or `--profile-out`): the engine has none for the
    /// game or `--provisional` asked for one.
    pub prov: bool,
}

impl Build {
    /// A build with nothing but a work folder and a provisional profile folder.
    #[cfg(test)]
    pub(crate) fn for_test(work: PathBuf, profile: PathBuf) -> Build {
        Build {
            title: "0400000000000000".into(),
            base: PathBuf::new(),
            updates: Vec::new(),
            package: "v0".into(),
            split_versions: Vec::new(),
            work,
            profile: (profile, BTreeMap::new()),
            shaders: PathBuf::new(),
            out: PathBuf::new(),
            gpus: Vec::new(),
            engine: PathBuf::new(),
            nodes: Vec::new(),
            src: crate::inputs::Inputs {
                source: crate::inputs::Source::Nup,
                app: 0x0400_0000_0000_0000,
                base: PathBuf::new(),
                updates: Vec::new(),
                dlc: Vec::new(),
                app_flag: None,
                has_base: false,
                notes: Vec::new(),
            },
            prov: true,
        }
    }

    /// The shader split runs inside the extract's pass (packages with a profile in place).
    pub(crate) fn split_fused(&self) -> bool {
        self.src.source == crate::inputs::Source::Packages && self.nodes[0].reused.is_none()
    }
}

/// Discovers the inputs and builds the graph.
pub fn plan(a: &Args) -> Result<Build, String> {
    let src = crate::inputs::discover(a)?;
    let title = format!("{:016X}", src.app);
    let (base, updates) = (src.base.clone(), src.updates.clone());
    // the folder the build folder and the output default into: the input folder, or the one holding the .nup file
    let home: PathBuf = if a.nsp_dir.is_file() {
        a.nsp_dir
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
    } else {
        a.nsp_dir.clone()
    };
    let engine = a
        .engine
        .clone()
        .ok_or("--engine <engine folder> is required (profiles under dm/games)")?;
    let work = a
        .work_dir
        .clone()
        .unwrap_or_else(|| home.join(format!("{title}_build")));
    let registered = engine.join("dm/games").join(&title);
    let has_registered = registered.join("game.tsv").is_file();
    let prov = a.provisional || !has_registered;
    if !prov && a.survey_only {
        crate::exit::set(2);
        return Err(format!(
            "--survey-only: the engine has a profile for {title}: it surveys a game with no profile (add --provisional to survey it anyway)"
        ));
    }
    let prof = if prov {
        let root = a
            .profile_out
            .as_deref()
            .map_or_else(|| work.join("profile"), crate::options::expand);
        profile_at(&root.join(&title), true)?
    } else {
        profile_at(&registered, false)?
    };
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
    // a provisional build of a game the engine has a profile for never takes the place of the registered build's app
    let shadow = if prov && has_registered {
        "-provisional"
    } else {
        ""
    };
    let out = a
        .out
        .clone()
        .unwrap_or_else(|| home.join(format!("{name}{shadow}.app")));
    // `--gpu`: architectures, `none`, or by default (a Mac app of a game with shader pairs) this Mac's GPU, as the
    // script resolves it, an iOS build takes its own list from the profile
    let gpus: Vec<String> = match a.gpu.as_deref().map(str::trim) {
        Some("none") => Vec::new(),
        Some(g) if !g.is_empty() => g.split(',').map(nxb_archive::plan::arch_name).collect(),
        _ if a.platform == "mac-app" && prof.0.join("shader_pairs.tsv").is_file() => {
            vec![crate::host::gpu::mac_arch(&work)?]
        }
        _ => Vec::new(),
    };
    let key = |name: &str, fields: &[(&str, String)]| {
        let mut b = ContentKey::builder(&format!("nxb-build {name} 1"));
        for (k, v) in fields {
            b = b.field_str(k, v);
        }
        b.finish()
    };
    let nsps: String = std::iter::once(identity(&base))
        .chain(updates.iter().map(|u| identity(&u.1)))
        .chain(src.dlc.iter().map(|d| identity(&d.2)))
        .collect::<Vec<_>>()
        .join(";");
    let ex = |v: &str| work.join("extract").join(v).join("RomFS.img");
    let have_extract = split_versions
        .iter()
        .chain([&package])
        .all(|v| ex(v).is_file());
    // a provisional build's rewrite is part of its extract stage: the images alone do not make it reusable
    let have_patched = !prov
        || std::fs::read_dir(work.join(format!("patched_{package}"))).is_ok_and(|d| {
            d.flatten()
                .any(|e| e.file_name().to_string_lossy().ends_with(".patched.bin"))
        });
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
            reused: (have_extract && have_patched)
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
        base,
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
        src,
        prov,
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
            b.out = b.work.parent().unwrap_or(&home).join(ipa);
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
    let _ = writeln!(s, "input {:?}", b.src.source);
    let _ = writeln!(s, "base {}", b.base.display());
    for (v, p) in &b.updates {
        let _ = writeln!(s, "update v{v} {}", p.display());
    }
    for (t, v, p) in &b.src.dlc {
        let _ = writeln!(s, "dlc {t:016X} v{v} {}", p.display());
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

/// Runs the build. Stages overlap where the graph allows ([`crate::order`]): the Mac host from the start, the app's
/// companion app, icon and settings once the extract and the host are done (beside the shader step), the first
/// archive streamed beside the shader step ([`crate::pipeline`]); the events stay those of a run one after another.
pub fn run(a: &Args, p: &std::sync::Arc<Progress>) -> Result<(), String> {
    use crate::order::{Stage, Work};
    use std::sync::Mutex;
    let mut b = plan(a)?;
    if a.dry_run {
        print!("{}", describe(&b));
        return Ok(());
    }
    if b.prov {
        // the tools the build starts (make, the Python helpers) find the provisional profile as the script's
        // DM_GAMES_DIR makes them
        crate::host::set_games(
            b.profile
                .0
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
            a.provisional
                && b.engine
                    .join("dm/games")
                    .join(&b.title)
                    .join("game.tsv")
                    .is_file(),
        );
        // no profile yet: survey and guess first (they write it), then plan again with it in place
        if crate::provisional::prepare(a, &b, p)? {
            return Ok(());
        }
        b = plan(a)?;
    }
    crate::options::check_metal(a, &b, p)?;
    let seen_args;
    let a = if a.seen_shaders {
        seen_args = crate::options::seen_shaders(a, &b, p)?;
        &seen_args
    } else {
        a
    };
    if a.platform == "ios" {
        return crate::iosbuild::run(a, &b, p);
    }
    p.plan_more(b.nodes.len());
    let b = &b;
    let streamed: Mutex<Option<String>> = Mutex::new(None);
    let archive_error: Mutex<Option<String>> = Mutex::new(None);
    let no_archive = std::sync::atomic::AtomicBool::new(false);
    // the script's rule: a pipeline archive that fails is no archive for the app (the host builds its bases on the first
    // start), the build goes on
    let give_up = |e: String| -> crate::order::Outcome {
        p.say(&format!(
            "[RED] pipeline archive: {e}; the app is packed without one and builds its pipeline bases on its first start (--gpu none skips this step)"
        ));
        no_archive.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok("failed, the app is packed without one".into())
    };
    let (archive_error, no_archive) = (&archive_error, &no_archive);
    let parts: Mutex<Option<crate::host::app::AppParts>> = Mutex::new(None);
    let (streamed, parts) = (&streamed, &parts);
    let previous = b.out.is_dir().then(|| b.out.clone());
    let host = b.nodes.iter().position(|n| n.name == "Mac host");
    let mut stages: Vec<Stage<'_>> = Vec::new();
    for (i, n) in b.nodes.iter().enumerate() {
        let mut deps = n.deps.clone();
        let work = match (&n.reused, i, n.name.as_str()) {
            // reused images still need their patched modules: the rewrite runs (or stops the build) here
            (Some(r), 0, _) => {
                let r = r.clone();
                Work::Turn(Box::new(move || {
                    let note =
                        crate::rewrite::ensure(&b.engine, &b.profile.0, &b.work, &b.package)?;
                    Ok(format!(
                        "reused: {r}{}",
                        note.map(|n| format!(", {n}")).unwrap_or_default()
                    ))
                }))
            }
            (Some(r), _, _) => Work::Reused(format!("reused: {r}")),
            (_, 0, _) => Work::Turn(Box::new(move || crate::extract_stage::run(a, b, p))),
            (_, 1, _) if b.split_fused() => Work::Reused("in the extract's pass".into()),
            (_, 1, _) => Work::Turn(Box::new(move || crate::extract_stage::split(a, b, p))),
            (_, 2, _) => Work::Turn(Box::new(move || {
                crate::provisional::take_tables(b)?;
                // the shader step and the first architecture's archive together (crate::pipeline)
                let o = crate::pipeline::shaders_and_archive(a, b, p)?;
                *streamed.lock().map_err(|_| "lock")? = o.archive;
                *archive_error.lock().map_err(|_| "lock")? = o.archive_error;
                Ok(o.shaders)
            })),
            (_, _, name) if name.starts_with("Pipeline archive") => {
                let arch = name.trim_start_matches("Pipeline archive ");
                Work::Turn(Box::new(move || {
                    let made = streamed.lock().map_err(|_| "lock")?.take();
                    let failed = archive_error.lock().map_err(|_| "lock")?.take();
                    match (made, failed) {
                        (Some(s), _) => Ok(s),
                        (None, Some(e)) => give_up(e),
                        (None, None) => match crate::pipeline::stage_pso(b, arch) {
                            Ok(s) => Ok(s),
                            Err(e) => give_up(crate::pipeline::archive_failure(&e)),
                        },
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
                    crate::app_stage::stage_app(
                        a,
                        b,
                        prev,
                        made.ok_or("the app's parts were not made")?,
                        no_archive.load(std::sync::atomic::Ordering::SeqCst),
                    )
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
