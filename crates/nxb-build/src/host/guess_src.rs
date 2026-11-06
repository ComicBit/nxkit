//! `profile_guess.py`: the shader sources a game's files show (`shader_sources`), and the modules the NROs and main
//! need that a profile's list leaves out (`nro_modules`, `main_modules`, `modules_supplying`).

use super::guess::{
    Facts, GPU_DRIVERS, ModFacts, load_order, nso_files, read_nso, sort_load_order,
};
use super::nsofacts::{self as nf, glsl_text};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

fn module_exports(exefs: &Path) -> Result<BTreeMap<String, (BTreeSet<String>, String)>, String> {
    let mut exp = BTreeMap::new();
    for p in nso_files(exefs, &[]) {
        let (img, ..) = read_nso(&p)?;
        let syms = nf::dynsyms(&img)?;
        let exports: BTreeSet<String> = syms
            .iter()
            .filter(|s| s.1 && (s.2 == 1 || s.2 == 2))
            .map(|s| s.0.clone())
            .collect();
        let kind = super::guess::kind_of(&nf::sdk_components(&img), &exports);
        exp.insert(
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            (exports, kind),
        );
    }
    Ok(exp)
}

/// `modules_supplying(exefs, want, modules, hooks)`: the modules left out of the list that export a symbol of `want`
/// which no listed module and no host hook supplies, `(module, kind)` in load order, never a GPU driver.
pub fn modules_supplying(
    exefs: &Path,
    want: &BTreeSet<String>,
    modules: &[String],
    hooks: &HashSet<String>,
) -> Result<Vec<(String, String)>, String> {
    let exp = module_exports(exefs)?;
    let mut have: HashSet<&str> = hooks.iter().map(String::as_str).collect();
    for m in modules {
        if let Some((e, _)) = exp.get(m) {
            have.extend(e.iter().map(String::as_str));
        }
    }
    let want: BTreeSet<&String> = want.iter().filter(|s| !have.contains(s.as_str())).collect();
    let mut names: Vec<&String> = exp.keys().collect();
    names.sort_by_key(|m| load_order(m));
    Ok(names
        .into_iter()
        .filter(|m| !modules.contains(m))
        .filter_map(|m| {
            let (e, kind) = &exp[m];
            (want.iter().any(|s| e.contains(*s)) && !GPU_DRIVERS.contains(&kind.as_str()))
                .then(|| (m.clone(), kind.clone()))
        })
        .collect())
}

/// `nro_modules(exefs, nros, modules, hooks)`: what the loose NROs import from modules the list leaves out.
pub fn nro_modules(
    exefs: &Path,
    nros: &[PathBuf],
    modules: &[String],
    hooks: &HashSet<String>,
) -> Result<Vec<(String, String)>, String> {
    let (mut want, mut have) = (BTreeSet::new(), BTreeSet::new());
    for p in nros {
        let b = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
        for (s, defined, bind) in nf::dynsyms(&b)? {
            if !defined {
                want.insert(s);
            } else if bind == 1 || bind == 2 {
                have.insert(s);
            }
        }
    }
    let want = want.difference(&have).cloned().collect();
    modules_supplying(exefs, &want, modules, hooks)
}

/// `main_modules(exefs, modules, hooks)`: the subsdks a hand-written list leaves out that main imports from.
pub fn main_modules(
    exefs: &Path,
    modules: &[String],
    hooks: &HashSet<String>,
) -> Result<Vec<(String, String)>, String> {
    if nso_files(exefs, modules).is_empty() {
        return Ok(Vec::new());
    }
    let (img, ..) = read_nso(&exefs.join("main"))?;
    let want = nf::dynsyms(&img)?
        .into_iter()
        .filter(|s| !s.1)
        .map(|s| s.0)
        .collect();
    modules_supplying(exefs, &want, modules, hooks)
}

