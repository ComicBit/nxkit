//! Tests of the IPA flow: the profile helpers, and the oracle against `build_ipa.sh`.

use super::*;
use crate::ios::ipagen::pso_gpus;
use crate::ios::oracle;

#[test]
fn pipeline_archive_architectures() {
    assert_eq!(pso_gpus(&None, "off", "g18p"), ("none".into(), None));
    assert_eq!(pso_gpus(&None, "on", ""), ("g18p".into(), None));
    assert_eq!(
        pso_gpus(&None, "on", "g18p  applegpu_g19p"),
        ("g18p,applegpu_g19p".into(), None)
    );
    assert_eq!(pso_gpus(&Some(String::new()), "on", "g18p").0, "");
    let (g, w) = pso_gpus(&None, "on", "none");
    assert_eq!(g, "g18p");
    assert!(w.unwrap().contains("not 1-4"));
    assert_eq!(pso_gpus(&None, "on", "a b c d e").0, "g18p");
}

#[test]
fn modules_of_the_main_build() {
    let d = oracle::scratch("ipa-mods");
    let g = d.join("g");
    std::fs::create_dir_all(&g).unwrap();
    std::fs::write(
        g.join("game.tsv"),
        "modules\tmain sdk subsdk0\nbuild\tv0\t1.0\tmain=aa\tsdk=bb\n",
    )
    .unwrap();
    let game = Game::open(&g).unwrap();
    std::fs::write(
        d.join("main.fixups.tsv"),
        "# TEXT_END\t0x10\n# BUILD_ID\taa\n",
    )
    .unwrap();
    assert_eq!(
        modules(&game, d.to_str().unwrap()).unwrap(),
        ["main", "sdk"]
    );
    std::fs::write(d.join("main.fixups.tsv"), "# BUILD_ID\tzz\n").unwrap();
    assert_eq!(
        modules(&game, d.to_str().unwrap()).unwrap(),
        ["main", "sdk", "subsdk0"]
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// Oracle: `NXB_ORACLE_ENGINE`, `NXB_ORACLE_GAME`, `NXB_ORACLE_IOS_PATCHED` (a rewriter output of that game),
/// `NXB_ORACLE_IOS_ROMFS` (its RomFS image or folder.`control.nacp` and `icon.jpg` beside it): `build_ipa.sh
/// --prepare-xcode` and [`run`] with the same inputs and a shader folder made here (a stamp of the profile's current
/// inputs and one library), every file of the two project folders equal (paths of the folders replaced), except the
/// button-icon font of the engine's font tool.
#[test]
fn prepare_equals_the_reference_script() {
    let (Some(e), Ok(game), Ok(patched), Ok(romfs)) = (
        oracle::engine(),
        std::env::var("NXB_ORACLE_GAME"),
        std::env::var("NXB_ORACLE_IOS_PATCHED"),
        std::env::var("NXB_ORACLE_IOS_ROMFS"),
    ) else {
        return;
    };
    let d = oracle::scratch("ipa-oracle");
    let v = crate::ios::script::chain_version(&e.join("dm/gfx/nvn/dmsh_chain.h"));
    let g = Profiles::new(&e, &|_| None).load(&game).unwrap();
    let inp = nxb_shader::chain::inputs(&g.dir).unwrap();
    let msl = d.join("msl");
    std::fs::create_dir_all(msl.join(format!("b-{v}"))).unwrap();
    std::fs::write(
        msl.join(format!("b-{v}/inputs.tsv")),
        format!("# stamp\ngame\t{game}\ninputs\t{inp}\n"),
    )
    .unwrap();
    std::fs::write(msl.join(format!("b-{v}/0000_b.metallib")), b"lib").unwrap();
    let font_py = std::env::var("NXB_ORACLE_PYTHON").unwrap_or_else(|_| "python3".into());
    let r = Runner::from_parent().unwrap();
    let path = r.env().get("PATH").unwrap().to_string_lossy().into_owned();
    let user = std::env::var("USER").unwrap_or_else(|_| "tester".into());
    let (pw, rw) = (d.join("py/w"), d.join("rs/w"));
    let o = std::process::Command::new("bash")
        .arg(e.join("dm/ios/tools/build_ipa.sh"))
        .arg("--prepare-xcode")
        .env("GAME", &game)
        .env("W", &pw)
        .env("PATCHED", &patched)
        .env("ROMFS", &romfs)
        .env("MSL_IOS", &msl)
        .env("FONT_PY", &font_py)
        .env("PATH", &path)
        .env("USER", &user)
        .output()
        .unwrap();
    let script_out = String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(o.status.success(), "{script_out}");
    let i = Inputs {
        engine: e.clone(),
        game: game.clone(),
        w: rw.to_string_lossy().into_owned(),
        patched: Some(patched),
        romfs: Some(romfs),
        msl_ios: Some(msl.to_string_lossy().into_owned()),
        font_py: Some(font_py),
        user,
        ..Inputs::default()
    };
    let mut lines = Vec::new();
    run(&r, Target::Prepare, &i, &mut |l| lines.push(l.to_string())).unwrap();
    // the step lines, without the script's time stamps and with the work folders' paths replaced
    let norm = |s: &str, w: &Path| {
        let real = std::fs::canonicalize(w).unwrap();
        s.replace(real.to_str().unwrap(), "<w>")
            .replace(w.to_str().unwrap(), "<w>")
    };
    let want: Vec<String> = script_out
        .lines()
        .filter_map(|l| l.get(11..).filter(|_| l.starts_with('[')))
        .map(|l| norm(l, &pw))
        .collect();
    let got: Vec<String> = lines.iter().map(|l| norm(l, &rw)).collect();
    assert_eq!(got, want);
    let (a, b) = (
        oracle::tree(&pw.join("xcode")),
        oracle::tree(&rw.join("xcode")),
    );
    let names = |v: &[(String, Vec<u8>)]| v.iter().map(|x| x.0.clone()).collect::<Vec<_>>();
    eprintln!("oracle: {} step lines, {} files", want.len(), a.len());
    assert_eq!(names(&b), names(&a));
    for ((n, x), (_, y)) in b.iter().zip(&a) {
        // the engine's font tool stamps its creation time (two runs of the tool itself differ in 4 bytes)
        if n.ends_with("nintendo_ext.ttf") {
            // the font carries its creation time, and `oracle::tree` reads text lossily: the bytes (and their length) differ by run
            let _ = (x, y);
            continue;
        }
        let (x, y) = (
            norm(&String::from_utf8_lossy(x), &rw),
            norm(&String::from_utf8_lossy(y), &pw),
        );
        assert!(x == y, "{n} differs");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// The entries of an IPA without their times (name, method, sizes, CRC, flags, attributes), by the oracle's Python.
pub(crate) fn entries(ipa: &Path) -> String {
    let code = "import sys, zipfile\nfor i in zipfile.ZipFile(sys.argv[1]).infolist(): print(i.filename, i.compress_type, i.compress_size, i.file_size, i.CRC, i.flag_bits, i.external_attr, i.create_system)";
    let o = oracle::python()
        .args(["-c", code])
        .arg(ipa)
        .output()
        .unwrap();
    assert!(o.status.success());
    String::from_utf8(o.stdout).unwrap()
}

/// Oracle (IPA mode, two `xcodebuild` runs): as [`prepare_equals_the_reference_script`] with
/// `NXB_ORACLE_IOS_IPA=1`: `build_ipa.sh <py.ipa>` and [`run`] into `<rs.ipa>`, each in its own work folder; the step
/// lines and every entry of the two IPAs (name, method, sizes, CRC, attributes, not the times, the builds ran at
/// different times) equal.
#[test]
fn ipa_equals_the_reference_script() {
    let (Some(e), Ok(game), Ok(patched), Ok(romfs), Ok(_)) = (
        oracle::engine(),
        std::env::var("NXB_ORACLE_GAME"),
        std::env::var("NXB_ORACLE_IOS_PATCHED"),
        std::env::var("NXB_ORACLE_IOS_ROMFS"),
        std::env::var("NXB_ORACLE_IOS_IPA"),
    ) else {
        return;
    };
    let d = std::path::PathBuf::from(std::env::var("NXB_ORACLE_IOS_SCRATCH").unwrap());
    let v = crate::ios::script::chain_version(&e.join("dm/gfx/nvn/dmsh_chain.h"));
    let g = Profiles::new(&e, &|_| None).load(&game).unwrap();
    let inp = nxb_shader::chain::inputs(&g.dir).unwrap();
    let msl = d.join("msl");
    std::fs::create_dir_all(msl.join(format!("b-{v}"))).unwrap();
    std::fs::write(
        msl.join(format!("b-{v}/inputs.tsv")),
        format!("# stamp\ngame\t{game}\ninputs\t{inp}\n"),
    )
    .unwrap();
    std::fs::write(msl.join(format!("b-{v}/0000_b.metallib")), b"lib").unwrap();
    std::fs::write(msl.join(format!("b-{v}/0000_bp.metallib")), b"pull").unwrap();
    let font_py = std::env::var("NXB_ORACLE_PYTHON").unwrap_or_else(|_| "python3".into());
    let r = Runner::from_parent().unwrap();
    let path = r.env().get("PATH").unwrap().to_string_lossy().into_owned();
    let user = std::env::var("USER").unwrap_or_else(|_| "tester".into());
    let (pw, rw) = (d.join("py/w"), d.join("rs/w"));
    let (pi, ri) = (d.join("py.ipa"), d.join("rs.ipa"));
    if !pi.is_file() {
        let o = std::process::Command::new("bash")
            .arg(e.join("dm/ios/tools/build_ipa.sh"))
            .arg(&pi)
            .env("GAME", &game)
            .env("W", &pw)
            .env("PATCHED", &patched)
            .env("ROMFS", &romfs)
            .env("MSL_IOS", &msl)
            .env("FONT_PY", &font_py)
            .env("PATH", &path)
            .env("USER", &user)
            .output()
            .unwrap();
        std::fs::write(d.join("py.log"), &o.stdout).unwrap();
        assert!(o.status.success());
    }
    let i = Inputs {
        engine: e.clone(),
        game: game.clone(),
        w: rw.to_string_lossy().into_owned(),
        patched: Some(patched),
        romfs: Some(romfs),
        msl_ios: Some(msl.to_string_lossy().into_owned()),
        font_py: Some(font_py),
        user,
        ..Inputs::default()
    };
    let mut lines = Vec::new();
    run(&r, Target::Ipa(Some(&ri)), &i, &mut |l| {
        lines.push(l.to_string())
    })
    .unwrap();
    std::fs::write(d.join("rs.log"), lines.join("\n")).unwrap();
    // the executable's debug map names the objects in each work folder ("py" / "rs" here) and the signature covers
    // it: those bytes are in __LINKEDIT, so the executables compare by their bytes before it (UUID zeroed), and the
    // font by its size (the engine's font tool stamps its creation time)
    let loose = |s: String| -> String {
        s.lines()
            .map(|l| {
                let f: Vec<&str> = l.split(' ').collect();
                if f[0].ends_with("/DmIOS") || f[0].ends_with("nintendo_ext.ttf") {
                    format!("{} {} {}", f[0], f[1], f[3])
                } else {
                    l.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let (a, b) = (entries(&pi), entries(&ri));
    eprintln!("oracle: {} entries", a.lines().count());
    assert_eq!(loose(b), loose(a));
    let exe = |w: &Path| {
        let p = w.join("build/ios/pkg/dd/Build/Products/Release-iphoneos/DmIOS.app/DmIOS");
        crate::host::machost::code_bytes(&std::fs::read(p).unwrap()).unwrap()
    };
    assert!(exe(&rw) == exe(&pw), "the executables' code differs");
    let step = |s: &str, w: &Path| {
        s.replace(w.to_str().unwrap(), "<w>")
            .replace(&*pi.to_string_lossy(), "<ipa>")
            .replace(&*ri.to_string_lossy(), "<ipa>")
    };
    let want: Vec<String> = std::fs::read_to_string(d.join("py.log"))
        .unwrap()
        .lines()
        .filter_map(|l| l.get(11..).filter(|_| l.starts_with('[')))
        .map(|l| step(l, &pw))
        .collect();
    let got: Vec<String> = lines
        .iter()
        .filter(|l| !l.starts_with("pack: "))
        .map(|l| step(l, &rw))
        .collect();
    assert_eq!(got, want);
}
