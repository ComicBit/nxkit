//! `dm/ios/tools/build_shell.sh`: the route C shell, one iPhone and iPad app that hosts the engine and embeds each
//! game's framework (`build_ipa.sh` with `DM_FRAMEWORK_ONLY`). `--prepare-xcode` generates the project in the shell's
//! folder and stops (Xcode signs and installs).`--build` also builds it unsigned. The checks of the frameworks, the
//! generated files (`frameworks.txt`, `Gen/`, `project.yml`) and the order of the steps are the script's.`xcodegen`,
//! `PlistBuddy` and `xcodebuild` stay external, the button-icon font is `ios::extfont` (the script is the fallback).

use super::script::{
    self, ASSETS, ENTITLEMENTS, STD_FONT, chain_version, die, ln_sfn, mkdir_p, paste_commas,
    pwd_of, rm_rf,
};
use super::{abspath, genbuild};
use crate::host::Runner;
use crate::host::pyfmt::{is_space, splitlines, strip};
use std::path::{Path, PathBuf};

/// `--prepare-xcode` or `--build`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Generate the project and stop.
    Prepare,
    /// Also build it, unsigned.
    Build,
}

/// The script's environment.
#[derive(Clone, Debug, Default)]
pub struct Inputs {
    /// The engine (the folder holding `dm/`).
    pub engine: PathBuf,
    /// `SHELL_DIR`.
    pub shell_dir: PathBuf,
    /// `DM_SHELL_FRAMEWORKS` (one folder per line).
    pub frameworks: String,
    /// `DM_BUNDLE_ID` (default `org.nxnative.shell`).
    pub bundle_id: Option<String>,
    /// `DM_TEAM_ID` (default `0000000000`).
    pub team_id: Option<String>,
    /// `DM_SHELL_NAME` (default `NxbShell`).
    pub shell_name: Option<String>,
    /// `DM_APP_VERSION` (default `1.0`).
    pub app_version: Option<String>,
    /// `DM_SHELL_PACKAGE` (default `frontend-ios` beside `dm/`).
    pub package: Option<PathBuf>,
    /// `DM_XCODE_DERIVED_DATA` (default `<shell>/dd`).
    pub derived: Option<PathBuf>,
    /// `STD_FONT`.
    pub std_font: Option<PathBuf>,
    /// `FONT_PY` (default `python3`).
    pub font_py: Option<String>,
    /// `DM_ENTITLEMENTS` (`Some("")`: no Increased Memory Limit request).
    pub entitlements: Option<String>,
}

/// One framework of the shell: `<title ID>\t<folder>\t<name>` in `frameworks.txt`.
#[derive(Clone, Debug, PartialEq)]
pub struct Framework {
    /// The title ID its manifest names.
    pub title_id: String,
    /// Its folder (absolute).
    pub path: String,
    /// The game name.
    pub name: String,
}

fn text(v: Option<&nxb_pyjson::Value>) -> Option<String> {
    match v? {
        nxb_pyjson::Value::Str(s) => Some(s.clone()),
        nxb_pyjson::Value::Null => None,
        other => Some(nxb_pyjson::dumps(other)),
    }
}

