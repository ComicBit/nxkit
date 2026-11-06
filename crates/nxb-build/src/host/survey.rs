//! The survey of a game with no profile (`scripts/build_ipa_from_nsp.py` `survey`): what the code modules and the
//! RomFS manifest show, written as `survey.md`, and the facts the provisional profile is made from.
//!
//! The rewrite-site census (`dm/tools/insn_census.py`) is `nxb_rewrite::census`. A caller that has the text of the
//! census (`--census FILE` of `nxb profile guess`) hands it in (`census`).

use super::guess::{Facts, Guess, facts_of, guess};
use super::guess_src::{nro_modules, shader_sources, survey_inputs};
use super::nsofacts::{self as nf, Counter, area, host_hooks};
use crate::ios::nacp;
use std::collections::{BTreeSet, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// What a survey needs besides the extraction.
#[derive(Clone, Debug)]
pub struct SurveyInput {
    /// The extraction folder: `ExeFS/`, `romfs_manifest.tsv`, `romfs_nro/`, `control.nacp`.
    pub extract: PathBuf,
    /// The application title ID.
    pub app: u64,
    /// The name for a game without control data (a dump's folder).
    pub fallback_name: String,
    /// The report lines for the packages (`- base: ...`, `- update: ...`, `- DLC: ...`).
    pub package_lines: Vec<String>,
    /// The newest update's version number (`None`: no update).
    pub update: Option<u64>,
    /// The game has DLC.
    pub dlc: bool,
    /// The engine folder (the host hooks come from its `dm`).
    pub engine: PathBuf,
    /// The text of the rewrite-site census. `None`: the survey counts the sites of each module itself.
    pub census: Option<String>,
}

/// The result of a survey.
#[derive(Clone, Debug)]
pub struct Survey {
    /// The facts.
    pub facts: Facts,
    /// The game name (control data, else the fallback).
    pub name: String,
    /// The display version (`?` when unknown).
    pub display: String,
    /// The version tag of the package the survey read (`v0`, `v<update>`).
    pub version: String,
    /// The guessed keys.
    pub guess: Guess,
    /// The report (`survey.md`).
    pub report: String,
    /// The notes the survey prints.
    pub notes: Vec<String>,
}

/// Why a survey stops.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SurveyError {
    /// The game cannot be ported (a message the build prints as `[RED]`).
    Refused(String),
    /// Something in the files is wrong.
    Failed(String),
}

impl std::fmt::Display for SurveyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SurveyError::Refused(s) | SurveyError::Failed(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for SurveyError {}

/// `check_aarch64`: a 32-bit ARM program is refused.
pub fn check_aarch64(exefs: &Path, what: &str) -> Result<(), SurveyError> {
    if super::guess::aarch64(&exefs.join("main.npdm")) {
        return Ok(());
    }
    Err(SurveyError::Refused(format!(
        "{what} is a 32-bit ARM program (main.npdm: the 64-bit instruction flag is clear): Apple silicon cannot run 32-bit ARM code, and this repository runs a game's own AArch64 code, so it cannot be ported"
    )))
}

/// The census of the modules of `ExeFS` (the script runs `insn_census.py` on each, `rtld` left out: the host replaces
/// it). The first line of each module names its path, as the script prints the path it was given.
pub fn census_text<'a>(
    exefs: &Path,
    names: impl Iterator<Item = &'a str>,
) -> Result<String, SurveyError> {
    let mut out = String::new();
    for n in names.filter(|n| *n != "rtld") {
        let path = exefs.join(n);
        let data = std::fs::read(&path)
            .map_err(|e| SurveyError::Failed(format!("[RED] {}: {e}", path.display())))?;
        let text = nxb_rewrite::census(&path.to_string_lossy(), &data)
            .map_err(|e| SurveyError::Failed(e.to_string()))?;
        out.push_str(&text);
    }
    Ok(out)
}

