//! The facts of `profile_guess.py` `facts_of`: what the code modules and the RomFS file headers of a game show.

use super::nsofacts::{self as nf, Counter};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Header magics (`MAGICS`): bytes, name.
const MAGICS: &[(&[u8], &str)] = &[
    (b"UnityFS\0", "UnityFS"),
    (b"UnityWeb", "UnityWeb"),
    (b"UnityRaw", "UnityRaw"),
    (b"BNSH", "BNSH"),
    (b"BAHS", "BAHS"),
    (b"SARC", "SARC"),
    (b"Yaz0", "Yaz0"),
    (&[0x28, 0xb5, 0x2f, 0xfd], "zstd"),
    (b"FRES", "bfres"),
    (b"BNTX", "BNTX"),
    (b"BWAV", "BWAV"),
    (b"FSAR", "BFSAR"),
    (b"CPK ", "CRI CPK"),
    (b"CRID", "CRI USM"),
    (b"@UTF", "CRI UTF"),
    (b"AKPK", "Wwise pck"),
    (b"BKHD", "Wwise bnk"),
    (b"FSB5", "FMOD"),
    (b"\x89PNG", "PNG"),
    (b"RIFF", "RIFF"),
    (b"OggS", "Ogg"),
    (b"PK\x03\x04", "zip"),
    (b"NSO0", "NSO"),
    (&[0x03, 0x02, 0x23, 0x07], "SPIR-V"),
    (b"ONEPACK\0", "Trinity pack"),
    (b"ARC\0", "MT Framework ARC"),
    (b"SCNE", "Bezel Engine archive"),
    (b"XNB", "XNB"),
    (b"pkzl", "PlatinumGames PKZ"),
];
const PACKED: &[&str] = &[
    "UnityFS",
    "UnityWeb",
    "Yaz0",
    "zstd",
    "BWAV",
    "CRI CPK",
    "CRI USM",
    "Wwise pck",
    "FMOD",
    "PNG",
    "Ogg",
    "zip",
    "Trinity pack",
    "MT Framework ARC",
    "Bezel Engine archive",
    "PlatinumGames PKZ",
];
/// Engine and API markers (`MARKERS`).
const MARKERS: &[(&[u8], &str)] = &[
    (b"nvnBootstrapLoader", "NVN"),
    (b"vkCreateInstance", "Vulkan"),
    (b"eglGetProcAddress", "EGL / OpenGL"),
    (b"global-metadata.dat", "Unity IL2CPP"),
    (b"UnityEngine", "Unity"),
    (b"sead::", "Nintendo sead"),
    (b"agl::", "Nintendo agl"),
    (b"gsys::", "Nintendo gsys"),
    (b"Unreal", "Unreal"),
    (b"UE4", "Unreal 4"),
    (b"Havok", "Havok"),
    (b"CRIWARE", "CRIWARE"),
    (b"Wwise", "Wwise"),
    (b"MonoGame.Framework", "MonoGame"),
    (b"FMOD mixer", "FMOD"),
];
const GLSLC_MAGIC: [u8; 4] = [0x91, 0x68, 0x86, 0x19];

/// What one code module shows.
#[derive(Clone, Debug)]
pub struct ModFacts {
    /// The first 16 bytes of the build ID, hex.
    pub bid: String,
    /// Text, ro and data sizes.
    pub sizes: Vec<usize>,
    /// The bss size.
    pub bss: usize,
    /// The patch area goes before the base (`patch_nso.py` FAR).
    pub far: bool,
    /// Undefined dynamic symbols.
    pub imports: BTreeSet<String>,
    /// Defined global and weak dynamic symbols.
    pub exports: BTreeSet<String>,
    /// SDK components named in the image.
    pub comps: BTreeSet<String>,
    /// A short name for what the module is.
    pub kind: String,
}