/// The names the loose NROs in a folder import (`romfs_nro_imports`).
pub fn romfs_nro_imports(romfs: &Path) -> Result<BTreeSet<String>, String> {
    let mut out = BTreeSet::new();
    fn walk(d: &Path, out: &mut BTreeSet<String>) -> Result<(), String> {
        let Ok(rd) = std::fs::read_dir(d) else {
            return Ok(());
        };
        let mut es: Vec<_> = rd.flatten().map(|e| e.path()).collect();
        es.sort();
        for p in es {
            if p.is_dir() {
                walk(&p, out)?;
            } else if p.to_string_lossy().ends_with(".nro") {
                let b = std::fs::read(&p).map_err(|e| e.to_string())?;
                if b.get(0x10..0x14) == Some(b"NRO0") {
                    out.extend(nf::dynsyms(&b)?.into_iter().filter(|s| !s.1).map(|s| s.0));
                }
            }
        }
        Ok(())
    }
    walk(romfs, &mut out)?;
    Ok(out)
}

/// `romfs_glsl_files`: the `.vert` / `.frag` files that hold GLSL as text.
pub fn romfs_glsl_files(romfs: &Path) -> usize {
    fn walk(d: &Path, n: &mut usize) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for p in rd.flatten().map(|e| e.path()) {
            if p.is_dir() {
                walk(&p, n);
            } else {
                let l = p.to_string_lossy().to_lowercase();
                if l.ends_with(".vert") || l.ends_with(".frag") {
                    let mut h = vec![0u8; 4096];
                    let k = std::fs::File::open(&p)
                        .and_then(|mut f| std::io::Read::read(&mut f, &mut h))
                        .unwrap_or(0);
                    *n += usize::from(glsl_text(&h[..k]));
                }
            }
        }
    }
    let mut n = 0;
    walk(romfs, &mut n);
    n
}

/// The counts `shader_sources` decides from.
#[derive(Clone, Debug, Default)]
pub struct SourceInputs {
    /// Main's imports.
    pub imports: BTreeSet<String>,
    /// Complete GLSL sources in main.
    pub glsl_src: usize,
    /// The loaded modules that define `glslcCompile` (in load order).
    pub glslc_mods: Vec<String>,
    /// SPIR-V files.
    pub spirv: u64,
    /// Unreal paks with shader code archives.
    pub unreal_paks: u64,
    /// Trinity packs.
    pub trinity_packs: u64,
    /// Unreal paks with inline shaders.
    pub legacy_paks: u64,
    /// `glShaderSource` names in main.
    pub gl_names: usize,
    /// GLSL source strings in main.
    pub gl_src: usize,
    /// GLSL-named files in the RomFS.
    pub gl_files: u64,
    /// PKZ archives with Oodle.
    pub oodle_pkz: u64,
    /// What the NROs import.
    pub nro_imports: BTreeSet<String>,
    /// GFPAK archives with Oodle.
    pub oodle_gfpak: u64,
    /// Unreal 3 packages with Oodle.
    pub oodle_ue3: u64,
    /// Frostbite superbundles.
    pub frostbite: bool,
    /// GLSL source files in the RomFS.
    pub romfs_glsl: u64,
}

/// What `shader_sources` decides: `(value, reason)` of each key.
#[derive(Clone, Debug)]
pub struct Sources {
    /// `vulkan`, `gl` or `nvn`, and why.
    pub graphics: (String, String),
    /// A Vulkan game without SPIR-V files.
    pub vk_capture: (bool, String),
    /// An OpenGL game that puts its shaders together at run time.
    pub gl_capture: (bool, String),
    /// A game that compiles GLSL at run time.
    pub glslc_capture: (bool, String),
    /// Oodle-compressed Unreal shader archives.
    pub ue_decode: (bool, String),
    /// Oodle-compressed packs.
    pub oodle_packs: (bool, String),
    /// Inline Unreal shaders.
    pub ue_legacy: (bool, String),
}

