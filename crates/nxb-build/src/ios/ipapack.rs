//! The end of `build_ipa.sh` in IPA mode: `xcodebuild` (Release, unsigned), the leftovers of an older pack removed, the
//! executable signed ad hoc with the entitlements (`codesign`, external), and the IPA packed straight from the source
//! folders with `nxb_pack::ipa` (byte-identical to `pack_ipa.py`, its `.idx` reuse of a previous IPA is not ported,
//! so every pack is a full one).

use super::ipa::{Inputs, State, or};
use super::ipagen::entitlements;
use super::script::{self, die, mkdir_p, rm_rf};
use std::collections::HashSet;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// `[ a -ef b ]`.
fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(x), Ok(y)) => x.dev() == y.dev() && x.ino() == y.ino(),
        _ => false,
    }
}

fn with_suffix(p: &Path, s: &str) -> PathBuf {
    let mut o = p.as_os_str().to_owned();
    o.push(s);
    PathBuf::from(o)
}

/// `$(ls dir | tr '\n' ' ')`.
fn ls_line(dir: &str) -> String {
    let mut n: Vec<String> = std::fs::read_dir(dir)
        .map(|d| {
            d.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| !n.starts_with('.'))
                .collect()
        })
        .unwrap_or_default();
    n.sort();
    n.iter().map(|x| format!("{x} ")).collect()
}

/// `DM_GAME_PSO_MTL4` of the generated profile header (`0` or `1`).
pub fn pull_default(engine: &Path, game: &str) -> Result<String, String> {
    let h = engine.join(format!("dm/host/gen/{game}/game_profile.h"));
    let text = std::fs::read_to_string(&h).map_err(|_| die("read game profile header"))?;
    let v: Vec<&str> = text
        .split('\n')
        .filter_map(|l| l.strip_prefix("#define DM_GAME_PSO_MTL4 "))
        .collect();
    match v.join("\n").as_str() {
        x @ ("0" | "1") => Ok(x.to_string()),
        _ => Err(die("no DM_GAME_PSO_MTL4 in generated game profile")),
    }
}

/// The archive's mappings, the skipped paths and the pack options.
pub type Plan = (
    Vec<(String, PathBuf)>,
    Vec<String>,
    nxb_pack::ipa::PackOptions<'static>,
);

/// The archive's mappings, skipped paths and options as the script builds them, with its step lines.
pub fn plan(st: &State, i: &Inputs, app: &Path, say: &mut dyn FnMut(&str)) -> Result<Plan, String> {
    const P: &str = "Payload/DmIOS.app";
    let skip: Vec<String> = [
        "_CodeSignature",
        "embedded.mobileprovision",
        "msl",
        "aoc",
        "romfs",
        "romfs.img",
        "romfs.id",
        "html",
        "jit",
    ]
    .iter()
    .map(|s| format!("{P}/{s}"))
    .collect();
    let romfs = &st.romfs;
    let rd = Path::new(romfs)
        .parent()
        .map(|d| d.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".".into());
    let mut maps: Vec<(String, PathBuf)> = Vec::new();
    let copy = match i.copy_romfs.clone().filter(|x| !x.is_empty()) {
        Some(c) => c,
        None if i.full_romfs.as_deref().is_some_and(|x| !x.is_empty()) => "YES".into(),
        None => "NO".into(),
    };
    let is_file = Path::new(romfs).is_file();
    if copy == "NO" {
        say(
            "no game files in the IPA (DM_COPY_ROMFS=NO): the app takes Documents/romfs.img (scripts/push_romfs.sh)",
        );
        if is_file {
            mkdir_p(&st.o)?;
            let py = i.engine.join("dm/tools/romfs_image.py");
            let id = st
                .runner
                .run(
                    "python3",
                    [
                        "-I",
                        &py.to_string_lossy(),
                        "--identity",
                        romfs,
                        &i.game,
                        &st.app_version,
                        &st.pkg,
                    ],
                    None,
                )
                .map_err(|_| die(format!("identity of {romfs}")))?
                .stdout;
            let f = st.o.join("romfs.id");
            std::fs::write(&f, &id).map_err(|_| die(format!("identity of {romfs}")))?;
            maps.push((format!("{P}/romfs.id"), f));
            let shown = String::from_utf8_lossy(&id);
            say(&format!(
                "game files identity: {}",
                shown.trim_end_matches('\n')
            ));
        }
    } else if is_file {
        maps.push((format!("{P}/romfs.img"), romfs.into()));
    } else {
        maps.push((format!("{P}/romfs"), romfs.into()));
    }
    let aoc = &st.aoc;
    let aoc_dir = !aoc.is_empty() && Path::new(aoc).is_dir();
    if copy != "NO" && aoc_dir {
        maps.push((format!("{P}/aoc"), aoc.into()));
        say(&format!(
            "add-on content packed into the IPA: {}",
            ls_line(aoc)
        ));
    } else if aoc_dir {
        say(&format!(
            "add-on content alongside the game data (DM_FULL_ROMFS=1 packs it): {}",
            ls_line(aoc)
        ));
    }
    if !st.pso.is_empty() {
        maps.push((format!("{P}/pso"), st.pso.clone().into()));
    }
    maps.push((
        format!("{P}/msl/{}", st.msldir),
        format!("{}/{}", st.msl_ios, st.msldir).into(),
    ));
    maps.push((P.into(), app.to_path_buf()));
    if Path::new(&format!("{rd}/html")).is_dir() {
        maps.push((format!("{P}/html"), format!("{rd}/html").into()));
        say(&format!("offline HTML pages: {rd}/html"));
    }
    if Path::new(&format!("{rd}/jit")).is_dir() {
        maps.push((format!("{P}/jit"), format!("{rd}/jit").into()));
        say(&format!("nn::jit image: {rd}/jit"));
    }
    say("pack IPA (already-compressed game files stored, everything else deflated)");
    let pull = pull_default(&i.engine, &i.game)?;
    let zs =
        st.g.get("zip_store")
            .map_err(|_| die(format!("game profile {}: zip_store", st.g.dir.display())))?;
    let mut skip_suffix = Vec::new();
    if st.pso.is_empty() && or(&i.ios_pull, pull) != "1" {
        skip_suffix.push("_bp.metallib".to_string());
    }
    let o = nxb_pack::ipa::PackOptions {
        store: zs
            .split(':')
            .filter(|s| !s.is_empty())
            .map(str::to_lowercase)
            .collect(),
        skip: skip
            .iter()
            .map(|s| s.trim_matches('/').to_string())
            .collect::<HashSet<_>>(),
        skip_suffix,
        compress: nxb_pack::ipa::ZLIB,
    };
    Ok((maps, skip, o))
}

