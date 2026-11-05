//! The Mac app's file set (`dm/mac/pack_app.py`): which build outputs go where in `<Game>.app`, the generated
//! `Info.plist` and `PkgInfo`. The result is a [`StagePlan`] for [`crate::appstage`] (clones, digests, signing) or for
//! [`crate::incremental`]. The companion app (`clang` + `swiftc`) and the icon (`sips` + `iconutil`) are built by tools and
//! come in as files.

use std::path::{Path, PathBuf};

use nxb_pyjson::Value;

use crate::appstage::{Content, StageEntry, StagePlan};
use crate::error::{Error, Result, format_err};
use crate::pyplist::{PyValue, dumps};

/// The settings file name the engine's host reads (in the bundle and in the work folder): a format fact of the
/// host, kept as it is so the app stays byte-identical.
pub const SETTINGS_JSON: &str = "launcher.json";
const MODULE_FILES: [&str; 7] = [
    "*.patched.bin",
    "*.fixups.tsv",
    "*.gates.tsv",
    "*.cuts.tsv",
    "*.obs.tsv",
    "*.changed.tsv",
    "nro.tsv",
];
const MSL_FILES: [&str; 3] = ["*.metallib", "*.json", "*.tsv"];
const TABLES: [&str; 6] = [
    "shader_map.tsv",
    "pso_manifest.tsv",
    "c1_full.tsv",
    "cb_extent.tsv",
    "shader_pairs.tsv",
    "pso_pairs.tsv",
];
const PSO_FILES: [&str; 4] = ["*.mtl4a", "prebuilt.tsv", "rest_pairs.tsv", "archive.tsv"];

/// `pack_app.py`'s arguments (the ones this port takes.`--program` and `--aoc` are `Unsupported`).
#[derive(Clone, Debug, Default)]
pub struct MacAppInputs {
    /// `--host`: the Mac host.
    pub host: PathBuf,
    /// The companion app embedded next to the built app (`Contents/MacOS/nxnative`), built by the engine's script.
    pub companion: PathBuf,
    /// `--patched`.
    pub patched: PathBuf,
    /// `--romfs` (an image file or a folder).
    pub romfs: PathBuf,
    /// `--msl`.
    pub msl: PathBuf,
    /// `--msl-name`.
    pub msl_name: Option<String>,
    /// `--tables`.
    pub tables: PathBuf,
    /// `--font`.
    pub font: PathBuf,
    /// `--pso`.
    pub pso: Option<PathBuf>,
    /// `--pso-pairs`.
    pub pso_pairs: Option<PathBuf>,
    /// `--html`.
    pub html: Option<PathBuf>,
    /// `--env-file`.
    pub env_file: Option<PathBuf>,
    /// `--settings`.
    pub settings: Option<PathBuf>,
    /// The `.icns` made from `--icon` by `make_icns`.
    pub icns: Option<PathBuf>,
    /// `--title-id`, `--name`, `--version`, `--bundle-id`, `--category`, `--lang`, `--marker`.
    pub title_id: String,
    /// Display name.
    pub name: String,
    /// Version.
    pub version: Option<String>,
    /// Bundle identifier.
    pub bundle_id: Option<String>,
    /// Category.
    pub category: Option<String>,
    /// Language.
    pub lang: Option<String>,
    /// RomFS marker.
    pub marker: Option<String>,
    /// `DMForkPatches` (the comma-joined lines of `dm/fork_patches.txt`).
    pub fork_patches: String,
    /// `DMCommit`, when packed from a checkout.
    pub commit: Option<String>,
    /// `--aoc`: the DLC RomFS images or folders by index (`Resources/aoc`).
    pub aoc: Option<PathBuf>,
    /// `--program`: the other programs of a game of several programs.
    pub programs: Vec<Program>,
}

/// One `--program` row: index, rewritten modules, RomFS, Mac shader libraries, tables, offline pages.
#[derive(Clone, Debug, Default)]
pub struct Program {
    /// Index (`p<N>`).
    pub n: String,
    /// Rewritten modules.
    pub patched: PathBuf,
    /// RomFS image or folder.
    pub romfs: PathBuf,
    /// Shader libraries.
    pub msl: PathBuf,
    /// Tables folder.
    pub tables: PathBuf,
    /// Offline pages.
    pub html: PathBuf,
}

