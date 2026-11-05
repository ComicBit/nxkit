//! The end of `build_ipa.sh`'s generation: fonts, icon, entitlements, `xcodegen`, the Info.plist lines and the
//! resource paths (`gen_resources`), then the `--prepare-xcode` summary.

use super::ipa::{Inputs, append, copy, or};
use super::resources;
use super::script::{self, ASSETS, ENTITLEMENTS, STD_FONT, die, mkdir_p};
use crate::host::Runner;
use crate::host::profile::Game;
use std::path::Path;

/// What [`finish`] needs from the generation.
pub(crate) struct Finish<'a> {
    pub r: &'a Runner,
    pub prepare: bool,
    pub i: &'a Inputs,
    pub ios: &'a Path,
    pub gen_dir: &'a Path,
    pub cfg: &'a Path,
    pub rd: &'a str,
    pub msldir: &'a str,
    pub version: &'a str,
    pub pkg: &'a str,
    pub scheme: &'a str,
    /// RomFS, marker, shader folder, its name in the app, AOC, Xcode's derived data, pipeline archive.
    pub paths: [&'a str; 7],
    /// The programs of a game of several (empty: one program) and the chain version.
    pub programs: &'a [super::ipaprog::Prog],
    pub chain: &'a str,
    /// Route C.
    pub fw: Option<super::ipafw::FwIn<'a>>,
}

/// The rest of the generation, in the script's order.
pub(crate) fn finish(f: &Finish<'_>, say: &mut dyn FnMut(&str)) -> Result<(), String> {
    let (r, i) = (f.r, f.i);
    fonts_icon(r, i, f.gen_dir, f.cfg, f.rd, say)?;
    if let Some(fw) = &f.fw {
        super::ipafw::gen_framework(i, f.prepare, f.gen_dir, fw, say)?;
    }
    if f.prepare && i.entitlements.as_deref() != Some("") {
        let ent = f.gen_dir.join("DmIOS.entitlements");
        entitlements(i, &ent)?;
        append(f.cfg, "CODE_SIGN_ENTITLEMENTS = Gen/DmIOS.entitlements\n")?;
        let json = r
            .stdout(
                "plutil",
                [
                    "-convert".as_ref(),
                    "json".as_ref(),
                    "-o".as_ref(),
                    "-".as_ref(),
                    ent.as_os_str(),
                ],
                None,
            )
            .unwrap_or_default();
        say(&format!("entitlements: {}", json.trim_end_matches('\n')));
    }
    if f.prepare && i.copy_romfs.as_deref() == Some("NO") {
        append(f.cfg, "DM_COPY_ROMFS = NO\n")?;
    }
    let spec = (!f.prepare && f.fw.is_some()).then_some("project-framework.yml");
    project(r, i, f.ios, f.msldir, spec, say)?;
    resource_paths(i, f.ios, f.paths, f.version, f.pkg, (f.programs, f.chain))?;
    if f.prepare {
        say(&format!(
            "Xcode project: {}/DmIOS.xcodeproj",
            f.ios.display()
        ));
        say(&format!("Xcode derived data: {}", f.paths[5]));
        say(&format!("scheme: {}", f.scheme));
        say(
            "ready: select your Team in Signing & Capabilities, select a physical device, and Run (Release)",
        );
    }
    Ok(())
}

/// `game.py modules <game> <patched>`: the modules of main's build row, else all the profile's.
pub fn modules(g: &Game, patched: &str) -> Result<Vec<String>, String> {
    let fx = std::fs::read(format!("{patched}/main.fixups.tsv")).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&fx);
    let bid = text.split('\n').find_map(|l| {
        let r = l.strip_prefix("# BUILD_ID")?;
        let r2 = r.trim_start_matches(|c: char| c.is_whitespace());
        if r2.len() == r.len() {
            return None;
        }
        let w: String = r2
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        (!w.is_empty()).then_some(w)
    });
    let rows = g.build_modules()?;
    Ok(
        match rows
            .iter()
            .find(|(b, _)| Some(b.as_str()) == bid.as_deref())
        {
            Some((_, m)) => m.clone(),
            None => g.words("modules")?,
        },
    )
}

