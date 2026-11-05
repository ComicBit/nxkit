//! `dm/ios/tools/build_ipa.sh`, the one-game app: the checks of the game's inputs, `Gen/` (host sources, embedded
//! modules, tables, app binary settings, fonts, icon, entitlements), `xcodegen`, the Info.plist lines and the resource
//! paths. With `--prepare-xcode` it stops there (the per-game project in `DM_XCODE_PROJECT_DIR`), otherwise
//! [`super::ipapack`] builds the app unsigned, signs its executable ad hoc with the entitlements and packs the IPA.
//!
//! Ported generators run here (`gen_build`, `gen_embed`, `gen_resources`, the app binary settings, `game.py modules`,
//! the profile inputs digest), the engine's other Python tools (`nacp.py`, `romfs.py`, `romfs_image.py`,
//! `make_ext_font.py`, `pso_offline.py`) and Apple's tools run as the script runs them, through [`Runner`].
//! Not ported (refused as unsupported, the caller runs the script): a game of several programs (`PROGRAMS_TSV`) and
//! route C's framework builds (`DM_FRAMEWORK=1`, `DM_FRAMEWORK_ONLY`).

pub use super::ipagen::modules;
use super::ipagen::{check_program, pso_gpus};
use super::script::{
    chain_version, die, ln_sfn, mkdir_p, paste_commas, pwd_of, rm_rf, set_nonempty,
};
use super::{embed, genbuild};
use crate::host::Runner;
use crate::host::profile::{Game, Profiles};
use std::path::{Path, PathBuf};

/// The script's environment (`None`: unset, the script tells unset from empty where it matters).
#[derive(Clone, Debug, Default)]
#[allow(missing_docs)]
pub struct Inputs {
    /// The engine (the folder holding `dm/`).
    pub engine: PathBuf,
    pub game: String,
    pub w: String,
    pub patched: Option<String>,
    pub romfs: Option<String>,
    pub msl_ios: Option<String>,
    pub aoc: Option<String>,
    pub gfxapi: Option<String>,
    pub font_py: Option<String>,
    pub std_font: Option<String>,
    pub display_name: Option<String>,
    pub app_version: Option<String>,
    pub bundle_id: Option<String>,
    pub team_id: Option<String>,
    pub lang: Option<String>,
    pub control: Option<String>,
    pub xcode_project_dir: Option<String>,
    pub xcode_dd: Option<String>,
    pub copy_romfs: Option<String>,
    pub full_romfs: Option<String>,
    pub entitlements: Option<String>,
    pub icon: Option<String>,
    pub env_file: Option<String>,
    pub show_fps: Option<String>,
    pub metal_capture: Option<String>,
    pub pso_gpus: Option<String>,
    pub pso_ios_min: Option<String>,
    pub ios_pull: Option<String>,
    pub v: Option<String>,
    pub framework: Option<String>,
    pub framework_only: Option<String>,
    pub programs_tsv: Option<String>,
    /// `DM_GAMES_DIR`, `DM_GAMES_FIRST`, `USER`.
    pub games_dir: Option<String>,
    pub games_first: Option<String>,
    pub user: String,
}

/// What the generation leaves for the build and the pack.
#[derive(Clone, Debug)]
pub struct State {
    /// The project folder (`dm/ios`, or the prepared project's folder).
    pub ios: PathBuf,
    /// The runner with the variables the script exported.
    pub runner: Runner,
    /// The profile.
    pub g: Game,
    pub(crate) pkg: String,
    pub(crate) romfs: String,
    pub(crate) msl_ios: String,
    pub(crate) msldir: String,
    pub(crate) aoc: String,
    pub(crate) pso: String,
    pub(crate) out_ipa: PathBuf,
    pub(crate) o: PathBuf,
    pub(crate) dd: PathBuf,
    pub(crate) app_version: String,
}

/// `${X:-default}`.
pub(crate) fn or(v: &Option<String>, d: impl Into<String>) -> String {
    v.clone()
        .filter(|x| !x.is_empty())
        .unwrap_or_else(|| d.into())
}