/// `shader_sources(...)`.
pub fn shader_sources(i: &SourceInputs) -> Sources {
    let has = |s: &str| i.imports.contains(s);
    let vk = i
        .imports
        .iter()
        .filter(|s| {
            let b = s.as_bytes();
            b.len() > 2
                && b.starts_with(b"vk")
                && b[2].is_ascii_uppercase()
                && *s != "vkGetInstanceProcAddr"
        })
        .count();
    let vulkan = vk != 0 && !has("nvnBootstrapLoader");
    let nro_nvn = i.nro_imports.contains("nvnBootstrapLoader");
    let glsl_any = i.glsl_src != 0 || i.romfs_glsl != 0;
    let own_glslc = has("nvnBootstrapLoader")
        && has("glslcCompile")
        && i.glsl_src != 0
        && !i.glslc_mods.is_empty();
    let gl = !vulkan && !nro_nvn && has("eglCreateWindowSurface") && i.gl_names != 0 && !own_glslc;
    let nro_glslc =
        !vulkan && !gl && i.nro_imports.contains("glslcCompile") && !i.glslc_mods.is_empty();
    let mods = i.glslc_mods.join(", ");
    let or_none = if mods.is_empty() {
        "no module it loads"
    } else {
        mods.as_str()
    };
    let graphics = if vulkan {
        (
            "vulkan".into(),
            format!(
                "main imports {vk} Vulkan functions and not nvnBootstrapLoader: the host's Vulkan layer (dm/gfx/vk/hle_vk.m)"
            ),
        )
    } else if gl {
        ("gl".into(), "main imports EGL and names glShaderSource: it renders with OpenGL and compiles GLSL, the host's GL layer (dm/gfx/gl/hle_gl.m)".into())
    } else {
        let why = if nro_nvn && !has("nvnBootstrapLoader") {
            "the NROs the game loads import nvnBootstrapLoader"
        } else if own_glslc && has("eglCreateWindowSurface") && i.gl_names != 0 {
            "main imports nvnBootstrapLoader and compiles its GLSL with GLSLC; its EGL and glShaderSource are left unused (Rayman 30th Anniversary Edition)"
        } else {
            "main imports nvnBootstrapLoader"
        };
        ("nvn".into(), why.into())
    };
    let glslc_on = (!vulkan && !gl && glsl_any && has("glslcCompile") && !i.glslc_mods.is_empty())
        || nro_glslc;
    let glslc_why = if nro_glslc && !(glsl_any && has("glslcCompile")) {
        format!(
            "the NROs the game loads call glslcCompile in {mods}: shaders compiled at run time (Portal), captured by booting it on the Mac"
        )
    } else if i.romfs_glsl != 0 && i.glsl_src == 0 {
        format!(
            "the RomFS holds {} GLSL source files and main calls glslcCompile in {or_none}: shaders compiled at run time (Turrican Anthology Vol. I), captured by booting it on the Mac",
            i.romfs_glsl
        )
    } else {
        format!(
            "main holds {} GLSL sources and calls glslcCompile in {or_none}: shaders compiled at run time (Fire Emblem: Three Houses), captured by booting it on the Mac",
            i.glsl_src
        )
    };
    let plain = !vulkan && !gl;
    let packs_on = plain
        && (i.trinity_packs != 0
            || i.oodle_pkz != 0
            || i.oodle_gfpak != 0
            || i.oodle_ue3 != 0
            || i.frostbite);
    let packs_what = if i.trinity_packs != 0 {
        format!(
            "{} Trinity pack file(s) (ONEPACK, arc/data.trpfs: Pok\u{e9}mon Legends: Z-A)",
            i.trinity_packs
        )
    } else if i.frostbite {
        "Frostbite superbundles (Data/layout.toc, cas files: EA SPORTS FC 27), the shaders in their CompiledBytecode resources".into()
    } else if i.oodle_pkz != 0 {
        format!(
            "{} PlatinumGames PKZ archive(s) with Oodle Kraken entries (Bayonetta 3)",
            i.oodle_pkz
        )
    } else if i.oodle_gfpak != 0 {
        format!(
            "{} Game Freak GFPAK archive(s) with Oodle Kraken entries (Pok\u{e9}mon Legends: Arceus)",
            i.oodle_gfpak
        )
    } else {
        format!(
            "{} Unreal Engine 3 package(s) with Oodle-compressed chunks (Batman: Arkham Knight, its combined shader cache among them)",
            i.oodle_ue3
        )
    };
    Sources {
        graphics,
        vk_capture: (vulkan && i.spirv == 0, "no SPIR-V file in the RomFS: the game generates its shaders at run time (Super Mario Galaxy), captured by booting it on the Mac".into()),
        gl_capture: (gl && i.gl_src == 0 && i.gl_files == 0, "no GLSL source in the RomFS or main: the game puts its shaders together at run time (L\u{d6}VE: Balatro), captured by booting it on the Mac".into()),
        glslc_capture: (glslc_on, glslc_why),
        ue_decode: (plain && i.unreal_paks != 0, format!("{} Unreal pak(s): the shaders are Oodle-compressed in their shader code archives (Octopath Traveler 0), decoded by booting the game on the Mac", i.unreal_paks)),
        oodle_packs: (packs_on, format!("{packs_what}, Oodle-compressed: the shader split decodes them with the game's own decoder in the Mac host (dm/host/hle_oodle.c DM_OODLE_SERVE)")),
        ue_legacy: (plain && i.legacy_paks != 0, format!("{} Unreal pak(s) with the shaders inline in the cooked packages (legacy, version 4, UE 4.18: DRAGON QUEST XI S, LZ5, decoded by the game's own decoder in the Mac host, dm/host/hle_oodle.c DM_OODLE_SERVE; version 11 without shader code archives, UE 4.27: OCTOPATH TRAVELER II, Zlib, or with the cooked packages in an IoStore container beside it: Mario & Luigi: Brothership, Oodle): the shader split reads them file by file", i.legacy_paks)),
    }
}