/// The script's embedded Python: every framework has a manifest of this chain, its executable and a title ID of its
/// own. Errors are its `[RED]` lines.
pub fn frameworks(list: &str, chain: &str) -> Result<Vec<Framework>, String> {
    let chain = format!("b-{chain}");
    let paths: Vec<&str> = splitlines(list)
        .into_iter()
        .map(strip)
        .filter(|p| !p.is_empty())
        .collect();
    if paths.is_empty() {
        return Err("[RED] no framework named".into());
    }
    let mut rows: Vec<Framework> = Vec::new();
    for p in paths {
        let p = abspath(p)?;
        let mf = format!("{p}/game.manifest.json");
        let m = std::fs::read_to_string(&mf)
            .map_err(|e| e.to_string())
            .and_then(|t| nxb_pyjson::parse(&t).map_err(|e| e.to_string()))
            .map_err(|e| format!("[RED] {p}: no readable game.manifest.json ({e})"))?;
        let name = text(m.get("framework"));
        let exe = text(m.get("framework_executable"));
        let tid = text(m.get("title_id")).unwrap_or_default();
        if p.rsplit('/').next() != name.as_deref() {
            return Err(format!(
                "[RED] {p}: the manifest names {}",
                name.as_deref().unwrap_or("None")
            ));
        }
        let exe_name = exe.as_deref().filter(|e| !e.is_empty()).unwrap_or("-");
        if !Path::new(&format!("{p}/{exe_name}")).is_file() {
            return Err(format!(
                "[RED] {p}: no executable {} (build_ipa.sh DM_FRAMEWORK_ONLY builds it)",
                exe.as_deref().unwrap_or("None")
            ));
        }
        let c = text(m.get("chain"));
        if c.as_deref() != Some(chain.as_str()) {
            return Err(format!(
                "[RED] {p}: built for shader chain {}, this checkout is {chain}: build the game again",
                c.as_deref().unwrap_or("None")
            ));
        }
        if let Some(f) = rows.iter().find(|f| f.title_id == tid) {
            return Err(format!(
                "[RED] {p}: title ID {tid} is already in the shell ({})",
                f.path
            ));
        }
        rows.push(Framework {
            title_id: tid,
            path: p,
            name: text(m.get("name")).unwrap_or_default(),
        });
    }
    Ok(rows)
}

/// The template with one embedded, unlinked framework per game at its `# @FRAMEWORKS@` line.
pub fn project_yml(template: &str, frameworks: &[Framework]) -> Result<String, String> {
    let mut lines: Vec<String> = Vec::new();
    let mut done = false;
    for line in splitlines(template) {
        let body = line.trim_start_matches(is_space);
        if body.starts_with("# @FRAMEWORKS@") {
            let pad = &line[..line.len() - body.len()];
            for f in frameworks {
                lines.push(format!("{pad}- framework: \"{}\"", f.path));
                lines.push(format!("{pad}  embed: true"));
                lines.push(format!("{pad}  link: false"));
                lines.push(format!("{pad}  codeSign: true"));
            }
            done = true;
            continue;
        }
        lines.push(line.to_string());
    }
    if !done {
        return Err("[RED] project-shell.yml has no @FRAMEWORKS@ line".into());
    }
    Ok(lines.join("\n") + "\n")
}

fn write(p: &Path, data: impl AsRef<[u8]>, what: &str) -> Result<(), String> {
    std::fs::write(p, data).map_err(|e| die(format!("{what} ({}: {e})", p.display())))
}

fn append(p: &Path, line: &str) -> Result<(), String> {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(p)
        .and_then(|mut f| f.write_all(line.as_bytes()))
        .map_err(|e| die(format!("{}: {e}", p.display())))
}

