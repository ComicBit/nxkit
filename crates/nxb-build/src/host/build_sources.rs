//! `profile_guess.py` `build_sources`: the inputs of `shader_sources` from the files of an extraction, as the NSP
//! script reads them at build time. The survey only has the manifest (the first 32 bytes of every RomFS file), so
//! it guesses these counts from markers. Here the files are opened: pak indexes, archive headers, NROs.

use super::guess::{Facts, load_order, read_nso};
use super::guess_src::SourceInputs;
use super::nsofacts::{self as nf, glsl_text};
use super::rom::{Rom, RomFile};
use super::ue_paks::count_paks;
use std::collections::BTreeSet;
use std::path::Path;

const UE3_TAG: [u8; 4] = [0xc1, 0x83, 0x2a, 0x9e];
const FROSTBITE_TOC_MAGIC: [u8; 4] = [0x00, 0xd1, 0xce, 0x01];

fn le(b: &[u8], o: usize, n: usize) -> Option<u64> {
    let s = b.get(o..o.checked_add(n)?)?;
    Some(s.iter().rev().fold(0u64, |a, &x| a << 8 | u64::from(x)))
}

/// The names an NRO file imports (nothing for a file that is not an NRO).
fn nro_imports(rom: &Rom, f: &RomFile) -> Result<BTreeSet<String>, String> {
    let b = rom.read(f, 0, f.size)?;
    if b.get(0x10..0x14) != Some(b"NRO0") {
        return Ok(BTreeSet::new());
    }
    Ok(nf::dynsyms(&b)?
        .into_iter()
        .filter(|s| !s.1)
        .map(|s| s.0)
        .collect())
}

/// A PlatinumGames PKZ archive whose name table names Oodle Kraken.
fn pkz_oodle(rom: &Rom, f: &RomFile) -> Result<bool, String> {
    let h = rom.head(f, 0x20)?;
    if h.get(..4) != Some(b"pkzl") {
        return Ok(false);
    }
    let (Some(cnt), Some(eo), Some(nl)) = (le(&h, 0x10, 4), le(&h, 0x14, 4), le(&h, 0x18, 4))
    else {
        return Ok(false);
    };
    let t = rom.read(f, eo + 0x20 * cnt, nl)?;
    Ok(t.windows(12).any(|w| w == b"OodleKraken\0"))
}

/// A Game Freak GFPAK archive with a file table entry of compression 3 (Oodle Kraken).
fn gfpak_oodle(rom: &Rom, f: &RomFile) -> Result<bool, String> {
    let h = rom.head(f, 0x20)?;
    if h.get(..8) != Some(b"GFLXPACK") {
        return Ok(false);
    }
    let (Some(cnt), Some(tab)) = (le(&h, 0x10, 4), le(&h, 0x18, 8)) else {
        return Ok(false);
    };
    let t = rom.read(f, tab, 0x18 * cnt)?;
    Ok((0..t.len() / 0x18).any(|i| le(&t, 0x18 * i + 2, 2) == Some(3)))
}

/// An Unreal Engine 3 package whose first compressed block, found in the first 4 KB, is an Oodle one (`8C`).
fn ue3_oodle(h: &[u8]) -> bool {
    if h.get(..4) != Some(&UE3_TAG[..]) || !le(h, 4, 2).is_some_and(|v| 0 < v && v < 0x8000) {
        return false;
    }
    let Some(i) = h[4..].windows(4).position(|w| w == UE3_TAG).map(|p| p + 4) else {
        return false;
    };
    let (Some(size), Some(raw)) = (le(h, i + 12, 4), le(h, i + 4, 4)) else {
        return false;
    };
    if i + 16 > h.len() {
        return false;
    }
    let per = raw.max(1);
    let nb = size.div_ceil(per);
    usize::try_from(8 * nb)
        .ok()
        .and_then(|k| h.get(i + 16 + k))
        .is_some_and(|&b| b == 0x8c)
}