/// RomFS counts.
#[derive(Clone, Debug, Default)]
pub struct RomFacts {
    /// Files.
    pub n: u64,
    /// Bytes.
    pub bytes: u64,
    /// Files by top level (two levels under a deep tree).
    pub tops: Counter,
    /// Bytes by top level.
    pub topb: Counter,
    /// Files by lower-case extension.
    pub exts: Counter,
    /// Bytes by extension.
    pub extb: Counter,
    /// Files of a packed format by extension.
    pub extc: Counter,
    /// Files by header magic.
    pub mags: Counter,
    /// Bytes by header magic.
    pub magb: Counter,
    /// Files by first folder.
    pub dirs: Counter,
    /// The biggest file: (size, path).
    pub biggest: (u64, String),
    /// Files at the root.
    pub root: BTreeSet<String>,
}

/// What the profile rules and the survey report need.
#[derive(Clone, Debug, Default)]
pub struct Facts {
    /// The modules in name order.
    pub mods: Vec<(String, ModFacts)>,
    /// Built for the Switch 2.
    pub ounce: bool,
    /// Marker name to the modules that hold it, in order of first sight.
    pub markers: Vec<(String, Vec<String>)>,
    /// SDK version strings.
    pub sdkver: BTreeSet<String>,
    /// MW SDK components.
    pub comps: BTreeSet<String>,
    /// Unity versions in main.
    pub unity: BTreeSet<String>,
    /// GLSLC output magics in main.
    pub glslc: usize,
    /// Complete GLSL sources in main.
    pub glsl_src: usize,
    /// `glShaderSource` names in main.
    pub gl_names: usize,
    /// GLSL source strings in main.
    pub gl_src: usize,
    /// NPDM system resource size.
    pub sysres: u64,
    /// NPDM program id.
    pub program: Option<u64>,
    /// The lowest SDK major version (0 when none).
    pub sdk: u64,
    /// The markers main holds.
    pub main_markers: BTreeSet<String>,
    /// The RomFS.
    pub romfs: RomFacts,
}

impl Facts {
    /// A module.
    pub fn module(&self, n: &str) -> Option<&ModFacts> {
        self.mods.iter().find(|(m, _)| m == n).map(|(_, f)| f)
    }
}

/// `kind(m)` of a module's components and exports.
pub fn kind_of(comps: &BTreeSet<String>, exports: &BTreeSet<String>) -> String {
    if let Some(c) = comps.iter().next() {
        return c.clone();
    }
    if exports.iter().any(|s| s.starts_with("glslc")) {
        return "GLSLC".into();
    }
    let mut c = Counter::default();
    for s in exports {
        c.add(&nf::area(s), 1);
    }
    c.most_common(1)
        .first()
        .map_or("?".into(), |(k, _)| (*k).to_string())
}

/// `nso(path)`: image, build ID (16 bytes hex), segment sizes, bss.
pub fn read_nso(p: &Path) -> Result<(Vec<u8>, String, Vec<usize>, usize), String> {
    let d = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let m = nxb_rewrite::nso::load(&d, &p.to_string_lossy()).map_err(|e| e.to_string())?;
    let bid: String = m.build_id[..0x10]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok((
        m.image,
        bid,
        m.segments.iter().map(|s| s.1).collect(),
        m.bss,
    ))
}

/// The code modules of an ExeFS folder (`NSO0` files), in name order.
pub fn nso_files(exefs: &Path, skip: &[String]) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(exefs)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    v.sort();
    v.retain(|p| {
        let n = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        !skip.contains(&n) && n != "rtld" && std::fs::read(p).is_ok_and(|b| b.starts_with(b"NSO0"))
    });
    v
}

/// `npdm`: (system resource size, program ID), (0, None) without the file.
pub fn npdm(p: &Path) -> Result<(u64, Option<u64>), String> {
    let Ok(d) = std::fs::read(p) else {
        return Ok((0, None));
    };
    let a = nf::find(&d, b"ACI0", 0);
    let u32at = |o: usize| {
        d.get(o..o + 4)
            .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    };
    let sys = if d.starts_with(b"META") {
        u64::from(u32at(0x14).ok_or("main.npdm is cut short")?)
    } else {
        0
    };
    let prog = match a {
        Some(a) => {
            let s = d.get(a + 0x10..a + 0x18).ok_or("main.npdm is cut short")?;
            Some(u64::from_le_bytes(s.try_into().unwrap_or([0; 8])))
        }
        None => None,
    };
    Ok((sys, prog))
}

