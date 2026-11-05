//! Route C in `build_ipa.sh` (`DM_FRAMEWORK=1`, `DM_FRAMEWORK_ONLY=<folder>`): the game's modules in
//! `DmGame.framework`. `Gen/framework` (tables, `profile.tsv`, `game.manifest.json`, link flags), the framework
//! signature of the one-game app, and the shell's framework alone (`xcodebuild -target DmGame`, then the game's
//! shader libraries, pipeline archive and icon inside it).

use super::ipa::{Inputs, State, or};
use super::manifest;
use super::script::{self, die, mkdir_p, rm_rf};
use crate::host::Runner;
use crate::host::profile::Game;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// What the framework step reads from the generation.
pub(crate) struct FwIn<'a> {
    pub name: &'a str,
    pub only: bool,
    pub mods: &'a [String],
    pub flags: &'a str,
    pub display_name: &'a str,
    pub display_version: &'a str,
    pub pkg: &'a str,
    pub patched: &'a str,
    pub chain: &'a str,
    pub marker: &'a str,
    pub romfs: &'a str,
    pub o: &'a Path,
    pub g: &'a Game,
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    mkdir_p(to)?;
    for e in std::fs::read_dir(from)
        .map_err(|_| die("copy the framework tables"))?
        .flatten()
    {
        let (p, t) = (e.path(), to.join(e.file_name()));
        let m = std::fs::symlink_metadata(&p).map_err(|_| die("copy the framework tables"))?;
        if m.is_dir() {
            copy_tree(&p, &t)?;
        } else if m.file_type().is_symlink() {
            let l = std::fs::read_link(&p).map_err(|_| die("copy the framework tables"))?;
            let _ = std::fs::remove_file(&t);
            std::os::unix::fs::symlink(l, &t).map_err(|_| die("copy the framework tables"))?;
        } else {
            std::fs::copy(&p, &t).map_err(|_| die("copy the framework tables"))?;
        }
    }
    Ok(())
}

fn done<T>(c: Result<T, String>, what: &str) -> Result<(), String> {
    c.map(|_| ()).map_err(|_| die(what))
}

/// The `if [ "$FRAMEWORK" = 1 ]` block after the icon: `Gen/framework`.
pub(crate) fn gen_framework(
    i: &Inputs,
    prepare: bool,
    gen_dir: &Path,
    f: &FwIn<'_>,
    say: &mut dyn FnMut(&str),
) -> Result<(), String> {
    let fw = gen_dir.join("framework");
    rm_rf(&fw)?;
    mkdir_p(&fw.join("dm")).map_err(|_| die("mkdir Gen/framework"))?;
    copy_tree(&gen_dir.join("dm"), &fw.join("dm"))?;
    rm_rf(&fw.join("dm/font"))?;
    let _ = std::fs::remove_file(fw.join("dm/env.txt"));
    // the one-game app reads its launcher.json from the app's dm/, the shell reads each game's from its framework
    if !f.only {
        let _ = std::fs::remove_file(fw.join("dm/launcher.json"));
    }
    let here = i.engine.join("dm/games");
    crate::host::profile_tsv::write(f.g, &here, &fw.join("profile.tsv"))
        .map_err(|_| die("game profile"))?;
    let mut copy = i.copy_romfs.clone().unwrap_or_default();
    if copy.is_empty() && !prepare {
        copy = if i.full_romfs.as_deref().is_some_and(|x| !x.is_empty()) {
            "YES".into()
        } else {
            "NO".into()
        };
    }
    let mut id = String::new();
    if copy == "NO" && Path::new(f.romfs).is_file() {
        mkdir_p(f.o)?;
        id = super::romfs::identity(Path::new(f.romfs), &i.game, f.display_version, f.pkg)
            .map_err(|_| die(format!("identity of {}", f.romfs)))?;
    }
    let nro = Path::new(f.patched).join("nro.tsv");
    let opts = manifest::Options {
        game: i.game.clone(),
        name: f.display_name.into(),
        display_version: f.display_version.into(),
        package_version: f.pkg.into(),
        modules: f.mods.join(" "),
        nro_table: nro.is_file().then_some(nro),
        chain: format!("b-{}", f.chain),
        romfs_marker: f.marker.into(),
        romfs_identity: id,
        framework_name: f.name.into(),
        ..manifest::Options::default()
    };
    manifest::write(&fw.join("game.manifest.json"), &opts)
        .map_err(|_| die("game manifest (gen_manifest.py)"))?;
    std::fs::write(
        gen_dir.join("framework.xcconfig"),
        format!("OTHER_LDFLAGS = $(inherited) {}\n", f.flags),
    )
    .map_err(|_| die("framework link flags"))?;
    say(&format!(
        "framework: Gen/framework (modules {}, profile.tsv, game.manifest.json)",
        f.mods.join(" ")
    ));
    Ok(())
}

/// The framework of the one-game app, signed ad hoc without entitlements before the executable.
pub(crate) fn sign_framework(app: &Path, say: &mut dyn FnMut(&str)) -> Result<(), String> {
    let fw = app.join("Frameworks/DmGame.framework");
    if !fw.is_dir() {
        return Err(die(format!(
            "no {}: project-framework.yml embeds the game framework",
            fw.display()
        )));
    }
    let opts = nxb_pack::bundle::BundleOptions {
        entitlements: None,
        ..nxb_pack::bundle::BundleOptions::default()
    };
    nxb_pack::bundle::sign_bundle(&fw, &opts)
        .map_err(|e| die(format!("ad-hoc signature of the game framework ({e})")))?;
    say(&format!(
        "game framework signed ad-hoc, no entitlements: {}",
        fw.display()
    ));
    Ok(())
}