/// The output of `nso_syms.py <host> - <ExeFS>`: the SDK surface against the host hooks (`forced` hooks included).
/// Areas of equal count come in sorted order (the script's order follows its set iteration).
pub fn nso_syms_lines(
    exefs: &Path,
    hooks_forced: &HashSet<String>,
) -> Result<Vec<String>, SurveyError> {
    let imp = |m: &str| -> Result<Vec<nf::Sym>, SurveyError> {
        let (img, ..) = super::guess::read_nso(&exefs.join(m))
            .map_err(|e| SurveyError::Failed(format!("[RED] {e}")))?;
        nf::dynsyms(&img).map_err(SurveyError::Failed)
    };
    let ib: BTreeSet<String> = imp("main")?
        .into_iter()
        .filter(|s| !s.1)
        .map(|s| s.0)
        .collect();
    let eb: BTreeSet<String> = imp("sdk")?
        .into_iter()
        .filter(|s| s.1 && (s.2 == 1 || s.2 == 2))
        .map(|s| s.0)
        .collect();
    let present = hooks_forced.iter().filter(|h| eb.contains(*h)).count();
    let mut o = vec![
        format!(
            "HLE hooks in host: {} | present in new game sdk exports: {present}",
            hooks_forced.len()
        ),
        format!(
            "main imports: old game 0, new game {n}, shared 0, new-only {n}",
            n = ib.len()
        ),
        String::new(),
        "new game main imports by area (total / not imported by old game):".into(),
    ];
    let mut allb = Counter::default();
    for s in &ib {
        allb.add(&area(s), 1);
    }
    for (k, v) in allb.most_common(40) {
        o.push(format!("  {k:<22} {v:>5} / {v:>4}"));
    }
    o.push(String::new());
    o.push("examples of new-only imports per area:".into());
    for (k, _) in allb.most_common(14) {
        let ex: Vec<&str> = ib
            .iter()
            .filter(|s| area(s) == k)
            .take(4)
            .map(String::as_str)
            .collect();
        o.push(format!("  {k}: {}", ex.join(", ")));
    }
    Ok(o)
}

fn gib(n: u64) -> f64 {
    n as f64 / 2f64.powi(30)
}

fn mib(n: u64) -> f64 {
    n as f64 / 1048576.0
}

fn join(s: &BTreeSet<String>, sep: &str) -> String {
    s.iter().cloned().collect::<Vec<_>>().join(sep)
}