/// `aarch64`: the 64-bit instruction flag of main.npdm (true without the file).
pub fn aarch64(p: &Path) -> bool {
    match std::fs::read(p) {
        Err(_) => true,
        Ok(d) => !d.starts_with(b"META") || d.get(0xc).is_some_and(|b| b & 1 != 0),
    }
}

/// `(path, size, first 32 bytes)` of every RomFS file: a manifest file, or a folder.
pub fn romfs_files(romfs: &Path) -> Result<Vec<(String, u64, Vec<u8>)>, String> {
    let mut out = Vec::new();
    if romfs.is_file() {
        let t =
            String::from_utf8_lossy(&std::fs::read(romfs).map_err(|e| e.to_string())?).into_owned();
        for l in super::pyfmt::splitlines(&t) {
            if l.starts_with('#') {
                continue;
            }
            let c: Vec<&str> = l.split('\t').collect();
            let [p, s, h] = c[..] else {
                return Err(format!(
                    "{}: a manifest row needs three cells: {l}",
                    romfs.display()
                ));
            };
            let size = s.parse().map_err(|_| format!("bad size {s}"))?;
            let head = (0..h.len() / 2)
                .map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16))
                .collect::<Result<Vec<u8>, _>>()
                .map_err(|_| format!("bad hex {h}"))?;
            out.push((p.to_string(), size, head));
        }
        return Ok(out);
    }
    fn walk(root: &Path, d: &Path, out: &mut Vec<(String, u64, Vec<u8>)>) -> Result<(), String> {
        let mut es: Vec<_> = std::fs::read_dir(d)
            .map_err(|e| format!("{}: {e}", d.display()))?
            .flatten()
            .collect();
        es.sort_by_key(|e| e.file_name());
        for e in &es {
            let p = e.path();
            if p.is_file() {
                let mut head = vec![0u8; 32];
                let n = std::io::Read::read(
                    &mut std::fs::File::open(&p).map_err(|e| e.to_string())?,
                    &mut head,
                )
                .map_err(|e| e.to_string())?;
                head.truncate(n);
                let rel = p.strip_prefix(root).map_err(|e| e.to_string())?;
                let size = p.metadata().map_err(|e| e.to_string())?.len();
                out.push((format!("/{}", rel.to_string_lossy()), size, head));
            }
        }
        for e in &es {
            if e.path().is_dir() {
                walk(root, &e.path(), out)?;
            }
        }
        Ok(())
    }
    walk(romfs, romfs, &mut out)?;
    Ok(out)
}

/// `os.path.splitext(path)[1]`.
pub fn splitext(p: &str) -> &str {
    let sep = p.rfind('/').map_or(0, |i| i + 1);
    match p.rfind('.') {
        Some(dot) if dot >= sep && p[sep..dot].chars().any(|c| c != '.') => &p[dot..],
        _ => "",
    }
}

/// `facts_of(ExeFS, RomFS)`.
pub fn facts_of(exefs: &Path, romfs: &Path) -> Result<Facts, String> {
    let mut f = exefs_facts(exefs)?;
    f.romfs = romfs_facts(romfs)?;
    Ok(f)
}