/// What the RomFS files show.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RomChecks {
    /// SPIR-V files (only counted for a game that imports Vulkan functions).
    pub spirv: u64,
    /// Trinity pack files.
    pub trinity: u64,
    /// GLSL-named files (only counted for a game that imports EGL).
    pub gl_files: u64,
    /// PKZ archives with Oodle Kraken.
    pub oodle_pkz: u64,
    /// GFPAK archives with Oodle Kraken.
    pub oodle_gfpak: u64,
    /// Unreal Engine 3 packages with Oodle blocks.
    pub oodle_ue3: u64,
    /// Whether the RomFS is Frostbite's.
    pub frostbite: bool,
    /// `.vert` / `.frag` files that hold GLSL as text.
    pub glsl_text_files: u64,
    /// Unreal paks with shader code archives.
    pub unreal_paks: u64,
    /// Unreal paks with the shaders inline.
    pub legacy_paks: u64,
}

/// Looks into every file of the RomFS. `vulkan` and `egl`: main imports Vulkan functions or `eglCreateWindowSurface`.
pub fn check_rom(rom: &Rom, vulkan: bool, egl: bool) -> Result<RomChecks, String> {
    let mut c = RomChecks::default();
    for f in rom.files() {
        let lower = f.path.to_lowercase();
        let head = rom.head(f, 0x1000)?;
        if vulkan && head.starts_with(&[0x03, 0x02, 0x23, 0x07]) {
            c.spirv += 1;
        }
        if f.path.ends_with(".trpfs") && head.starts_with(b"ONEPACK\0") {
            c.trinity += 1;
        }
        if egl
            && [".mvgl", ".vert", ".frag"]
                .iter()
                .any(|e| lower.ends_with(e))
        {
            c.gl_files += 1;
        }
        if lower.ends_with(".vert") || lower.ends_with(".frag") {
            c.glsl_text_files += u64::from(glsl_text(&head[..head.len().min(4096)]));
        }
        if f.path.ends_with(".pkz") {
            c.oodle_pkz += u64::from(pkz_oodle(rom, f)?);
        }
        if f.path.ends_with(".gfpak") {
            c.oodle_gfpak += u64::from(gfpak_oodle(rom, f)?);
        }
        c.oodle_ue3 += u64::from(ue3_oodle(&head));
        if f.path == "/Data/layout.toc" && head.starts_with(&FROSTBITE_TOC_MAGIC) {
            c.frostbite = true;
        }
    }
    let paks = count_paks(rom);
    (c.unreal_paks, c.legacy_paks) = (paks.archives, paks.inline);
    Ok(c)
}