fn dirname(p: &str) -> String {
    Path::new(p)
        .parent()
        .map(|d| d.to_string_lossy().into_owned())
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| ".".into())
}

pub(crate) fn append(p: &Path, line: &str) -> Result<(), String> {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(p)
        .and_then(|mut f| f.write_all(line.as_bytes()))
        .map_err(|e| die(format!("{}: {e}", p.display())))
}

pub(crate) fn copy(from: &Path, to: &Path, what: &str) -> Result<(), String> {
    std::fs::copy(from, to).map(|_| ()).map_err(|_| die(what))
}

/// What the script makes: `--prepare-xcode`, or an IPA (`None`: the profile's IPA name beside `W`).
#[derive(Clone, Copy, Debug)]
pub enum Target<'a> {
    /// The per-game Xcode project.
    Prepare,
    /// The unsigned IPA.
    Ipa(Option<&'a Path>),
}

/// `build_ipa.sh [out.ipa | --prepare-xcode]`: returns the project folder or the IPA.
pub fn run(
    base: &Runner,
    t: Target<'_>,
    i: &Inputs,
    say: &mut dyn FnMut(&str),
) -> Result<PathBuf, String> {
    match t {
        Target::Prepare => Ok(generate(base, true, None, i, say)?.ios),
        Target::Ipa(out) => {
            let st = generate(base, false, out, i, say)?;
            super::ipapack::build(&st, i, say)
        }
    }
}