impl Program {
    /// Parses a `--program` value (tab-separated).
    pub fn parse(row: &str) -> Result<Program> {
        let c: Vec<&str> = row.split('\t').collect();
        let [n, patched, romfs, msl, tables, html] = c.as_slice() else {
            return Err(format_err(format!(
                "--program needs 6 tab-separated fields: {row:?}"
            )));
        };
        Ok(Program {
            n: n.to_string(),
            patched: patched.into(),
            romfs: romfs.into(),
            msl: msl.into(),
            tables: tables.into(),
            html: html.into(),
        })
    }
}

fn detritus(name: &str) -> bool {
    name.starts_with("._") || name == ".DS_Store"
}

/// `Path.glob(pattern)` for the patterns used here: `*suffix` or an exact name, files and folders of `dir` (not
/// recursive), sorted.
fn glob(dir: &Path, pats: &[&str]) -> Result<Vec<PathBuf>> {
    let mut out = std::collections::BTreeSet::new();
    for e in std::fs::read_dir(dir).map_err(|e| Error::io(dir, e))? {
        let e = e.map_err(|e| Error::io(dir, e))?;
        let name = e.file_name().to_string_lossy().into_owned();
        let hit = pats.iter().any(|p| match p.strip_prefix('*') {
            Some(suffix) => name.ends_with(suffix),
            None => name == *p,
        });
        if hit && !detritus(&name) {
            out.insert(e.path());
        }
    }
    Ok(out.into_iter().collect())
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// `min_os`: the host's `LC_BUILD_VERSION` minos as `otool` prints it (`26.6`, `14.0.1`), `14.0` without one.
pub fn min_os(host: &[u8]) -> Result<String> {
    let thin = if crate::fat::is_fat(host) {
        let slices = crate::fat::split(host)?;
        slices
            .first()
            .map(|s| s.data.clone())
            .ok_or_else(|| format_err("empty universal file"))?
    } else {
        host.to_vec()
    };
    let m = crate::macho::MachO::parse(&thin)?;
    Ok(match m.minos {
        Some(v) => {
            let (a, b, c) = (v >> 16, (v >> 8) & 0xff, v & 0xff);
            if c == 0 {
                format!("{a}.{b}")
            } else {
                format!("{a}.{b}.{c}")
            }
        }
        None => "14.0".into(),
    })
}

fn float_of(v: Option<&Value>) -> Result<f64> {
    match v {
        Some(Value::Int(i)) => Ok(*i as f64),
        Some(Value::Float(f)) => Ok(*f),
        Some(Value::Str(s)) => s
            .trim()
            .parse()
            .map_err(|_| format_err(format!("setting bound {s:?}"))),
        _ => Err(format_err("setting bound missing")),
    }
}

fn str_of(v: Option<&Value>) -> String {
    v.and_then(Value::as_str).unwrap_or("").to_string()
}

/// `settings_rules` and the `DM_LANG` default of the app settings.
fn settings(json: &str) -> Result<(PyValue, Option<String>)> {
    let v = nxb_pyjson::parse(json).map_err(|e| format_err(format!("app settings: {e:?}")))?;
    let listed = v
        .get("settings")
        .and_then(Value::as_list)
        .ok_or_else(|| format_err("app settings: no list"))?;
    let mut rules = Vec::new();
    let mut lang = None;
    for s in listed {
        let env = str_of(s.get("env"));
        match s.get("kind").and_then(Value::as_str) {
            Some("range") => rules.push(PyValue::Dict(vec![
                ("env".into(), PyValue::Str(env.clone())),
                ("min".into(), PyValue::Real(float_of(s.get("min"))?)),
                ("max".into(), PyValue::Real(float_of(s.get("max"))?)),
            ])),
            Some("choice") => {
                let opts = s.get("options").and_then(Value::as_list).unwrap_or(&[]);
                let values = opts.iter().map(|o| {
                    PyValue::Str(
                        o.as_list()
                            .and_then(|l| l.first())
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .into(),
                    )
                });
                rules.push(PyValue::Dict(vec![
                    ("env".into(), PyValue::Str(env.clone())),
                    ("values".into(), PyValue::List(values.collect())),
                ]));
            }
            Some("toggle") => {
                let on = s
                    .get("on")
                    .and_then(Value::as_str)
                    .unwrap_or("1")
                    .to_string();
                let off = s
                    .get("off")
                    .and_then(Value::as_str)
                    .unwrap_or("0")
                    .to_string();
                rules.push(PyValue::Dict(vec![
                    ("env".into(), PyValue::Str(env.clone())),
                    (
                        "values".into(),
                        PyValue::List(vec![PyValue::Str(on), PyValue::Str(off)]),
                    ),
                ]));
            }
            _ => {}
        }
        if env == "DM_LANG" && lang.is_none() {
            lang = s.get("default").and_then(Value::as_str).map(str::to_string);
        }
    }
    Ok((PyValue::List(rules), lang))
}

fn tree(src: &Path, dest: &str, out: &mut Vec<StageEntry>) -> Result<()> {
    let mut names: Vec<_> = std::fs::read_dir(src)
        .map_err(|e| Error::io(src, e))?
        .filter_map(|e| e.ok())
        .collect();
    names.sort_by_key(|e| e.file_name());
    for e in names {
        let name = e.file_name().to_string_lossy().into_owned();
        let p = e.path();
        let d = format!("{dest}/{name}");
        let ft = e.file_type().map_err(|x| Error::io(&p, x))?;
        if ft.is_dir() {
            tree(&p, &d, out)?;
        } else if ft.is_file() {
            out.push(StageEntry {
                dest: d,
                content: Content::File(p),
                digests: None,
            });
        } else {
            return Err(Error::Unsupported(format!(
                "symbolic link in a copied tree: {}",
                p.display()
            )));
        }
    }
    Ok(())
}

fn romfs_name(p: &Path) -> &'static str {
    if p.is_file() { "romfs.img" } else { "romfs" }
}