/// `build_sources(exefs, romfs, modules)`'s inputs of `shader_sources`. `modules` are the profile's modules that the
/// ExeFS holds, in load order.
pub fn build_inputs(
    f: &Facts,
    exefs: &Path,
    rom: &Rom,
    modules: &[String],
) -> Result<SourceInputs, String> {
    let main = f.module("main").ok_or("no main module")?;
    let imports = main.imports.clone();
    let vulkan = imports
        .iter()
        .any(|s| s.len() > 2 && s.starts_with("vk") && s.as_bytes()[2].is_ascii_uppercase());
    let c = check_rom(rom, vulkan, imports.contains("eglCreateWindowSurface"))?;
    let mut nro = BTreeSet::new();
    for n in rom.files().iter().filter(|n| n.path.ends_with(".nro")) {
        nro.extend(nro_imports(rom, n)?);
    }
    let glslc_import = imports.contains("glslcCompile");
    let romfs_glsl = if glslc_import && f.glsl_src == 0 {
        c.glsl_text_files
    } else {
        0
    };
    let defines = |m: &str| -> Result<bool, String> {
        let p = exefs.join(m);
        if !p.exists() {
            return Ok(false);
        }
        let (img, ..) = read_nso(&p)?;
        Ok(nf::dynsyms(&img)?
            .iter()
            .any(|s| s.0 == "glslcCompile" && s.1))
    };
    let mut glslc_mods = Vec::new();
    for m in modules {
        if m != "main" && (f.glsl_src != 0 || romfs_glsl != 0) && glslc_import && defines(m)? {
            glslc_mods.push(m.clone());
        }
    }
    if nro.contains("glslcCompile") {
        let mut all: BTreeSet<String> = glslc_mods.iter().cloned().collect();
        let mut names: Vec<String> = std::fs::read_dir(exefs)
            .map_err(|e| format!("{}: {e}", exefs.display()))?
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("subsdk"))
            .collect();
        names.sort();
        for n in names {
            if defines(&n)? {
                all.insert(n);
            }
        }
        glslc_mods = all.into_iter().collect();
        glslc_mods.sort_by_key(|m| load_order(m));
    }
    Ok(SourceInputs {
        imports,
        glsl_src: f.glsl_src,
        glslc_mods,
        spirv: c.spirv,
        unreal_paks: c.unreal_paks,
        trinity_packs: c.trinity,
        legacy_paks: c.legacy_paks,
        gl_names: f.gl_names,
        gl_src: f.gl_src,
        gl_files: c.gl_files,
        oodle_pkz: c.oodle_pkz,
        nro_imports: nro,
        oodle_gfpak: c.oodle_gfpak,
        oodle_ue3: c.oodle_ue3,
        frostbite: c.frostbite,
        romfs_glsl,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rom(files: Vec<(&str, Vec<u8>)>) -> Rom {
        Rom::in_memory(files)
    }

    #[test]
    fn the_checks_count_each_archive_family() {
        let mut pkz = b"pkzl".to_vec();
        pkz.resize(0x10, 0);
        for v in [1u32, 0x40, 12] {
            pkz.extend(v.to_le_bytes());
        }
        pkz.resize(0x40 + 0x20, 0);
        pkz.extend(b"OodleKraken\0");
        let mut gf = b"GFLXPACK".to_vec();
        gf.resize(0x10, 0);
        gf.extend(1u32.to_le_bytes());
        gf.extend(0u32.to_le_bytes());
        gf.extend(0x20u64.to_le_bytes());
        gf.extend([0, 0, 3, 0]);
        gf.resize(0x20 + 0x18, 0);
        let mut ue3 = UE3_TAG.to_vec();
        ue3.extend(500u16.to_le_bytes());
        ue3.resize(0x20, 0);
        ue3.extend(UE3_TAG);
        ue3.extend(0x20000u32.to_le_bytes());
        ue3.extend(0u32.to_le_bytes());
        ue3.extend(0x20000u32.to_le_bytes());
        ue3.extend([0u8; 8]);
        ue3.push(0x8c);
        let r = rom(vec![
            ("/Data/layout.toc", FROSTBITE_TOC_MAGIC.to_vec()),
            ("/a.pkz", pkz),
            ("/b.gfpak", gf),
            ("/c.bin", ue3),
            ("/d.trpfs", b"ONEPACK\0xx".to_vec()),
            ("/e.spv", vec![3, 2, 0x23, 7, 0]),
            ("/f.frag", b"void main() {}".to_vec()),
            ("/g.vert", vec![0, 1, 2]),
        ]);
        let c = check_rom(&r, true, true).unwrap();
        assert_eq!(
            c,
            RomChecks {
                spirv: 1,
                trinity: 1,
                gl_files: 2,
                oodle_pkz: 1,
                oodle_gfpak: 1,
                oodle_ue3: 1,
                frostbite: true,
                glsl_text_files: 1,
                ..RomChecks::default()
            }
        );
        let off = check_rom(&r, false, false).unwrap();
        assert_eq!((off.spirv, off.gl_files), (0, 0));
    }

    #[test]
    fn a_layout_toc_with_another_magic_is_not_frostbite() {
        let r = rom(vec![("/Data/layout.toc", vec![1, 2, 3, 4])]);
        assert!(!check_rom(&r, false, false).unwrap().frostbite);
    }

    #[test]
    fn a_pkz_without_the_oodle_name_is_not_counted() {
        let mut pkz = b"pkzl".to_vec();
        pkz.resize(0x10, 0);
        for v in [1u32, 0x40, 4] {
            pkz.extend(v.to_le_bytes());
        }
        pkz.resize(0x40 + 0x20, 0);
        pkz.extend(b"Zstd");
        let r = rom(vec![("/a.pkz", pkz)]);
        assert_eq!(check_rom(&r, false, false).unwrap().oodle_pkz, 0);
    }
}
