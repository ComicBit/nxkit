//! The guess against `profile_guess.py`: every extraction folder under `NXB_ORACLE_EXTRACTS` (each with `ExeFS`,
//! `romfs_manifest.tsv`, optionally `romfs_nro`) has a report from the Python driver in `NXB_ORACLE_GUESS_PY`
//! (`<name>.txt`), the Rust report must be the same text. `NXB_ORACLE_ENGINE` is the engine folder (host hooks).

use super::guess_dump::report;
use super::nsofacts::host_hooks;
use std::path::PathBuf;

#[test]
fn guess_equals_the_reference_script() {
    let (Some(ex), Some(py), Some(engine)) = (
        std::env::var_os("NXB_ORACLE_EXTRACTS"),
        std::env::var_os("NXB_ORACLE_GUESS_PY"),
        std::env::var_os("NXB_ORACLE_ENGINE"),
    ) else {
        return;
    };
    let hooks = host_hooks(&PathBuf::from(engine).join("dm"), false);
    assert!(hooks.len() > 100, "{} hooks", hooks.len());
    let (mut same, mut bad) = (0, Vec::new());
    for d in std::fs::read_dir(ex).unwrap().flatten() {
        let name = d.file_name().to_string_lossy().into_owned();
        let want = PathBuf::from(&py).join(format!("{name}.txt"));
        let Ok(want) = std::fs::read_to_string(&want) else {
            continue;
        };
        let got = report(&d.path(), &hooks).unwrap();
        if got == want {
            same += 1;
        } else {
            let (g, w): (Vec<_>, Vec<_>) = (got.lines().collect(), want.lines().collect());
            let first = g.iter().zip(&w).find(|(a, b)| a != b);
            bad.push(format!(
                "{name}: {first:?} ({} vs {} lines)",
                g.len(),
                w.len()
            ));
        }
    }
    eprintln!("{same} reports identical");
    assert!(bad.is_empty() && same > 0, "{bad:#?}");
}

fn unesc(s: &str) -> String {
    let mut o = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => o.push('\n'),
                Some('t') => o.push('\t'),
                Some(x) => o.push(x),
                None => {}
            }
        } else {
            o.push(c);
        }
    }
    o
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

fn set(s: &str) -> std::collections::BTreeSet<String> {
    s.split(',')
        .filter(|x| !x.is_empty())
        .map(str::to_string)
        .collect()
}

fn pairs(v: &[(String, (String, String))]) -> String {
    v.iter()
        .map(|(k, (a, b))| format!("{k}={a}|{b}"))
        .collect::<Vec<_>>()
        .join(";")
}