/// `DM_FRAMEWORK_ONLY`: the framework target alone, unsigned, into `<folder>/<name>.framework` with the shader
/// libraries, the pipeline archive and the icon of the game.
pub(crate) fn build_framework_only(
    st: &State,
    i: &Inputs,
    say: &mut dyn FnMut(&str),
) -> Result<PathBuf, String> {
    let r: &Runner = &st.runner;
    let folder = PathBuf::from(or(&i.framework_only, ""));
    let name = &st.fw_name;
    say(&format!(
        "xcodebuild (Release, unsigned, the game framework {name} only)"
    ));
    mkdir_p(&st.o)?;
    mkdir_p(&folder)?;
    let dd = st.dd.to_string_lossy().into_owned();
    let bundle_id = r
        .env()
        .get("DM_BUNDLE_ID")
        .map(|v| v.to_string_lossy().into_owned())
        .unwrap_or_default();
    let args: Vec<String> = [
        "-project",
        "DmIOS.xcodeproj",
        "-target",
        "DmGame",
        "-configuration",
        "Release",
        "-sdk",
        "iphoneos",
        "CODE_SIGNING_ALLOWED=NO",
        "CODE_SIGNING_REQUIRED=NO",
        "CODE_SIGN_IDENTITY=",
        &format!("PRODUCT_NAME={name}"),
        &format!("PRODUCT_BUNDLE_IDENTIFIER={bundle_id}.game"),
        &format!("DM_XCODE_OUTPUT_ROOT={dd}"),
        &format!("SYMROOT={dd}/Build/Products"),
        &format!("OBJROOT={dd}/Build/Intermediates.noindex"),
        "build",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if let Err((e, errors)) =
        script::xcodebuild(r, &st.ios, &args, &st.o.join("build-framework.log"))
    {
        errors.iter().for_each(|l| say(l));
        return Err(e);
    }
    let built = st
        .dd
        .join(format!("Build/Products/Release-iphoneos/{name}.framework"));
    if !built.join(name).is_file() {
        return Err(die(format!("no built framework {}", built.display())));
    }
    let out = folder.join(format!("{name}.framework"));
    mkdir_p(&out)?;
    // rsync -a --delete --exclude /msl --exclude /pso --exclude /icon.png -- BUILT/ OUT/
    done(
        r.run(
            "rsync",
            [
                OsString::from("-a"),
                OsString::from("--delete"),
                OsString::from("--exclude"),
                OsString::from("/msl"),
                OsString::from("--exclude"),
                OsString::from("/pso"),
                OsString::from("--exclude"),
                OsString::from("/icon.png"),
                OsString::from("--"),
                OsString::from(format!("{}/", built.display())),
                OsString::from(format!("{}/", out.display())),
            ],
            None,
        ),
        &format!("copy the framework to {}", out.display()),
    )?;
    let msl = out.join("msl").join(&st.msldir);
    mkdir_p(&msl)?;
    done(
        r.run(
            "rsync",
            [
                OsString::from("-a"),
                OsString::from("--delete"),
                OsString::from("--"),
                OsString::from(format!("{}/{}/", st.msl_ios, st.msldir)),
                OsString::from(format!("{}/", msl.display())),
            ],
            None,
        ),
        "copy the shader libraries",
    )?;
    if let Ok(rd) = std::fs::read_dir(out.join("msl")) {
        for e in rd.flatten() {
            if e.path() != msl {
                rm_rf(&e.path())?;
            }
        }
    }
    if !st.pso.is_empty() {
        let pso = out.join("pso");
        mkdir_p(&pso)?;
        done(
            r.run(
                "rsync",
                [
                    OsString::from("-a"),
                    OsString::from("--delete"),
                    OsString::from("--"),
                    OsString::from(format!("{}/", st.pso)),
                    OsString::from(format!("{}/", pso.display())),
                ],
                None,
            ),
            "copy the pipeline archive",
        )?;
    } else {
        rm_rf(&out.join("pso"))?;
    }
    if Path::new(&st.icon).is_file() {
        done(
            r.run(
                "sips",
                [
                    OsString::from("-s"),
                    OsString::from("format"),
                    OsString::from("png"),
                    OsString::from("-z"),
                    OsString::from("512"),
                    OsString::from("512"),
                    OsString::from(&st.icon),
                    OsString::from("--out"),
                    out.join("icon.png").into_os_string(),
                ],
                None,
            ),
            &format!("sips: cannot convert the icon {}", st.icon),
        )?;
    } else {
        let _ = std::fs::remove_file(out.join("icon.png"));
    }
    let m = std::fs::read_to_string(out.join("game.manifest.json")).unwrap_or_default();
    let named = nxb_pyjson::parse(&m).ok().and_then(|v| {
        v.get("framework_executable")
            .and_then(|x| x.as_str().map(str::to_string))
    });
    if named.as_deref() != Some(name.as_str()) {
        return Err(die(format!(
            "{}/game.manifest.json does not name {name}",
            out.display()
        )));
    }
    say(&format!(
        "done: {} ({}; build_shell.sh embeds it in the shell)",
        out.display(),
        script::du_sh(r, &out).replace('\t', "")
    ));
    Ok(out)
}