/// The other programs: `p<N>/` (RomFS, pages), `msl/p<N>/<library>`, `dm/p<N>/` (modules, tables) and `dm/programs.tsv`.
fn programs(a: &MacAppInputs, tables: &[PathBuf], e: &mut Vec<StageEntry>) -> Result<()> {
    let file = |dest: String, src: &Path| StageEntry {
        dest,
        content: Content::File(src.to_path_buf()),
        digests: None,
    };
    let mut first = vec![
        "0".to_string(),
        "exe=@C@/MacOS/dmhost".into(),
        "arg=@R@/dm".into(),
        "cwd=@S@".into(),
    ];
    first.push(format!("DM_ROMFS=@R@/{}", romfs_name(&a.romfs)));
    first.push("DM_MSL_DIR=@R@/msl".into());
    if tables.iter().any(|t| file_name(t) == "shader_map.tsv") {
        first.push("DM_NVN_SHMAP=@R@/dm/shader_map.tsv".into());
    }
    if a.pso.is_some() {
        first.push("DM_PSO_BUNDLE=@R@/pso".into());
    }
    let mut rows = vec![
        "# The programs of this game (dm/mac/pack_app.py; hle_oe.c DM_PROGRAMS)".to_string(),
        first.join("\t"),
    ];
    for p in &a.programs {
        let n = &p.n;
        for m in glob(&p.patched, &MODULE_FILES)? {
            e.push(file(
                format!("Contents/Resources/dm/p{n}/{}", file_name(&m)),
                &m,
            ));
        }
        for t in TABLES
            .iter()
            .map(|x| p.tables.join(x))
            .filter(|x| x.is_file())
        {
            e.push(file(
                format!("Contents/Resources/dm/p{n}/{}", file_name(&t)),
                &t,
            ));
        }
        let lib = file_name(&p.msl);
        for m in glob(&p.msl, &MSL_FILES)?
            .into_iter()
            .filter(|x| x.is_file() && file_name(x) != "_b_build.json")
        {
            e.push(file(
                format!("Contents/Resources/msl/p{n}/{lib}/{}", file_name(&m)),
                &m,
            ));
        }
        if p.romfs.is_file() {
            e.push(file(format!("Contents/Resources/p{n}/romfs.img"), &p.romfs));
        } else {
            tree(&p.romfs, &format!("Contents/Resources/p{n}/romfs"), e)?;
        }
        if p.html.is_dir() {
            tree(&p.html, &format!("Contents/Resources/p{n}/html"), e)?;
        }
        rows.push(
            [
                n.clone(),
                "exe=@C@/MacOS/dmhost".into(),
                format!("arg=@R@/dm/p{n}"),
                "cwd=@S@".into(),
                format!("DM_ROMFS=@R@/p{n}/{}", romfs_name(&p.romfs)),
                format!("DM_MSL_DIR=@R@/msl/p{n}"),
                format!("DM_NVN_SHMAP=@R@/dm/p{n}/shader_map.tsv"),
                format!("DM_PROGRAM={n}"),
            ]
            .join("\t"),
        );
    }
    e.push(StageEntry {
        dest: "Contents/Resources/dm/programs.tsv".into(),
        content: Content::Bytes((rows.join("\n") + "\n").into_bytes()),
        digests: None,
    });
    Ok(())
}