/// The pure parts of the guess (heap and depth signals, region findings, shader sources) against the script on cases
/// the Python driver wrote (`NXB_ORACLE_SYN` holds `cases.txt` and `expected.txt`).
#[test]
fn signals_regions_and_sources_equal_the_reference_script() {
    use super::guess_check::*;
    use super::guess_src::{SourceInputs, shader_sources};
    let Some(dir) = std::env::var_os("NXB_ORACLE_SYN") else {
        return;
    };
    let dir = PathBuf::from(dir);
    let cases = std::fs::read_to_string(dir.join("cases.txt")).unwrap();
    let want = std::fs::read_to_string(dir.join("expected.txt")).unwrap();
    let syms: Vec<(String, (u64, u64))> = (0..3u64)
        .map(|i| {
            (
                format!("_ZN2nn3mem17StandardAllocator{i}InitializeEPvmb"),
                (0x1000 * (i + 1), 0x100),
            )
        })
        .chain([
            (
                "_ZN2nn2os19AllocateAddressRegionEPmm".to_string(),
                (0x9000, 0x40),
            ),
            ("plain_sym".to_string(), (0xa000, 0)),
        ])
        .collect();
    let (mut n, mut bad) = (0, Vec::new());
    for (c, w) in cases.lines().zip(want.lines()) {
        let f: Vec<&str> = c.split('\t').collect();
        let got = match f[0] {
            "heap" => {
                let g: Keys = f[1]
                    .split(';')
                    .filter_map(|kv| kv.split_once('='))
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();
                let log = unesc(f[2]);
                format!(
                    "{}||{}",
                    pairs(&heap_signals(&log, &g).unwrap()),
                    pairs(&depth_signal(&log, &g))
                )
            }
            "demangle" => demangle(f[1]),
            "region" => {
                let (alias, cap, secs): (i128, i128, u32) = (
                    f[1].parse().unwrap(),
                    f[2].parse().unwrap(),
                    f[3].parse().unwrap(),
                );
                let table = unesc(f[4]);
                let out = region_findings(&table, alias, &syms, secs, cap).unwrap();
                let rows = region_rows(&table, &syms).unwrap();
                let e = out
                    .iter()
                    .map(|(r, t)| format!("{}:{t}", u8::from(*r)))
                    .collect::<Vec<_>>()
                    .join("\n");
                esc(&format!(
                    "{e}||{}",
                    stdalloc_cap_needed(&rows, alias).map_or("None".to_string(), |v| v.to_string())
                ))
            }
            "src" => {
                let u = |i: usize| f[i].parse::<u64>().unwrap();
                let i = SourceInputs {
                    imports: set(f[1]),
                    glsl_src: u(2) as usize,
                    glslc_mods: f[3]
                        .split(',')
                        .filter(|x| !x.is_empty())
                        .map(str::to_string)
                        .collect(),
                    spirv: u(4),
                    unreal_paks: u(5),
                    trinity_packs: u(6),
                    legacy_paks: u(7),
                    gl_names: u(8) as usize,
                    gl_src: u(9) as usize,
                    gl_files: u(10),
                    oodle_pkz: u(11),
                    nro_imports: set(f[12]),
                    oodle_gfpak: u(13),
                    oodle_ue3: u(14),
                    frostbite: f[15] == "1",
                    romfs_glsl: u(16),
                };
                let s = shader_sources(&i);
                let b =
                    |x: &(bool, String)| format!("{}|{}", if x.0 { "True" } else { "False" }, x.1);
                esc(&format!(
                    "graphics={}|{};vk_capture={};gl_capture={};glslc_capture={};ue_decode={};oodle_packs={};ue_legacy={}",
                    s.graphics.0,
                    s.graphics.1,
                    b(&s.vk_capture),
                    b(&s.gl_capture),
                    b(&s.glslc_capture),
                    b(&s.ue_decode),
                    b(&s.oodle_packs),
                    b(&s.ue_legacy)
                ))
            }
            other => panic!("{other}"),
        };
        n += 1;
        if got != w && bad.len() < 3 {
            bad.push(format!("{}: got {got:?} want {w:?}", f[0]));
        }
    }
    eprintln!("{n} cases");
    assert!(bad.is_empty(), "{bad:#?}");
}

/// `mem_used` against the script on fixups tables and patched modules the Python driver wrote (`NXB_ORACLE_MU`).
#[test]
fn mem_used_equals_the_reference_script() {
    let Some(dir) = std::env::var_os("NXB_ORACLE_MU") else {
        return;
    };
    let dir = PathBuf::from(dir);
    let text = std::fs::read_to_string(dir.join("expected.txt")).unwrap();
    let mut n = 0;
    for l in text.lines() {
        let f: Vec<&str> = l.split('\t').collect();
        let mods: Vec<String> = f[1].split(',').map(str::to_string).collect();
        let got = super::guess::mem_used(&dir.join(f[0]), &mods).unwrap();
        assert_eq!(
            got.map_or("None".to_string(), |v| v.to_string()),
            f[2],
            "case {}",
            f[0]
        );
        n += 1;
    }
    assert_eq!(n, 200);
}