/// The ExeFS part of `facts_of`: the modules and `main.npdm`, with no RomFS facts.
pub fn exefs_facts(exefs: &Path) -> Result<Facts, String> {
    let mut f = Facts::default();
    for p in nso_files_all(exefs) {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let (img, bid, sizes, bss) = read_nso(&p)?;
        let syms = nf::dynsyms(&img).map_err(|e| format!("{}: {e}", p.display()))?;
        let end = (((img.len() + bss + 0x3FFF) & !0x3FFF) + 0x4000 + 0xFFFF) & !0xFFFF;
        let imports = syms.iter().filter(|s| !s.1).map(|s| s.0.clone()).collect();
        let exports: BTreeSet<String> = syms
            .iter()
            .filter(|s| s.1 && (s.2 == 1 || s.2 == 2))
            .map(|s| s.0.clone())
            .collect();
        let comps = nf::sdk_components(&img);
        f.sdkver.extend(nf::sdk_versions(&img));
        if name == "sdk" {
            f.ounce = nf::find(&img, b"spec: Ounce", 0).is_some();
        }
        f.comps.extend(nf::mw_components(&img));
        for (pat, mname) in MARKERS {
            if nf::find(&img, pat, 0).is_some() {
                match f.markers.iter_mut().find(|(k, _)| k == mname) {
                    Some((_, v)) => v.push(name.clone()),
                    None => f.markers.push(((*mname).to_string(), vec![name.clone()])),
                }
            }
        }
        if name == "main" {
            f.unity = nf::unity_versions(&img);
            f.glslc = nf::count(&img, &GLSLC_MAGIC);
            f.glsl_src = nf::glsl_sources(&img);
            f.gl_names = nf::count(&img, b"glShaderSource");
            f.gl_src = nf::gl_src_count(&img);
            f.main_markers = MARKERS
                .iter()
                .filter(|(pat, _)| nf::find(&img, pat, 0).is_some())
                .map(|(_, n)| (*n).to_string())
                .collect();
        }
        let kind = match name.as_str() {
            "main" => "the game".to_string(),
            "sdk" => "nnSdk".to_string(),
            "rtld" => "rtld".to_string(),
            _ => kind_of(&comps, &exports),
        };
        f.mods.push((
            name,
            ModFacts {
                bid,
                sizes,
                bss,
                far: end > 0x700_0000,
                imports,
                exports,
                comps,
                kind,
            },
        ));
    }
    (f.sysres, f.program) = npdm(&exefs.join("main.npdm"))?;
    f.sdk = f
        .sdkver
        .iter()
        .filter_map(|v| v.split('_').next().and_then(|m| m.parse::<u64>().ok()))
        .min()
        .unwrap_or(0);
    Ok(f)
}

/// The facts of main alone that `build_sources` reads (its imports, GLSL sources and `glShaderSource` names), for a
/// build whose profile is registered (no survey).
pub fn main_facts(exefs: &Path) -> Result<Facts, String> {
    let p = exefs.join("main");
    let (img, bid, sizes, bss) = read_nso(&p)?;
    let syms = nf::dynsyms(&img).map_err(|e| format!("{}: {e}", p.display()))?;
    let mut f = Facts {
        glslc: nf::count(&img, &GLSLC_MAGIC),
        glsl_src: nf::glsl_sources(&img),
        gl_names: nf::count(&img, b"glShaderSource"),
        gl_src: nf::gl_src_count(&img),
        ..Default::default()
    };
    f.mods.push((
        "main".into(),
        ModFacts {
            bid,
            sizes,
            bss,
            far: false,
            imports: syms.iter().filter(|s| !s.1).map(|s| s.0.clone()).collect(),
            exports: BTreeSet::new(),
            comps: BTreeSet::new(),
            kind: "the game".into(),
        },
    ));
    Ok(f)
}

/// Every `NSO0` file including `rtld`.
fn nso_files_all(exefs: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(exefs)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && std::fs::read(p).is_ok_and(|b| b.starts_with(b"NSO0")))
        .collect();
    v.sort();
    v
}

/// The RomFS part of `facts_of`.
pub fn romfs_facts(romfs: &Path) -> Result<RomFacts, String> {
    let mut r = RomFacts::default();
    for (path, size, h) in romfs_files(romfs)? {
        r.n += 1;
        r.bytes += size;
        let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
        let top = if parts.len() > 2 {
            parts[..2].join("/")
        } else if parts.len() > 1 {
            parts[0].to_string()
        } else {
            "(root)".to_string()
        };
        r.tops.add(&top, 1);
        r.topb.add(&top, size);
        let ext = splitext(&path).to_lowercase();
        r.exts.add(if ext.is_empty() { "(none)" } else { &ext }, 1);
        r.extb.add(&ext, size);
        if parts.len() > 1 {
            r.dirs.add(parts[0], 1);
        } else {
            r.root.insert(parts[0].to_string());
        }
        let key = (size, path.trim_matches('/').to_string());
        if key > r.biggest {
            r.biggest = key;
        }
        if let Some((_, name)) = MAGICS.iter().find(|(m, _)| h.starts_with(m)) {
            r.mags.add(name, 1);
            r.magb.add(name, size);
            if PACKED.contains(name) {
                r.extc.add(&ext, 1);
            }
        }
    }
    Ok(r)
}