/// Run the generation part of the script (everything before `xcodebuild`). `out` is the IPA argument (`None` with
/// `--prepare-xcode`).
pub fn generate(
    base: &Runner,
    prepare: bool,
    out: Option<&Path>,
    i: &Inputs,
    say: &mut dyn FnMut(&str),
) -> Result<State, String> {
    if set_nonempty(&i.programs_tsv)
        || set_nonempty(&i.framework_only)
        || or(&i.framework, "0") != "0"
    {
        return Err("unsupported: a game of several programs or a route C framework build (run build_ipa.sh)".into());
    }
    let dm = i.engine.join("dm");
    let ios_source = dm.join("ios");
    if i.game.is_empty() {
        return Err(die("GAME is not set: GAME=<base game title ID>"));
    }
    let var = |k: &str| match k {
        "DM_GAMES_DIR" => i.games_dir.clone(),
        "DM_GAMES_FIRST" => i.games_first.clone(),
        _ => None,
    };
    let g = Profiles::new(&i.engine, &var)
        .load(&i.game)
        .map_err(|_| die(format!("no game profile {}", i.game)))?;
    let gd = g.dir.display().to_string();
    let gp = |k: &str| g.get(k).map_err(|_| die(format!("game profile {gd}: {k}")));
    if i.w.is_empty() {
        return Err(die(format!(
            "W is not set: W=<NSP folder>/{}-work (the game's work folder)",
            i.game
        )));
    }
    let w = &i.w;
    let pkg = gp("package")?;
    let mut romfs = i.romfs.clone().filter(|x| !x.is_empty());
    let img = format!("{w}/data/{pkg}/RomFS.img");
    if romfs.is_none() && Path::new(&img).is_file() {
        romfs = Some(img);
    }
    let patched = or(&i.patched, format!("{w}/build/patched_{pkg}"));
    let romfs = romfs.unwrap_or_else(|| format!("{w}/data/{pkg}/RomFS"));
    let msl = or(&i.msl_ios, format!("{w}/build/ios/dmsh_b"));
    let aoc = i.aoc.clone().unwrap_or_else(|| format!("{w}/data/aoc"));
    let out_ipa = match out {
        Some(p) => p.to_path_buf(),
        None => PathBuf::from(dirname(w)).join(gp("ipa")?),
    };
    let o = PathBuf::from(format!("{w}/build/ios/pkg"));
    let dd = o.join("dd");
    let xcode_dd = or(&i.xcode_dd, format!("{w}/build/ios/xcode-dd"));
    let marker = gp("romfs_marker")?;
    let rd = dirname(&romfs);
    let mut nacp = format!("{w}/data/v0/control.nacp");
    if !Path::new(&nacp).is_file() {
        nacp = format!("{rd}/control.nacp");
    }
    let nacp_py = dm.join("tools/nacp.py").to_string_lossy().into_owned();
    let py_out = |args: &[&str]| -> Result<String, String> {
        base.stdout("python3", args, None)
            .map(|s| s.trim_end_matches('\n').to_string())
    };
    let name = match i.display_name.clone().filter(|x| !x.is_empty()) {
        Some(n) => n,
        None => py_out(&[&nacp_py, &nacp]).map_err(|_| {
            die(format!(
                "no game name in {nacp} (re-extract with nx_extract, or set DM_DISPLAY_NAME)"
            ))
        })?,
    };
    let mut vnacp = format!("{rd}/control.nacp");
    if !Path::new(&vnacp).is_file() {
        vnacp = format!("{w}/data/v0/control.nacp");
    }
    let version = match i.app_version.clone().filter(|x| !x.is_empty()) {
        Some(v) => v,
        None => match py_out(&[&nacp_py, "--version", &vnacp]) {
            Ok(v) => v,
            Err(_) => {
                say(&format!(
                    "no display version in {vnacp}: the app version is 1.0"
                ));
                "1.0".into()
            }
        },
    };
    let vnacp = or(&i.control, vnacp);
    let bundle_id = or(
        &i.bundle_id,
        format!("org.nxnative.{}", i.game.to_lowercase()),
    );
    let show_fps = if i.show_fps.as_deref() == Some("1") {
        "YES"
    } else {
        "NO"
    };
    let mut lang = or(&i.lang, "en-US");
    let fork = paste_commas(&dm.join("fork_patches.txt"));
    let mut ios = ios_source.clone();
    let scheme = if prepare {
        let p = PathBuf::from(or(&i.xcode_project_dir, format!("{w}/xcode")));
        mkdir_p(&p)?;
        ios = pwd_of(&p)?;
        if ios == pwd_of(&ios_source)? {
            return Err(die("the per-game Xcode project must be outside dm/ios"));
        }
        for item in ["Sources", "tools"] {
            ln_sfn(&ios_source.join(item), &ios.join(item))?;
        }
        copy(
            &ios_source.join("project.yml"),
            &ios.join("project.yml"),
            "copy project template",
        )?;
        format!("{} ({})", name.replace(['/', ':'], "_"), i.game)
    } else {
        "DmIOS".into()
    };
    let mut exports: Vec<(&str, String)> = vec![
        ("GAME", i.game.clone()),
        ("LC_ALL", "C".into()),
        ("DM_BUNDLE_ID", bundle_id.clone()),
        ("DM_DISPLAY_NAME", name.clone()),
        ("DM_APP_VERSION", version.clone()),
        ("DM_APP_CATEGORY", gp("app_category")?),
        ("DM_TEAM_ID", or(&i.team_id, "0000000000")),
        ("DM_SHOW_FPS", show_fps.into()),
        ("DM_LANG", lang.clone()),
        ("DM_FORK_PATCHES", fork),
        ("DM_SCHEME_NAME", scheme),
    ];
    let runner = |ex: &[(&str, String)]| -> Result<Runner, String> {
        let mut r = base.clone();
        for (k, v) in ex {
            r = r.with_var(k, v.clone())?;
        }
        Ok(r)
    };
    let r = runner(&exports)?;
    let v =
        i.v.clone()
            .filter(|x| !x.is_empty())
            .unwrap_or_else(|| chain_version(&dm.join("gfx/nvn/dmsh_chain.h")));
    let (mods, gfx) = check_program(&r, i, &g, [&patched, &romfs, &msl], &marker, &v)?;
    say(&format!(
        "app: {name} {version} ({bundle_id}), language {lang}{}",
        if show_fps == "YES" {
            ", frame-rate counter on"
        } else {
            ""
        }
    ));
    say("generate Gen/ (host sources, embedded modules, tables)");
    let gen_dir = ios.join("Gen");
    rm_rf(&gen_dir)?;
    mkdir_p(&gen_dir.join("dm"))?;
    let cfg = gen_dir.join("dm.xcconfig");
    let text = genbuild::generate(&r, &dm.join("host"), &gen_dir)
        .map_err(|e| die(format!("gen_build ({e})")))?;
    std::fs::write(&cfg, text).map_err(|_| die("gen_build"))?;
    let jit = format!("{rd}/jit/jit.img");
    let jit = Path::new(&jit).is_file().then_some(jit);
    let flags = embed::write(
        &patched,
        &gen_dir.join("embed.S"),
        &mods,
        "",
        jit.as_deref(),
    )
    .map_err(|e| die(format!("gen_embed ({e})")))?;
    append(
        &cfg,
        &format!("OTHER_LDFLAGS = $(inherited) $(DM_FRAMEWORKS) {flags}\n"),
    )?;
    // program_tables
    let td = gen_dir.join("dm");
    let mut tables: Vec<PathBuf> = Vec::new();
    for suf in [".fixups.tsv", ".gates.tsv", ".cuts.tsv", ".obs.tsv"] {
        let mut m: Vec<PathBuf> = std::fs::read_dir(&patched)
            .map(|d| d.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        m.retain(|p| {
            let n = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            n.ends_with(suf) && !n.starts_with('.') && n.len() > suf.len()
        });
        if m.is_empty() {
            return Err(die("copy rewriter tables"));
        }
        tables.extend(m);
    }
    for t in &tables {
        copy(
            t,
            &td.join(t.file_name().unwrap_or_default()),
            "copy rewriter tables",
        )?;
    }
    let nro = Path::new(&patched).join("nro.tsv");
    if nro.is_file() {
        copy(&nro, &td.join("nro.tsv"), "copy the NRO table")?;
    }
    let mut shader_tables = vec!["shader_map.tsv", "pso_manifest.tsv"];
    if gfx != "vulkan" && gfx != "gl" {
        shader_tables.push("c1_full.tsv");
    }
    for t in &shader_tables {
        copy(&g.dir.join(t), &td.join(t), "copy shader tables")?;
    }
    if gfx != "vulkan" && gfx != "gl" {
        if g.dir.join("cb_extent.tsv").is_file() {
            copy(
                &g.dir.join("cb_extent.tsv"),
                &td.join("cb_extent.tsv"),
                "copy cb_extent.tsv",
            )?;
        } else {
            say(&format!(
                "no {gd}/cb_extent.tsv: the host copies whole uniform bindings"
            ));
        }
        for f in ["shader_pairs.tsv", "pso_pairs.tsv"] {
            if g.dir.join(f).is_file() {
                copy(&g.dir.join(f), &td.join(f), &format!("copy {f}"))?;
            }
        }
    }
    let lj = td.join("launcher.json");
    let env_of = |k: &str| -> Option<String> {
        if k == "GFXAPI" {
            return Some(gfx.clone());
        }
        exports
            .iter()
            .find(|(n, _)| *n == k)
            .map(|(_, v)| v.clone())
            .or_else(|| r.env().get(k).map(|v| v.to_string_lossy().into_owned()))
    };
    let settings = crate::host::settings::build(
        &i.engine,
        &i.game,
        Path::new(&patched),
        Path::new(&vnacp),
        &env_of,
    )
    .map_err(|_| die("app binary settings (dm/games/settings.py)"))?;
    std::fs::write(&lj, format!("{}\n", nxb_pyjson::dumps_indent(&settings, 1)))
        .map_err(|_| die("app binary settings (dm/games/settings.py)"))?;
    let list = settings
        .get("settings")
        .and_then(|s| s.as_list())
        .unwrap_or(&[]);
    let envs: Vec<&str> = list.iter().filter_map(|s| s.get("env")?.as_str()).collect();
    say(&format!("app binary: {}", envs.join(" ")));
    // the pipeline archive
    let mut pso = String::new();
    let p4 = g.get_or("pso_mtl4", "off");
    let pg = g.get_or("pso_gpu", "");
    let (gpus, warn) = pso_gpus(&i.pso_gpus, &p4, &pg);
    if let Some(wn) = warn {
        say(&format!("profile {gd}: {wn}"));
    }
    let pairs = g.dir.join("shader_pairs.tsv").is_file();
    if gfx == "nvn" && !gpus.is_empty() && gpus != "none" && pairs {
        pso = format!("{w}/build/ios/pso/bundle");
        say(&format!(
            "pipeline archive for {gpus} (metal-tt; minutes when its inputs changed)"
        ));
        let mut a: Vec<String> = vec![
            dm.join("gfx/tools/pso_offline.py")
                .to_string_lossy()
                .into_owned(),
            "--game".into(),
            i.game.clone(),
            "--msl".into(),
            format!("{msl}/b-{v}"),
            "--out".into(),
            pso.clone(),
            "--work".into(),
            format!("{w}/build/ios/pso/work"),
            "--gpu".into(),
            gpus.clone(),
        ];
        if let Some(m) = i.pso_ios_min.clone().filter(|x| !x.is_empty()) {
            a.extend(["--os-min".into(), m]);
        }
        match r.stdout("python3", &a, None) {
            Ok(text) => text.lines().for_each(&mut *say),
            Err(_) => {
                say(
                    "[RED] pipeline archive: pso_offline.py failed (its output is above); the app is built without one and the device builds its pipeline bases on its first start (DM_PSO_GPUS=none skips this step)",
                );
                rm_rf(Path::new(&pso))?;
                pso.clear();
            }
        }
    } else if gfx == "nvn" {
        let why = if i.pso_gpus.is_some() {
            format!(
                "DM_PSO_GPUS={}",
                if gpus.is_empty() { "empty" } else { &gpus }
            )
        } else {
            "the profile does not set pso_mtl4 on; DM_PSO_GPUS=g18p builds one".into()
        };
        say(&format!(
            "no pipeline archive ({why}{})",
            if pairs { "" } else { ", no shader_pairs.tsv" }
        ));
    }
    // the packed language is the Game language row's default
    let ld = list
        .iter()
        .find(|s| s.get("env").and_then(|e| e.as_str()) == Some("DM_LANG"))
        .and_then(|s| s.get("default"))
        .map(|d| match d {
            nxb_pyjson::Value::Str(s) => s.clone(),
            other => nxb_pyjson::dumps(other),
        })
        .unwrap_or_default();
    if !ld.is_empty() && ld != lang {
        say(&format!(
            "game language: the game has no {lang}; packing {ld}, its first"
        ));
        lang = ld;
        for e in exports.iter_mut().filter(|e| e.0 == "DM_LANG") {
            e.1 = lang.clone();
        }
    }
    if let Some(f) = i.env_file.clone().filter(|x| !x.is_empty()) {
        copy(Path::new(&f), &td.join("env.txt"), "copy DM_ENV_FILE")?;
        let t = std::fs::read_to_string(&f).unwrap_or_default();
        let kept: String = t
            .split_inclusive('\n')
            .filter(|l| !l.starts_with('#'))
            .collect::<String>()
            .replace('\n', " ");
        say(&format!("env: {kept}"));
    }
    let msldir = match gfx.as_str() {
        "vulkan" => "vk".to_string(),
        "gl" => "gl".to_string(),
        _ => format!("b-{v}"),
    };
    let r = runner(&exports)?;
    let scheme = exports
        .iter()
        .find(|(k, _)| *k == "DM_SCHEME_NAME")
        .map(|(_, v)| v.clone())
        .unwrap_or_default();
    let shaders = format!("{msl}/{msldir}");
    super::ipagen::finish(
        &super::ipagen::Finish {
            r: &r,
            prepare,
            i,
            ios: &ios,
            gen_dir: &gen_dir,
            cfg: &cfg,
            rd: &rd,
            msldir: &msldir,
            version: &version,
            pkg: &pkg,
            scheme: &scheme,
            paths: [&romfs, &marker, &shaders, &msldir, &aoc, &xcode_dd, &pso],
        },
        say,
    )?;
    Ok(State {
        ios,
        runner: r,
        g,
        pkg,
        romfs,
        msl_ios: msl,
        msldir,
        aoc,
        pso,
        out_ipa,
        o,
        dd,
        app_version: version,
    })
}

#[cfg(test)]
#[path = "ipa_tests.rs"]
mod tests;