/// The survey's inputs of `shader_sources` from the facts and a manifest (the survey has no RomFS folder, so no
/// pak indexes, no archive headers: the manifest's counts stand in, as in `scripts/build_ipa_from_nsp.py` `survey`).
/// `nro_dir` is the `romfs_nro` folder of the extraction, `nro_add` what `nro_modules` adds.
pub fn survey_inputs(
    f: &Facts,
    keep: &[String],
    nro_dir: Option<&Path>,
    nro_add: &[(String, String)],
) -> Result<SourceInputs, String> {
    let main: &ModFacts = f.module("main").ok_or("no main module")?;
    let mut mods: Vec<String> = keep
        .iter()
        .cloned()
        .chain(nro_add.iter().map(|m| m.0.clone()))
        .collect();
    mods.sort();
    mods.dedup();
    sort_load_order(&mut mods);
    let r = &f.romfs;
    let unreal = f.main_markers.contains("Unreal") || f.main_markers.contains("Unreal 4");
    Ok(SourceInputs {
        imports: main.imports.clone(),
        glsl_src: f.glsl_src,
        glslc_mods: mods
            .into_iter()
            .filter(|m| {
                f.module(m)
                    .is_some_and(|x| x.exports.contains("glslcCompile"))
            })
            .collect(),
        spirv: r.mags.get("SPIR-V"),
        unreal_paks: if unreal { r.exts.get(".pak") } else { 0 },
        trinity_packs: r.mags.get("Trinity pack"),
        gl_names: f.gl_names,
        gl_src: f.gl_src,
        gl_files: [".mvgl", ".vert", ".frag"]
            .iter()
            .map(|e| r.exts.get(e))
            .sum(),
        oodle_pkz: r.mags.get("PlatinumGames PKZ"),
        nro_imports: match nro_dir {
            Some(d) => romfs_nro_imports(d)?,
            None => BTreeSet::new(),
        },
        ..SourceInputs::default()
    })
}