/// `survey(...)` from an extraction folder.
pub fn survey(i: &SurveyInput) -> Result<Survey, SurveyError> {
    let ex = &i.extract;
    let hooks = host_hooks(&i.engine.join("dm"), false);
    let hooks_forced = host_hooks(&i.engine.join("dm"), true);
    let display = nacp::version(&ex.join("control.nacp"));
    let mut title = nacp::name(&ex.join("control.nacp"));
    if title.is_empty() {
        title = i.fallback_name.clone();
    }
    let what = format!(
        "{} ({:016X})",
        if title.is_empty() { "?" } else { &title },
        i.app
    );
    check_aarch64(&ex.join("ExeFS"), &what)?;
    let f = facts_of(&ex.join("ExeFS"), &ex.join("romfs_manifest.tsv"))
        .map_err(|e| SurveyError::Failed(format!("[RED] {e}")))?;
    let (mods, rom) = (&f.mods, &f.romfs);
    let q = |s: &str| {
        if s.is_empty() {
            "?".to_string()
        } else {
            s.to_string()
        }
    };
    let mut r: Vec<String> = vec![
        format!("# Survey: {} ({:016X})", q(&title), i.app),
        String::new(),
        format!("- display version: {}", q(&display)),
    ];
    r.extend(i.package_lines.iter().cloned());
    r.extend([
        "".into(),
        "## Code modules".into(),
        "".into(),
        "| module | what | build ID | text / ro / data (MB) | bss (MB) |".into(),
        "|---|---|---|---|---|".into(),
    ]);
    for (n, m) in mods {
        let sz: Vec<String> = m
            .sizes
            .iter()
            .map(|&z| format!("{:.1}", mib(z as u64)))
            .collect();
        r.push(format!(
            "| {n} | {} | `{}` | {} | {:.1} |",
            m.kind,
            m.bid,
            sz.join(" / "),
            mib(m.bss as u64)
        ));
    }
    if let Some(p) = f.program {
        r.push(format!("\n- main.npdm program ID: `{p:016X}`"));
    }
    if ex.join("ExeFS/main.npdm").exists() {
        r.push(format!("- system resource: {:.0} MB", mib(f.sysres)));
    }
    r.push(format!(
        "- SDK: {} (Mario: 16_2_0 / 16_2_12)",
        if f.sdkver.is_empty() {
            "?".into()
        } else {
            join(&f.sdkver, ", ")
        }
    ));
    r.push(format!(
        "- SDK components: {}",
        if f.comps.is_empty() {
            "\u{2014}".into()
        } else {
            join(&f.comps, ", ")
        }
    ));
    if !f.unity.is_empty() {
        r.push(format!("- Unity: {}", join(&f.unity, ", ")));
    }
    let markers: Vec<String> = f
        .markers
        .iter()
        .map(|(k, v)| format!("{k} ({})", v.join(", ")))
        .collect();
    r.push(format!(
        "- engine and API markers: {}",
        if markers.is_empty() {
            "\u{2014}".into()
        } else {
            markers.join("; ")
        }
    ));
    let extra: Vec<&String> = mods
        .iter()
        .map(|m| &m.0)
        .filter(|n| !["main", "sdk", "rtld"].contains(&n.as_str()))
        .collect();

    r.extend([
        String::new(),
        format!("## RomFS: {} files, {:.2} GB", rom.n, gib(rom.bytes)),
        String::new(),
        "| top level | files | GB |".into(),
        "|---|---|---|".into(),
    ]);
    for (t, n) in rom.tops.most_common(20) {
        r.push(format!("| {t} | {n} | {:.2} |", gib(rom.topb.get(t))));
    }
    r.extend([
        String::new(),
        "| header magic | files | GB |".into(),
        "|---|---|---|".into(),
    ]);
    for (m, n) in rom.mags.most_common(usize::MAX) {
        r.push(format!("| {m} | {n} | {:.2} |", gib(rom.magb.get(m))));
    }
    let exts: Vec<String> = rom
        .exts
        .most_common(20)
        .iter()
        .map(|(e, n)| format!("`{e}` {n}"))
        .collect();
    r.extend([String::new(), format!("extensions: {}", exts.join(", "))]);

    r.extend([
        String::new(),
        "## Rewrite-site census (insn_census.py)".into(),
        String::new(),
        "```".into(),
    ]);
    let census = match &i.census {
        Some(c) => c.clone(),
        None => census_text(&ex.join("ExeFS"), mods.iter().map(|(n, _)| n.as_str()))?,
    };
    r.extend(census.trim_end().lines().map(str::to_string));
    r.extend([
        "```".into(),
        String::new(),
        "## Imports vs host hooks (nso_syms.py)".into(),
        String::new(),
        "```".into(),
    ]);
    r.extend(nso_syms_lines(&ex.join("ExeFS"), &hooks_forced)?);
    r.push("```".into());

    let g = guess(&f, &hooks).map_err(SurveyError::Failed)?;
    let keep: Vec<String> = g
        .value("modules")
        .unwrap_or("")
        .split_whitespace()
        .map(str::to_string)
        .collect();
    let nro_dir = ex.join("romfs_nro");
    let mut nros = Vec::new();
    collect_nros(&nro_dir, &mut nros);
    nros.sort();
    let nro_add = if nros.is_empty() {
        Vec::new()
    } else {
        nro_modules(&ex.join("ExeFS"), &nros, &keep, &hooks).map_err(SurveyError::Failed)?
    };
    let inp = survey_inputs(
        &f,
        &keep,
        nro_dir.is_dir().then_some(nro_dir.as_path()),
        &nro_add,
    )
    .map_err(SurveyError::Failed)?;
    let s = shader_sources(&inp);
    let (vulkan, gl) = (s.graphics.0 == "vulkan", s.graphics.0 == "gl");
    let main = f
        .module("main")
        .ok_or_else(|| SurveyError::Failed("no main module".into()))?;
    let mut notes: Vec<String> = Vec::new();
    if !vulkan && !gl && !main.imports.contains("nvnBootstrapLoader") {
        notes.push("main imports neither nvnBootstrapLoader nor Vulkan functions nor OpenGL through EGL: the host implements only NVN, Vulkan and OpenGL".into());
    }
    if vulkan {
        notes.push(format!("{}; the NVN shader steps are skipped{}", s.graphics.1, if s.vk_capture.0 { ", and its shaders are captured by booting it on the Mac (no SPIR-V file in the RomFS)" } else { "" }));
    }
    if gl {
        notes.push(format!("{}; the NVN shader steps are skipped: gl_shaders.py translates its GLSL sources (MVGL archives, main's strings{}", s.graphics.1, if s.gl_capture.0 { ", and the shaders it puts together at run time, captured by booting it on the Mac)" } else { ")" }));
    }
    if s.glslc_capture.0 {
        notes.push(s.glslc_capture.1.clone());
    }
    if s.ue_decode.0 {
        notes.push(format!("{} Unreal pak(s): version 11 paks keep the shaders compressed in shader code archives (Octopath Traveler 0), decoded by booting the game on the Mac; legacy paks (version 4, DRAGON QUEST XI S) and version 11 paks without shader code archives (OCTOPATH TRAVELER II) keep them inline in the cooked packages, which the shader split reads file by file", rom.exts.get(".pak")));
    }
    if !extra.is_empty() {
        let list: Vec<String> = extra
            .iter()
            .map(|n| format!("{n} ({})", f.module(n).map_or("", |m| m.kind.as_str())))
            .collect();
        let mut n = format!(
            "modules besides main / sdk: {}; the provisional profile loads {} (load order main, subsdk*, sdk)",
            list.join(", "),
            g.value("modules").unwrap_or("")
        );
        if !nro_add.is_empty() {
            let names: Vec<&str> = nro_add.iter().map(|m| m.0.as_str()).collect();
            let _ = write!(
                n,
                "; the build adds {}: the NROs import from {}",
                names.join(", "),
                if nro_add.len() == 1 { "it" } else { "them" }
            );
        }
        notes.push(n);
    }
    if rom.mags.get("UnityFS") != 0 || f.markers.iter().any(|m| m.0 == "Unity IL2CPP") {
        notes.push("Unity: the shaders are Shader objects in UnityFS bundles and data.unity3d, which shader_split.py reads through unityfs.py (written for Unity 2020.3's Switch format, also read for 2017.4, 2018.3, 2019.4, 2021.2, 2021.3, 2022.3, 6000.0 and 6000.3; check other Unity versions first)".into());
    }
    if f.glslc != 0 {
        notes.push(format!("main has {} GLSLC compiler output magics (0x19866891): shader_split.py takes those that are whole outputs (13 Sentinels and the KTGL games compile their shaders into main; Unity's player has 11 of its own)", f.glslc));
    }
    let m = |k: &str| rom.mags.get(k) != 0;
    if m("Trinity pack") {
        notes.push("Trinity packs (Game Freak, arc/data.trpfs with its index data.trpfd): the shaders are BNSH inside Oodle Mermaid streams; shader_split.py decodes them with the game's own OodleLZ_Decompress in the Mac host, which dm/host/hle_oodle.c finds by the library's name string".into());
    } else if m("PlatinumGames PKZ") {
        notes.push("PlatinumGames PKZ archives (pkzl; Bayonetta 3): DAT archives inside, compressed per entry (OodleKraken, ZStandard or None); the shaders are shader objects (VSO / PSO / CSO \u{2026}) in DATs, which shader_split.py reads (pkz_entries, pg_shader), Oodle Kraken through the game's own streaming decoder in the Mac host (dm/host/hle_oodle.c)".into());
    } else if m("MT Framework ARC") {
        notes.push("MT Framework ARC archives (Capcom, The Great Ace Attorney Chronicles): the shaders are in a shader package (SPK) inside one of them, which shader_split.py reads (mtarc_members, mtspk_shaders)".into());
    } else if m("Bezel Engine archive") {
        notes.push("NintendoWare Bezel Engine archives (.bea, magic SCNE; Big Brain Academy: Brain vs. Brain): every entry is a zstd frame, and the shaders are BNSH in their .bfsha entries, which shader_split.py reads (bea_members)".into());
    } else if m("CRI CPK") && f.glslc == 0 {
        notes.push("CRI CPK archives (Bayonetta 2; Vanillaware's and Atlus's games keep their shaders elsewhere): files compressed one by one with CRILAYLA; shader_split.py decodes every entry (dm/gfx/tools/cpk.py, cpk_jobs) and scans it as a loose file (Bayonetta 2: BNSH files in PlatinumGames DAT archives)".into());
    } else if m("XNB") && f.markers.iter().any(|k| k.0 == "MonoGame") {
        notes.push("MonoGame (XNB content): the shaders are MonoGame effects (MGFX version 8, NVN control sections and code areas, or MonoGame 3.0's version 5, whole GLSLC outputs) in the XNB files EffectReader reads, in .mgfxo files and SpriteBatch's in main, which shader_split.py reads (mgfx_effect; Celeste, Bastion), a compressed XNB (MonoGame's LZX or LZ4, Stardew Valley) after decompressing it (xnb_decompress)".into());
    } else if !(m("BNSH") || m("BAHS")) && !m("UnityFS") && !vulkan && !gl && f.glslc == 0 {
        notes.push("no BNSH / BAHS at the top level: the shaders are inside compressed or other containers or in main, and must be located first".into());
    }
    let major: BTreeSet<String> = f
        .sdkver
        .iter()
        .map(|v| v.split('_').next().unwrap_or("").to_string())
        .collect();
    if major.is_empty() {
        notes.push("no SDK version string found".into());
    } else if major.len() != 1 || !major.contains("16") {
        notes.push(format!("SDK {}, not Mario's 16: NVN object sizes, API version and service layouts differ by SDK (the host was converted from D2R's SDK 12 to SDK 16 sizes); check them one by one per docs/reference/nvn-objects.md", join(&major, "/")));
    }
    if r.iter()
        .any(|l| l.split_whitespace().next() == Some("nn::ro"))
    {
        let (nro, nrr) = (rom.exts.get(".nro"), rom.exts.get(".nrr"));
        notes.push(format!("imports nn::ro (loads NRO modules at run time): the RomFS has {nro} .nro files and {nrr} .nrr files; registered NROs are collected and rewritten beside the other modules{} and embedded in the iOS app (gen_embed.py)", if nrr != 0 && nro == 0 { " (look inside archives for the registered modules)" } else { "" }));
    }
    if i.dlc {
        notes.push("DLC present: it is extracted into the IPA and the host reports the indices as owned and mounts them; check that the host hooks every aoc / fs add-on content call main imports".into());
    }
    r.extend([
        String::new(),
        "## What it means for a port".into(),
        String::new(),
    ]);
    if notes.is_empty() {
        r.push("- \u{2014}".into());
    } else {
        r.extend(notes.iter().map(|n| format!("- {n}")));
    }
    r.extend([String::new(), "## Provisional profile keys (dm/games/profile_guess.py)".into(), String::new(), "What the game files decide; mem_used follows the rewrite, and a game with virtual address memory that sets a heap gets a heap check on the Mac (scripts/build_ipa_from_nsp.py heap_check).".into(), String::new(), "```".into()]);
    for (k, (v, why)) in &g.0 {
        r.push(format!(
            "{k}\t{v}{}",
            if why.is_empty() {
                String::new()
            } else {
                format!("\t# {why}")
            }
        ));
    }
    r.extend([
        "```".into(),
        String::new(),
        "Next steps: check the profile values against the first run of the game.".into(),
    ]);
    let version = match i.update {
        None => "v0".to_string(),
        Some(v) => format!("v{}", if v == 0 { 1 } else { v }),
    };
    Ok(Survey {
        facts: f.clone(),
        name: title,
        display: if display.is_empty() {
            "?".into()
        } else {
            display
        },
        version,
        guess: g,
        report: format!("{}\n", r.join("\n")),
        notes,
    })
}

fn collect_nros(d: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(d) else { return };
    for p in rd.flatten().map(|e| e.path()) {
        if p.is_dir() {
            collect_nros(&p, out);
        } else if p.extension().is_some_and(|e| e == "nro") {
            out.push(p);
        }
    }
}
