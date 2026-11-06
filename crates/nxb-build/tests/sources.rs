//! The source checks of `nxb build` against the counts of the reference script `profile_guess.build_sources`.
//! `tools/gen_synth_sources.py` made the RomFS folder and `expected.txt` in `tests/data/sources`.

#![allow(clippy::unwrap_used)]

use nxb_build::host::build_sources::check_rom;
use nxb_build::host::rom::Rom;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/sources")
}

fn expected(case: &str) -> BTreeMap<String, u64> {
    let text = std::fs::read_to_string(dir().join("expected.txt")).unwrap_or_default();
    let mut m = BTreeMap::new();
    for line in text.lines() {
        let mut w = line.split(' ');
        if let (Some(c), Some(k), Some(v)) = (w.next(), w.next(), w.next())
            && c == case
        {
            m.insert(k.to_string(), v.parse().unwrap_or(u64::MAX));
        }
    }
    m
}

fn counts(case: &str, vulkan: bool, egl: bool) -> BTreeMap<String, u64> {
    let rom = Rom::open(&dir().join("romfs")).unwrap();
    let c = check_rom(&rom, vulkan, egl).unwrap();
    let mut m = BTreeMap::new();
    m.insert("spirv".to_string(), c.spirv);
    m.insert("unreal_paks".to_string(), c.unreal_paks);
    m.insert("trinity".to_string(), c.trinity);
    m.insert("legacy_paks".to_string(), c.legacy_paks);
    m.insert("gl_files".to_string(), c.gl_files);
    m.insert("oodle_pkz".to_string(), c.oodle_pkz);
    m.insert("oodle_gfpak".to_string(), c.oodle_gfpak);
    m.insert("oodle_ue3".to_string(), c.oodle_ue3);
    m.insert("frostbite".to_string(), u64::from(c.frostbite));
    // The script reads the GLSL text files only when main imports glslcCompile.
    let glsl = if case == "vk_egl" {
        c.glsl_text_files
    } else {
        0
    };
    m.insert("romfs_glsl".to_string(), glsl);
    m
}

#[test]
fn a_game_that_imports_vulkan_and_egl_matches_the_script() {
    let want = expected("vk_egl");
    assert!(want.len() >= 10, "the fixture is missing");
    assert_eq!(counts("vk_egl", true, true), want);
}

#[test]
fn a_game_that_imports_neither_matches_the_script() {
    let want = expected("plain");
    assert!(want.len() >= 10, "the fixture is missing");
    assert_eq!(counts("plain", false, false), want);
}