/// Build, sign and pack: the IPA path.
pub fn build(st: &State, i: &Inputs, say: &mut dyn FnMut(&str)) -> Result<PathBuf, String> {
    let r = &st.runner;
    say("xcodebuild (Release, unsigned, iPhone + iPad)");
    mkdir_p(&st.o)?;
    let dd = st.dd.to_string_lossy().into_owned();
    let args: Vec<String> = [
        "-project",
        "DmIOS.xcodeproj",
        "-scheme",
        "DmIOS",
        "-configuration",
        "Release",
        "-destination",
        "generic/platform=iOS",
        "-derivedDataPath",
        &dd,
        "CODE_SIGNING_ALLOWED=NO",
        "CODE_SIGNING_REQUIRED=NO",
        "CODE_SIGN_IDENTITY=",
        "DM_COPY_GAME_RESOURCES=NO",
        &format!("DM_XCODE_OUTPUT_ROOT={dd}"),
        &format!("SYMROOT={dd}/Build/Products"),
        &format!("OBJROOT={dd}/Build/Intermediates.noindex"),
        "build",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if let Err((e, errors)) = script::xcodebuild(r, &st.ios, &args, &st.o.join("build.log")) {
        errors.iter().for_each(|l| say(l));
        return Err(e);
    }
    let app = st.dd.join("Build/Products/Release-iphoneos/DmIOS.app");
    if !app.is_dir() {
        return Err(die("no built app"));
    }
    say(&format!("app: {}", app.display()));
    rm_rf(&app.join("msl"))?;
    rm_rf(&st.o.join("Payload"))?;
    let out = &st.out_ipa;
    for s in ["", ".idx"] {
        let old = with_suffix(&st.o.join("out.ipa"), s);
        if !same_file(&old, &with_suffix(out, s)) {
            let _ = std::fs::remove_file(&old);
        }
    }
    if i.entitlements.as_deref() != Some("") {
        let ent = st.o.join("DmIOS.entitlements");
        entitlements(i, &ent)?;
        let mut exe = app.join("DmIOS");
        let info = app.join("Info.plist");
        if info.is_file() {
            let name = r
                .stdout(
                    "/usr/libexec/PlistBuddy",
                    [
                        "-c".as_ref(),
                        "Print :CFBundleExecutable".as_ref(),
                        info.as_os_str(),
                    ],
                    None,
                )
                .map_err(|_| die(format!("CFBundleExecutable of {}", app.display())))?;
            exe = app.join(name.trim_end_matches('\n'));
        }
        let _ = r.run("xattr", ["-cr".as_ref(), exe.as_os_str()], None);
        r.run(
            "codesign",
            [
                "--force".as_ref(),
                "--sign".as_ref(),
                "-".as_ref(),
                "--entitlements".as_ref(),
                ent.as_os_str(),
                exe.as_os_str(),
            ],
            None,
        )
        .map_err(|_| die("ad-hoc signature with the entitlements"))?;
        r.run(
            "codesign",
            [
                "-d".as_ref(),
                "--entitlements".as_ref(),
                "-".as_ref(),
                exe.as_os_str(),
            ],
            None,
        )
        .map_err(|_| die("the executable's signature has no entitlements"))?;
        say(
            "entitlements in the executable's ad-hoc signature (the install tool re-signs and keeps them)",
        );
    }
    let (maps, _, o) = plan(st, i, &app, say)?;
    let n = nxb_pack::ipa::pack(out, &maps, &o).map_err(|e| die(format!("pack_ipa ({e})")))?;
    say(&format!("pack: {n} entries"));
    say(&format!(
        "done: {} ({})",
        out.display(),
        script::du_sh(r, out).replace("\t", "")
    ));
    Ok(out.clone())
}