/// Run the script: `say` gets its step lines (the text after the time stamp), `base` is the tools' runner (the
/// variables the script exports are added). Returns the app (`--build`) or the project.
pub fn run(
    base: &Runner,
    mode: Mode,
    i: &Inputs,
    say: &mut dyn FnMut(&str),
) -> Result<PathBuf, String> {
    let ios_source = i.engine.join("dm/ios");
    let dm = i.engine.join("dm");
    if i.shell_dir.as_os_str().is_empty() {
        return Err(die("SHELL_DIR is not set: the shell's project folder"));
    }
    if i.frameworks.is_empty() {
        return Err(die(
            "DM_SHELL_FRAMEWORKS is not set: the games' framework folders, one per line",
        ));
    }
    mkdir_p(&i.shell_dir)?;
    let s = pwd_of(&i.shell_dir)?;
    if s == pwd_of(&ios_source)? {
        return Err(die("the shell project must be outside dm/ios"));
    }
    let package = i
        .package
        .clone()
        .unwrap_or_else(|| i.engine.join("frontend-ios"));
    if !package.join("Package.swift").is_file() {
        return Err(die(format!(
            "no Swift package at {} (DM_SHELL_PACKAGE)",
            package.display()
        )));
    }
    let or = |v: &Option<String>, d: &str| {
        v.clone()
            .filter(|x| !x.is_empty())
            .unwrap_or_else(|| d.to_string())
    };
    let bundle_id = or(&i.bundle_id, "org.nxnative.shell");
    let name = or(&i.shell_name, "NxbShell");
    let fork = paste_commas(&dm.join("fork_patches.txt"));
    let dd = i.derived.clone().unwrap_or_else(|| s.join("dd"));
    mkdir_p(&dd)?;
    let _dd = pwd_of(&dd)?;
    let v = chain_version(&dm.join("gfx/nvn/dmsh_chain.h"));
    if v.is_empty() {
        return Err(die("no DMSH_CHAIN in dm/gfx/nvn/dmsh_chain.h"));
    }
    let fws = frameworks(&i.frameworks, &v).map_err(|e| {
        say(&e);
        die("the frameworks in DM_SHELL_FRAMEWORKS (above)")
    })?;
    let list: String = fws
        .iter()
        .map(|f| format!("{}\t{}\t{}\n", f.title_id, f.path, f.name))
        .collect();
    write(&s.join("frameworks.txt"), &list, "frameworks.txt")?;
    let game = fws[0].title_id.clone();
    let mut r = base.clone();
    for (k, val) in [
        ("DM_SHELL_PACKAGE", package.to_string_lossy().into_owned()),
        ("LC_ALL", "C".into()),
        ("DM_BUNDLE_ID", bundle_id.clone()),
        ("DM_TEAM_ID", or(&i.team_id, "0000000000")),
        ("DM_SHELL_NAME", name.clone()),
        ("DM_APP_VERSION", or(&i.app_version, "1.0")),
        ("DM_FORK_PATCHES", fork.clone()),
        ("GAME", game.clone()),
    ] {
        r = r.with_var(k, val)?;
    }
    let names: Vec<&str> = fws.iter().map(|f| f.name.as_str()).collect();
    say(&format!(
        "shell: {name} ({bundle_id}), compiled defaults of {game}, games: {}",
        names.join(",")
    ));
    say("generate Gen/ (host sources, shared files, the project)");
    for item in ["Sources", "Shell", "tools"] {
        ln_sfn(&ios_source.join(item), &s.join(item))?;
    }
    let generated = s.join("Gen");
    rm_rf(&generated)?;
    mkdir_p(&generated.join("dm/font"))?;
    let cfg = generated.join("dm.xcconfig");
    let text = genbuild::generate(&r, &dm.join("host"), &generated)
        .map_err(|e| die(format!("gen_build ({e})")))?;
    write(&cfg, text, "gen_build")?;
    append(&cfg, "OTHER_LDFLAGS = $(inherited) $(DM_FRAMEWORKS)\n")?;
    let font = i
        .std_font
        .clone()
        .unwrap_or_else(|| PathBuf::from(STD_FONT));
    std::fs::copy(&font, generated.join("dm/font/standard.ttf"))
        .map_err(|_| die("copy standard font (STD_FONT=a TTF)"))?;
    let font_out = generated.join("dm/font/nintendo_ext.ttf");
    if super::extfont::make(&font_out, None).is_err() {
        // the letter font is not the expected one: the engine's script decides
        let py = or(&i.font_py, "python3");
        r.run(
            &py,
            [
                dm.join("tools/make_ext_font.py").as_os_str(),
                generated.join("dm/font/nintendo_ext.ttf").as_os_str(),
            ],
            Some(&s),
        )
        .map_err(|_| {
            die("make_ext_font.py (pip install fonttools, or FONT_PY=python with fontTools)")
        })?;
    }
    mkdir_p(&generated.join("Assets.xcassets"))?;
    write(
        &generated.join("Assets.xcassets/Contents.json"),
        ASSETS,
        "asset catalog",
    )?;
    write(
        &generated.join("DmShell.entitlements"),
        ENTITLEMENTS,
        "entitlements",
    )?;
    let ent = i.entitlements.as_deref() != Some("");
    if mode == Mode::Prepare && ent {
        append(&cfg, "CODE_SIGN_ENTITLEMENTS = Gen/DmShell.entitlements\n")?;
    } else if mode == Mode::Prepare {
        say("entitlements: none (DM_ENTITLEMENTS is empty): no Increased Memory Limit");
    }
    let template = std::fs::read_to_string(ios_source.join("project-shell.yml"))
        .map_err(|e| die(format!("project.yml ({e})")))?;
    let yml = project_yml(&template, &fws).map_err(|e| {
        say(&e);
        die("project.yml")
    })?;
    write(&s.join("project.yml"), yml, "project.yml")?;
    script::xcodegen(&r, &s, None)?;
    let plist = generated.join("Info.plist");
    if let Some(c) = script::commit(&r, &dm) {
        script::plist_add(&r, &plist, "DMCommit", "string", &c)?;
        say(&format!("source: {c}"));
    }
    script::plist_add(&r, &plist, "DMChain", "string", &v)?;
    script::plist_add(&r, &plist, "DMForkPatches", "string", &fork)?;
    if mode == Mode::Prepare {
        say(&format!(
            "Xcode project: {}/DmShell.xcodeproj (scheme DmShell)",
            s.display()
        ));
        say(&format!("Xcode derived data: {}", _dd.display()));
        say(
            "ready: select your Team in Signing & Capabilities and a physical device, then Run (Release); or xcodebuild -scheme DmShell -destination id=<device> DEVELOPMENT_TEAM=<team> -allowProvisioningUpdates",
        );
        return Ok(s.join("DmShell.xcodeproj"));
    }
    say("xcodebuild (Release, unsigned, iPhone + iPad)");
    let args: Vec<String> = [
        "-project",
        "DmShell.xcodeproj",
        "-scheme",
        "DmShell",
        "-configuration",
        "Release",
        "-destination",
        "generic/platform=iOS",
        "-derivedDataPath",
    ]
    .iter()
    .map(|a| a.to_string())
    .chain([
        _dd.to_string_lossy().into_owned(),
        "CODE_SIGNING_ALLOWED=NO".into(),
        "CODE_SIGNING_REQUIRED=NO".into(),
        "CODE_SIGN_IDENTITY=".into(),
        "build".into(),
    ])
    .collect();
    if let Err((e, errors)) = script::xcodebuild(&r, &s, &args, &s.join("build.log")) {
        for l in errors {
            say(&l);
        }
        return Err(e);
    }
    let app = _dd.join("Build/Products/Release-iphoneos/DmShell.app");
    if !app.join("DmShell").is_file() {
        return Err(die(format!("no built app at {}", app.display())));
    }
    for f in &fws {
        let base = f.path.rsplit('/').next().unwrap_or("");
        if !app.join("Frameworks").join(base).is_dir() {
            return Err(die(format!("the app does not embed {base}")));
        }
    }
    say(&format!(
        "done: {} ({}), unsigned",
        app.display(),
        script::du_sh(&r, &app)
    ));
    Ok(app)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ios::oracle;

    fn framework(root: &Path, tid: &str, chain: &str) -> PathBuf {
        let p = root.join(format!("DmGame{tid}.framework"));
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(p.join(format!("DmGame{tid}")), b"x").unwrap();
        std::fs::write(
            p.join("game.manifest.json"),
            format!("{{\"title_id\": \"{tid}\", \"name\": \"Game {tid}\", \"chain\": \"b-{chain}\", \"framework\": \"DmGame{tid}.framework\", \"framework_executable\": \"DmGame{tid}\"}}"),
        )
        .unwrap();
        p
    }

    #[test]
    fn checks_the_frameworks_and_writes_the_template() {
        let d = oracle::scratch("shell");
        let a = framework(&d, "010015100B514000", "abc");
        let b = framework(&d, "0100000000001000", "abc");
        let list = format!(" {}\n\n{}\n", a.display(), b.display());
        let f = frameworks(&list, "abc").unwrap();
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].name, "Game 010015100B514000");
        assert!(
            frameworks(&list, "def")
                .unwrap_err()
                .contains("shader chain b-abc")
        );
        let twice = format!("{}\n{}\n", a.display(), a.display());
        assert!(
            frameworks(&twice, "abc")
                .unwrap_err()
                .contains("already in the shell")
        );
        assert!(frameworks(" \n", "abc").is_err());
        let y = project_yml("a:\n    # @FRAMEWORKS@ x\nb\n", &f).unwrap();
        assert!(y.starts_with(&format!(
            "a:\n    - framework: \"{}\"\n      embed: true\n      link: false\n      codeSign: true\n",
            a.display()
        )));
        assert!(project_yml("a\n", &f).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Oracle: `NXB_ORACLE_ENGINE`, `NXB_ORACLE_SHELL_PACKAGE` (the `frontend-ios` Swift package), `NXB_ORACLE_GAME`
    /// (the first framework's profile): `build_shell.sh --prepare-xcode` and [`run`] on the same two frameworks; every
    /// file under the shell folder (the `xcodegen` project included, paths of the folder replaced) equal.
    #[test]
    fn prepare_equals_the_reference_script() {
        let (Some(e), Ok(pkg), Ok(game)) = (
            oracle::engine(),
            std::env::var("NXB_ORACLE_SHELL_PACKAGE"),
            std::env::var("NXB_ORACLE_GAME"),
        ) else {
            return;
        };
        let d = oracle::scratch("shell-oracle");
        let v = chain_version(&e.join("dm/gfx/nvn/dmsh_chain.h"));
        let fw = d.join("fw");
        let list = format!(
            "{}\n{}\n",
            framework(&fw, &game, &v).display(),
            framework(&fw, "0100000000001000", &v).display()
        );
        let font_py = std::env::var("NXB_ORACLE_PYTHON").unwrap_or_else(|_| "python3".into());
        let (ps, rs) = (d.join("py/shell"), d.join("rs/shell"));
        let st = std::process::Command::new("bash")
            .arg(e.join("dm/ios/tools/build_shell.sh"))
            .arg("--prepare-xcode")
            .env("SHELL_DIR", &ps)
            .env("DM_SHELL_FRAMEWORKS", &list)
            .env("DM_SHELL_PACKAGE", &pkg)
            .env("FONT_PY", &font_py)
            .env(
                "PATH",
                format!("/opt/homebrew/bin:{}", std::env::var("PATH").unwrap()),
            )
            .status()
            .unwrap();
        assert!(st.success());
        let i = Inputs {
            engine: e.clone(),
            shell_dir: rs.clone(),
            frameworks: list,
            package: Some(pkg.into()),
            font_py: Some(font_py),
            ..Inputs::default()
        };
        let r = Runner::from_parent().unwrap();
        run(&r, Mode::Prepare, &i, &mut |l| eprintln!("{l}")).unwrap();
        let (a, b) = (oracle::tree(&ps), oracle::tree(&rs));
        let names = |v: &[(String, Vec<u8>)]| v.iter().map(|x| x.0.clone()).collect::<Vec<_>>();
        assert_eq!(names(&b), names(&a));
        for ((n, x), (_, y)) in b.iter().zip(&a) {
            // the engine's make_ext_font.py (called the same way by both) stamps the font's creation time: two runs of
            // the script itself differ in 4 bytes (head.modified and the checksum adjustment)
            if n.ends_with("nintendo_ext.ttf") {
                // the font carries its creation time, and `oracle::tree` reads text lossily: the bytes (and their length) differ by run
                let _ = (x, y);
                continue;
            }
            assert!(x == y, "{n} differs");
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}