/// `profile_guess.py --check` lines: the guess of each extraction against its registered profile
/// (`NXB_ORACLE_CHECK` holds `<extraction name>.txt` with the script's line, `NXB_ORACLE_EXTRACTS` the extractions).
#[test]
fn check_lines_equal_the_reference_script() {
    use super::guess::{facts_of, guess};
    use super::guess_dump::check_line;
    let (Some(ex), Some(chk), Some(engine)) = (
        std::env::var_os("NXB_ORACLE_EXTRACTS"),
        std::env::var_os("NXB_ORACLE_CHECK"),
        std::env::var_os("NXB_ORACLE_ENGINE"),
    ) else {
        return;
    };
    let engine = PathBuf::from(engine);
    let hooks = host_hooks(&engine.join("dm"), false);
    let mut n = 0;
    for d in std::fs::read_dir(&chk).unwrap().flatten() {
        let name = d.path().file_stem().unwrap().to_string_lossy().into_owned();
        let want = std::fs::read_to_string(d.path()).unwrap();
        let id = want.split(' ').next().unwrap();
        let g = nxb_profile::tsv::TsvFile::load(&engine.join("dm/games").join(id).join("game.tsv"))
            .unwrap();
        let ex = PathBuf::from(&ex).join(&name);
        let f = facts_of(&ex.join("ExeFS"), &ex.join("romfs_manifest.tsv")).unwrap();
        let (line, _) = check_line(id, &g, &f, &guess(&f, &hooks).unwrap());
        assert_eq!(format!("{line}\n"), want, "{name}");
        n += 1;
    }
    eprintln!("{n} check lines identical");
    assert!(n > 0);
}

fn strs(s: std::collections::BTreeSet<String>) -> Vec<String> {
    s.into_iter().collect()
}

/// The scans of `facts_of` on a buffer whose results were taken from the script's regular expressions.
#[test]
fn byte_scans_follow_the_regular_expressions() {
    use super::nsofacts as nf;
    let img: &[u8] = b"xx NintendoSDK_nnSdk-Generic_22_2_0 yy nintendosdk_NNSDK-16_2_12 NintendoSDK_nnSdk-abc_x NintendoSDK_nnSdk-_5 NintendoSDK_OpenGL-4_5_0 NintendoSDK_nnSdk-9 NintendoSDK_Vulkan-1_3 NintendoSDK_-1 SDK MW+Nintendo+Foo_bar-12_3 SDK MW+Nintendo+X- 2020.3.7f1 and 2019.4.31f1 2030.1.1f1 2022.3.5f \0#version 450\nvoid  main (){}\0#version 330\nint avoid main(){}\0\0#version 1 void\tmain\n(\x00X#version 300\0\0#version x";
    assert_eq!(
        strs(nf::sdk_versions(img)),
        ["16_2_12", "22_2_0", "9", "_5"]
    );
    assert_eq!(
        strs(nf::sdk_components(img)),
        ["NintendoSDK_OpenGL", "NintendoSDK_Vulkan"]
    );
    assert_eq!(strs(nf::mw_components(img)), ["Foo_bar-12_3"]);
    assert_eq!(strs(nf::unity_versions(img)), ["2019.4.31f1", "2020.3.7f1"]);
    assert_eq!(nf::glsl_sources(img), 2);
    assert_eq!(nf::gl_src_count(img), 3);
    assert_eq!(nf::count(img, b"\0\0"), 2);
    assert_eq!(nf::count(img, b"#version "), 5);
}

#[test]
fn small_helpers() {
    use super::guess::{load_order, splitext};
    assert_eq!(splitext("/a/b.c/d"), "");
    assert_eq!(splitext("/a/.hidden"), "");
    assert_eq!(splitext("/a/x.tar.GZ"), ".GZ");
    assert_eq!(splitext("/a/..x"), "");
    assert!(load_order("main") < load_order("subsdk0"));
    assert!(load_order("subsdk9") < load_order("subsdk10"));
    assert!(load_order("subsdk10") < load_order("sdk"));
}