/// The reuse key of an app (`<app>.key`): the plan's destinations with their sources' identity (path, size,
/// modification and change time, inode, as `pack_app.py`'s inventory takes them) and the generated files' bytes. Equal
/// keys and an app on disk: nothing to do.
pub fn reuse_key(plan: &StagePlan) -> Result<String> {
    use std::os::unix::fs::MetadataExt;
    let mut h = nxb_model::Sha256::new();
    let mut rows: Vec<(&str, Vec<u8>)> = Vec::new();
    for e in &plan.entries {
        let id = match &e.content {
            Content::File(p) => {
                let m = std::fs::metadata(p).map_err(|x| Error::io(p, x))?;
                format!(
                    "{}\t{}\t{}\t{}\t{}\t{}",
                    p.display(),
                    m.len(),
                    m.mtime_nsec() + m.mtime() * 1_000_000_000,
                    m.ctime(),
                    m.ino(),
                    m.mode()
                )
                .into_bytes()
            }
            Content::Bytes(b) => nxb_model::sha256(b).0.to_vec(),
        };
        rows.push((&e.dest, id));
    }
    rows.sort();
    h.update(b"nxb-pack mac app key 1\0");
    for (d, id) in rows {
        h.update(d.as_bytes());
        h.update(&[0]);
        h.update(&id);
        h.update(&[0]);
    }
    Ok(h.finalize().to_hex())
}