/// The checks of the program's inputs (`check_program`), returning the modules and the graphics API.
pub(crate) fn check_program(
    _r: &Runner,
    (gfxapi, game): (Option<&str>, &str),
    g: &Game,
    [patched, romfs, msl]: [&str; 3],
    marker: &str,
    v: &str,
) -> Result<(Vec<String>, String), String> {
    if !Path::new(&format!("{patched}/main.fixups.tsv")).is_file() {
        return Err(die(format!(
            "no rewritten main in {patched} (run dm/tools/patch_nso.py first)"
        )));
    }
    let gd = g.dir.display();
    let mods = modules(g, patched).map_err(|_| die(format!("game profile {gd}: modules")))?;
    for f in &mods {
        if !Path::new(&format!("{patched}/{f}.patched.bin")).is_file()
            || !Path::new(&format!("{patched}/{f}.fixups.tsv")).is_file()
        {
            return Err(die(format!(
                "no rewritten module {patched}/{f} (run dm/tools/patch_nso.py first)"
            )));
        }
    }
    super::romfs::check(Path::new(romfs), marker)
        .map_err(|_| die(format!("no RomFS at {romfs} (missing {marker})")))?;
    let b_libs = std::fs::read_dir(format!("{msl}/b-{v}"))
        .map(|d| {
            d.flatten()
                .any(|e| e.file_name().to_string_lossy().ends_with("_b.metallib"))
        })
        .unwrap_or(false);
    let vk = Path::new(&format!("{msl}/vk/vk_shaders.metallib")).is_file();
    let gl = Path::new(&format!("{msl}/gl/gl_shaders.metallib")).is_file();
    let gfx = match gfxapi.filter(|x| !x.is_empty()) {
        Some(x) => x.to_string(),
        None if b_libs => "nvn".into(),
        None if vk => "vulkan".into(),
        None if gl => "gl".into(),
        None => "nvn".into(),
    };
    if gfx == "vulkan" {
        if !vk {
            return Err(die(format!(
                "no iOS Vulkan shader library {msl}/vk (dm/gfx/tools/vk_shaders.py \"{romfs}\" <Mac dir> --ios \"{msl}\")"
            )));
        }
    } else if gfx == "gl" {
        if !gl {
            return Err(die(format!(
                "no iOS GL shader library {msl}/gl (dm/gfx/tools/gl_shaders.py \"{romfs}\" <main> <Mac dir> --ios \"{msl}\")"
            )));
        }
    } else {
        if v.is_empty() || !b_libs {
            return Err(die(format!(
                "no iOS shader libraries {msl}/b-{v} (IOSOUT={msl} dmsh_build.py ...)"
            )));
        }
        let inp = nxb_shader::chain::inputs(&g.dir)
            .map_err(|_| die(format!("dmsh_chain.py --inputs (game profile {gd})")))?;
        let stamp = std::fs::read_to_string(format!("{msl}/b-{v}/inputs.tsv")).ok();
        let has = |l: &str| {
            stamp
                .as_deref()
                .is_some_and(|s| s.split('\n').any(|x| x == l))
        };
        if !(has(&format!("game\t{game}")) && has(&format!("inputs\t{inp}"))) {
            let st = match &stamp {
                Some(s) => s
                    .split_inclusive('\n')
                    .filter(|l| !l.starts_with('#'))
                    .collect::<String>()
                    .replace(['\n', '\t'], " "),
                None => "none".into(),
            };
            return Err(die(format!(
                "the iOS shader libraries {msl}/b-{v} are not a full build from this profile's current tables (inputs {inp}; their stamp: {st}): build them again (IOSOUT={msl} dmsh_build.py ...)"
            )));
        }
        let c1 = std::fs::read_to_string(g.dir.join("c1_full.tsv")).unwrap_or_default();
        if !c1
            .split('\n')
            .next()
            .unwrap_or("")
            .starts_with(&format!("# chain b-{v} inputs {inp} "))
        {
            return Err(die(format!(
                "{gd}/c1_full.tsv is missing or not made from these libraries (chain {v}, inputs {inp}): a full dmsh_build.py run makes it and cb_extent.tsv"
            )));
        }
    }
    Ok((mods, gfx))
}

/// The pipeline archive's architectures (`GPUS`): `DM_PSO_GPUS`, else the profile's `pso_gpu` (1 to 4 names, joined by
/// commas) for `pso_mtl4 on`, else `none`, and the warning line for a bad `pso_gpu`.
pub(crate) fn pso_gpus(set: &Option<String>, p4: &str, pg: &str) -> (String, Option<String>) {
    let words: Vec<&str> = pg
        .split([' ', '\t', '\n'])
        .filter(|w| !w.is_empty())
        .collect();
    let mut n = 0;
    for w in &words {
        let x = w.strip_prefix("applegpu_").unwrap_or(w);
        if *w == "none" || x.is_empty() || !x.chars().all(|c| c.is_ascii_alphanumeric()) {
            n = 9;
            break;
        }
        n += 1;
    }
    let mut pgk = "g18p".to_string();
    let mut warn = None;
    if p4 == "on" && !pg.is_empty() {
        if (1..=4).contains(&n) {
            // tr -s ' \t' ',': runs of blanks become one comma
            let mut s = String::new();
            let mut last = false;
            for c in pg.chars() {
                let b = c == ' ' || c == '\t';
                if !(b && last) {
                    s.push(if b { ',' } else { c });
                }
                last = b;
            }
            pgk = s;
        } else {
            warn = Some(format!(
                "pso_gpu '{pg}' is not 1-4 whitespace-separated GPU architectures (each one such as g18p or applegpu_g19p, with or without the applegpu_ prefix; there is no none: without the key the archive defaults follow pso_mtl4) - taking g18p"
            ));
        }
    }
    let gpus = match set {
        Some(v) => v.clone(),
        None if p4 == "on" => pgk,
        None => "none".into(),
    };
    (gpus, warn)
}

/// Fonts, icon, entitlements, `xcodegen`, the Info.plist lines and the resource paths (the rest of the generation).
pub(crate) fn fonts_icon(
    r: &Runner,
    i: &Inputs,
    gen_dir: &Path,
    cfg: &Path,
    rd: &str,
    say: &mut dyn FnMut(&str),
) -> Result<(), String> {
    let dm = i.engine.join("dm");
    mkdir_p(&gen_dir.join("dm/font"))?;
    copy(
        Path::new(&or(&i.std_font, STD_FONT)),
        &gen_dir.join("dm/font/standard.ttf"),
        "copy standard font (STD_FONT=a TTF)",
    )?;
    let font_out = gen_dir.join("dm/font/nintendo_ext.ttf");
    if super::extfont::make(&font_out, None).is_err() {
        // the letter font is not the expected one: the engine's script decides
        r.run(
            &or(&i.font_py, "python3"),
            [
                dm.join("tools/make_ext_font.py").as_os_str(),
                gen_dir.join("dm/font/nintendo_ext.ttf").as_os_str(),
            ],
            gen_dir.parent(),
        )
        .map_err(|_| {
            die("make_ext_font.py (pip install fonttools, or FONT_PY=python with fontTools)")
        })?;
    }
    let icon = or(&i.icon, format!("{rd}/icon.jpg"));
    mkdir_p(&gen_dir.join("Assets.xcassets"))?;
    std::fs::write(gen_dir.join("Assets.xcassets/Contents.json"), ASSETS)
        .map_err(|_| die("asset catalog"))?;
    if Path::new(&icon).is_file() {
        let ai = gen_dir.join("Assets.xcassets/AppIcon.appiconset");
        mkdir_p(&ai)?;
        r.run(
            "sips",
            [
                "-s".as_ref(),
                "format".as_ref(),
                "png".as_ref(),
                "-z".as_ref(),
                "1024".as_ref(),
                "1024".as_ref(),
                icon.as_ref(),
                "--out".as_ref(),
                ai.join("icon.png").as_os_str(),
            ]
            .map(|s: &std::ffi::OsStr| s.to_os_string()),
            None,
        )
        .map_err(|_| die(format!("sips: cannot convert the icon {icon}")))?;
        std::fs::write(
            ai.join("Contents.json"),
            "{\"images\":[{\"filename\":\"icon.png\",\"idiom\":\"universal\",\"platform\":\"ios\",\"size\":\"1024x1024\"}],\"info\":{\"author\":\"xcode\",\"version\":1}}\n",
        )
        .map_err(|_| die("asset catalog"))?;
        append(cfg, "DM_APPICON = AppIcon\n")?;
        say(&format!("app icon: {icon}"));
    } else {
        say(&format!(
            "no app icon ({icon} not found): the home-screen icon stays blank"
        ));
    }
    Ok(())
}