/// The bundle's file set, destinations relative to the `.app` folder.
pub fn plan(a: &MacAppInputs) -> Result<StagePlan> {
    let mut e = Vec::new();
    let file = |dest: String, src: &Path| StageEntry {
        dest,
        content: Content::File(src.to_path_buf()),
        digests: None,
    };
    let mods = glob(&a.patched, &MODULE_FILES)?;
    if !mods.iter().any(|p| file_name(p).ends_with(".patched.bin")) {
        return Err(format_err(format!(
            "no rewritten module in {}",
            a.patched.display()
        )));
    }
    let msls: Vec<PathBuf> = glob(&a.msl, &MSL_FILES)?
        .into_iter()
        .filter(|p| p.is_file() && file_name(p) != "_b_build.json")
        .collect();
    if !msls.iter().any(|p| file_name(p).ends_with(".metallib")) {
        return Err(format_err(format!(
            "no shader libraries at {}",
            a.msl.display()
        )));
    }
    e.push(file("Contents/MacOS/nxnative".into(), &a.companion));
    e.push(file("Contents/MacOS/dmhost".into(), &a.host));
    for m in &mods {
        e.push(file(format!("Contents/Resources/dm/{}", file_name(m)), m));
    }
    let tables: Vec<PathBuf> = TABLES
        .iter()
        .map(|n| a.tables.join(n))
        .filter(|p| p.is_file())
        .collect();
    for t in &tables {
        e.push(file(format!("Contents/Resources/dm/{}", file_name(t)), t));
    }
    if let Some(p) = &a.pso_pairs
        && !tables.iter().any(|t| file_name(t) == "pso_pairs.tsv")
    {
        e.push(file("Contents/Resources/dm/pso_pairs.tsv".into(), p));
    }
    for f in glob(&a.font, &["*.ttf"])? {
        e.push(file(
            format!("Contents/Resources/dm/font/{}", file_name(&f)),
            &f,
        ));
    }
    if let Some(p) = &a.env_file {
        e.push(file("Contents/Resources/dm/env.txt".into(), p));
    }
    if let Some(p) = &a.settings {
        e.push(file(format!("Contents/Resources/dm/{SETTINGS_JSON}"), p));
    }
    let msl_name = a.msl_name.clone().unwrap_or_else(|| file_name(&a.msl));
    for m in &msls {
        e.push(file(
            format!("Contents/Resources/msl/{msl_name}/{}", file_name(m)),
            m,
        ));
    }
    if let Some(pso) = &a.pso {
        for p in glob(pso, &PSO_FILES)? {
            e.push(file(
                format!("Contents/Resources/pso/{}", file_name(&p)),
                &p,
            ));
        }
    }
    if let Some(aoc) = &a.aoc {
        if aoc.is_file() {
            e.push(file("Contents/Resources/aoc".into(), aoc));
        } else {
            tree(aoc, "Contents/Resources/aoc", &mut e)?;
        }
    }
    if a.romfs.is_file() {
        e.push(file("Contents/Resources/romfs.img".into(), &a.romfs));
    } else {
        tree(&a.romfs, "Contents/Resources/romfs", &mut e)?;
    }
    if let Some(h) = a.html.as_ref().filter(|h| h.is_dir()) {
        tree(h, "Contents/Resources/html", &mut e)?;
    }
    if !a.programs.is_empty() {
        programs(a, &tables, &mut e)?;
    }
    if let Some(i) = &a.icns {
        e.push(file("Contents/Resources/AppIcon.icns".into(), i));
    }
    let host = std::fs::read(&a.host).map_err(|x| Error::io(&a.host, x))?;
    let minos = min_os(&host)?;
    let s = |v: &str| PyValue::Str(v.to_string());
    let version = a.version.clone().unwrap_or_else(|| "1.0".into());
    let mut p = vec![
        ("CFBundleName".to_string(), s(&a.name)),
        ("CFBundleDisplayName".into(), s(&a.name)),
        (
            "CFBundleIdentifier".into(),
            s(&a.bundle_id
                .clone()
                .unwrap_or_else(|| format!("org.nxnative.{}", a.title_id.to_lowercase()))),
        ),
        ("CFBundleVersion".into(), s(&version)),
        ("CFBundleShortVersionString".into(), s(&version)),
        ("CFBundlePackageType".into(), s("APPL")),
        ("CFBundleExecutable".into(), s("nxnative")),
        ("CFBundleInfoDictionaryVersion".into(), s("6.0")),
        ("LSMinimumSystemVersion".into(), s(&minos)),
        (
            "LSArchitecturePriority".into(),
            PyValue::List(vec![s("arm64")]),
        ),
        ("NSHighResolutionCapable".into(), PyValue::Bool(true)),
        (
            "LSApplicationCategoryType".into(),
            s(a.category.as_deref().unwrap_or("public.app-category.games")),
        ),
        ("LSSupportsGameMode".into(), PyValue::Bool(false)),
        ("DMTitleID".into(), s(&a.title_id.to_uppercase())),
        ("DMRomfsMarker".into(), s(a.marker.as_deref().unwrap_or(""))),
        ("DMLanguage".into(), s(a.lang.as_deref().unwrap_or("en-US"))),
        ("DMForkPatches".into(), s(&a.fork_patches)),
    ];
    if a.icns.is_some() {
        p.push(("CFBundleIconFile".into(), s("AppIcon")));
    }
    if let Some(path) = &a.settings {
        let text = std::fs::read_to_string(path).map_err(|x| Error::io(path, x))?;
        let (rules, lang) = settings(&text)?;
        p.push(("DMSettings".into(), rules));
        if let Some(l) = lang
            && let Some(slot) = p.iter_mut().find(|(k, _)| k == "DMLanguage")
        {
            slot.1 = PyValue::Str(l);
        }
    }
    p.push((
        "DMModuleFiles".into(),
        PyValue::List(mods.iter().map(|m| PyValue::Str(file_name(m))).collect()),
    ));
    if let Some(c) = &a.commit {
        p.push(("DMCommit".into(), s(c)));
    }
    if let Some(chain) = msl_name.strip_prefix("b-") {
        p.push(("DMChain".into(), s(chain)));
    }
    e.push(StageEntry {
        dest: "Contents/Info.plist".into(),
        content: Content::Bytes(dumps(&PyValue::Dict(p))),
        digests: None,
    });
    e.push(StageEntry {
        dest: "Contents/PkgInfo".into(),
        content: Content::Bytes(b"APPL????".to_vec()),
        digests: None,
    });
    Ok(StagePlan { entries: e })
}