/// The entitlements file: `DM_ENTITLEMENTS`'s file, else the Increased Memory Limit request.
pub(crate) fn entitlements(i: &Inputs, to: &Path) -> Result<(), String> {
    match i.entitlements.clone().filter(|x| !x.is_empty()) {
        Some(f) => copy(Path::new(&f), to, "copy DM_ENTITLEMENTS"),
        None => std::fs::write(to, ENTITLEMENTS).map_err(|_| die("entitlements")),
    }
}

/// `xcodegen`, then `DMCommit` / `DMChain` / `MetalCaptureEnabled` in `Gen/Info.plist`.
pub(crate) fn project(
    r: &Runner,
    i: &Inputs,
    ios: &Path,
    msldir: &str,
    spec: Option<&str>,
    say: &mut dyn FnMut(&str),
) -> Result<(), String> {
    script::xcodegen(r, ios, spec)?;
    let plist = ios.join("Gen/Info.plist");
    if let Some(c) = script::commit(r, &i.engine.join("dm")) {
        script::plist_add(r, &plist, "DMCommit", "string", &c)?;
        say(&format!("source: {c}"));
    }
    if let Some(ch) = msldir.strip_prefix("b-") {
        script::plist_add(r, &plist, "DMChain", "string", ch)?;
    }
    if i.metal_capture.as_deref() == Some("1") {
        script::plist_add(r, &plist, "MetalCaptureEnabled", "bool", "true")?;
        say("Metal frame capture: enabled (DM_METAL_CAPTURE=1)");
    }
    Ok(())
}

/// `gen_resources.py` with the script's arguments.
pub(crate) fn resource_paths(
    i: &Inputs,
    ios: &Path,
    p: [&str; 7],
    version: &str,
    pkg: &str,
    (progs, chain): (&[super::ipaprog::Prog], &str),
) -> Result<(), String> {
    let [romfs, marker, shaders, msldir, aoc, derived, pso] = p;
    // a game of several programs: each program's files, which the Xcode build phase copies under p<index>
    let programs = if progs.is_empty() {
        None
    } else {
        let mut t =
            String::from("# program\tRomFS\tmarker\tiOS shaders\tshader folder (build_ipa.sh)\n");
        for p in progs {
            let d = p.msldir(chain);
            t.push_str(&format!(
                "{}\t{}\t{}\t{}/{d}\t{d}\n",
                p.n, p.romfs, p.marker, p.msl
            ));
        }
        std::fs::write(ios.join("Gen/resources_programs.tsv"), t)
            .map_err(|_| die("resources_programs.tsv"))?;
        Some("Gen/resources_programs.tsv".to_string())
    };
    resources::write(&resources::Inputs {
        ios: ios.to_string_lossy().into_owned(),
        romfs: romfs.into(),
        marker: marker.into(),
        shaders: shaders.into(),
        msldir: msldir.into(),
        aoc: aoc.into(),
        derived: derived.into(),
        chain_h: i
            .engine
            .join("dm/gfx/nvn/dmsh_chain.h")
            .to_string_lossy()
            .into_owned(),
        pso: pso.into(),
        programs,
        title: i.game.clone(),
        version: version.into(),
        build: pkg.into(),
        user: i.user.clone(),
    })
    .map_err(|e| die(format!("generate Xcode resource paths ({e})")))
}
