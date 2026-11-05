//! Differential tests against Apple's `codesign` (the oracle).
//!
//! Byte comparison needs `NXB_ORACLE_EXACT=1`. Each case builds an input with `clang`, signs one copy with `codesign --force --sign -` and
//! one with this crate, and requires byte-identical files and `codesign --verify --strict` to
//! pass on ours. The tests skip (with a note on stderr) where `codesign` or `clang` is missing,
//! for example on Linux or Windows.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::collapsible_if
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use nxb_pack::bundle::{BundleOptions, sign_bundle};
use nxb_pack::digest::TsvDigests;
use nxb_pack::macho_sign::{SignParams, sign_macho};

/// Byte comparison with the host's `codesign` only when `NXB_ORACLE_EXACT=1`. Other macOS and Xcode versions write other
/// bytes than the version on which the signer was proven. Without the variable the tests require that `codesign --verify`
/// accepts our output and that the structure is valid.
fn exact() -> bool {
    std::env::var("NXB_ORACLE_EXACT").is_ok_and(|v| v == "1")
}

fn have(tool: &str) -> bool {
    Command::new("/usr/bin/which")
        .arg(tool)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn tools() -> bool {
    let ok = cfg!(target_os = "macos") && have("codesign") && have("clang");
    if !ok {
        eprintln!("skipping: codesign or clang not available");
    }
    ok
}

fn run(cmd: &mut Command) {
    let out = cmd.output().expect("spawn");
    assert!(
        out.status.success(),
        "{cmd:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn tmpdir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("nxb-codesign-oracle-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// Builds `main.c` into `out` with extra clang arguments.
fn build(dir: &Path, out: &str, args: &[&str]) -> PathBuf {
    let src = dir.join("main.c");
    fs::write(&src, "int main(void){return 0;}\n").unwrap();
    let p = dir.join(out);
    let mut c = Command::new("clang");
    c.args(args).arg("-o").arg(&p).arg(&src);
    run(&mut c);
    p
}

fn sdk(name: &str) -> Option<String> {
    let o = Command::new("xcrun")
        .args(["--sdk", name, "--show-sdk-path"])
        .output()
        .ok()?;
    o.status
        .success()
        .then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn compare_file(input: &Path, name: &str, entitlements: Option<&Path>) {
    compare_file_opts(input, name, entitlements, false);
}

fn compare_file_opts(input: &Path, name: &str, entitlements: Option<&Path>, runtime: bool) {
    let base = input.parent().unwrap();
    let (od, nd) = (base.join("oracle"), base.join("ours"));
    fs::create_dir_all(&od).unwrap();
    fs::create_dir_all(&nd).unwrap();
    let (o, n) = (od.join(name), nd.join(name));
    fs::copy(input, &o).unwrap();
    let mut c = Command::new("codesign");
    c.args(["--force", "--sign", "-"]);
    if runtime {
        c.args(["--options", "runtime"]);
    }
    if let Some(e) = entitlements {
        c.arg("--entitlements").arg(e);
    }
    run(c.arg(&o));
    let mut params = SignParams::standalone(name);
    params.entitlements = entitlements.map(|e| fs::read(e).unwrap());
    params.runtime = runtime;
    let signed = sign_macho(&fs::read(input).unwrap(), &params).unwrap();
    fs::write(&n, &signed.data).unwrap();
    assert!(
        !exact() || fs::read(&o).unwrap() == signed.data,
        "{name}: output differs from codesign"
    );
    run(Command::new("codesign")
        .args(["--verify", "--strict"])
        .arg(&n));
}

#[test]
fn single_files_match_codesign() {
    if !tools() {
        return;
    }
    let d = tmpdir("files");
    let cases: Vec<(&str, Vec<String>)> = vec![
        ("arm64", vec!["-arch".into(), "arm64".into()]),
        ("x86_64", vec!["-arch".into(), "x86_64".into()]),
        (
            "old-target",
            vec![
                "-arch".into(),
                "x86_64".into(),
                "-mmacosx-version-min=10.9".into(),
            ],
        ),
        ("libtool.1.dylib", vec!["-dynamiclib".into()]),
        ("no-uuid", vec!["-Wl,-no_uuid".into()]),
    ];
    for (name, args) in &cases {
        let a: Vec<&str> = args.iter().map(String::as_str).collect();
        let p = build(&d, &format!("in-{name}"), &a);
        compare_file(&p, name, None);
    }
    if let Some(ios) = sdk("iphoneos") {
        for v in ["10.3", "12.0", "15.7", "16.0"] {
            let p = build(
                &d,
                &format!("in-ios{v}"),
                &["-target", &format!("arm64-apple-ios{v}"), "-isysroot", &ios],
            );
            compare_file(&p, &format!("ios{v}"), None);
        }
    }
    let ent = d.join("ent.plist");
    fs::write(
        &ent,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n\t<key>com.apple.security.get-task-allow</key>\n\t<true/>\n\t<key>keychain-access-groups</key>\n\t<array><string>A.*</string></array>\n\t<key>n</key>\n\t<integer>300</integer>\n</dict>\n</plist>\n",
    )
    .unwrap();
    let p = build(&d, "in-ent", &[]);
    compare_file(&p, "ent", Some(&ent));
    compare_file_opts(&p, "ent-runtime", Some(&ent), true);
    compare_file_opts(&p, "runtime", None, true);
    if let Some(tv) = sdk("appletvos") {
        let p = build(
            &d,
            "in-tv",
            &["-target", "arm64-apple-tvos17.0", "-isysroot", &tv],
        );
        compare_file(&p, "tvos17", None);
    }
    // Universal file: arm64 + x86_64.
    let arm = build(&d, "in-fat-arm", &["-arch", "arm64"]);
    let x86 = build(&d, "in-fat-x86", &["-arch", "x86_64"]);
    let fat = d.join("in-fat");
    run(Command::new("lipo")
        .arg("-create")
        .arg(&arm)
        .arg(&x86)
        .arg("-output")
        .arg(&fat));
    compare_file(&fat, "fat", None);
    compare_file_opts(&fat, "fat-runtime", None, true);
    fs::remove_dir_all(&d).unwrap();
}

/// A bundle with the shapes a typical Mac app has plus the corners of the rules.
fn make_bundle(root: &Path, main: &Path, helper: &Path) -> PathBuf {
    let app = root.join("T.app");
    let c = app.join("Contents");
    for d in [
        "MacOS",
        "Resources/sub/dir",
        "Resources/en.lproj",
        "Resources/Base.lproj",
        "Resources/empty",
        "Other/d",
    ] {
        fs::create_dir_all(c.join(d)).unwrap();
    }
    fs::write(
        c.join("Info.plist"),
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n\t<key>CFBundleExecutable</key>\n\t<string>main</string>\n\t<key>CFBundleIdentifier</key>\n\t<string>org.test.t</string>\n</dict>\n</plist>\n",
    )
    .unwrap();
    fs::write(c.join("PkgInfo"), "APPL????").unwrap();
    fs::copy(main, c.join("MacOS/main")).unwrap();
    fs::copy(helper, c.join("MacOS/helper")).unwrap();
    let files: &[(&str, &[u8])] = &[
        ("Resources/a.txt", b"hi\n"),
        ("Resources/empty.bin", b""),
        ("Resources/sp ace.txt", b"s"),
        ("Resources/\u{fc}.txt", b"u"),
        ("Resources/a&b<c>\"q'.txt", b"q"),
        ("Resources/Zed.txt", b"Z"),
        ("Resources/_u.txt", b"_"),
        ("Resources/.DS_Store", b"d"),
        ("Resources/sub/dir/x.json", b"{}"),
        ("Resources/en.lproj/Localizable.strings", b"l"),
        ("Resources/en.lproj/locversion.plist", b"v"),
        ("Resources/Base.lproj/b.txt", b"b"),
        ("Other/d/f", b"o"),
        ("version.plist", b"x"),
    ];
    for (p, data) in files {
        fs::write(c.join(p), data).unwrap();
    }
    let big: Vec<u8> = (0..(40u32 << 20))
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8)
        .collect();
    fs::write(c.join("Resources/big.bin"), big).unwrap();
    std::os::unix::fs::symlink("a.txt", c.join("Resources/link")).unwrap();
    std::os::unix::fs::symlink("../a.txt", c.join("Resources/sub/up")).unwrap();
    app
}

fn clone_tree(from: &Path, to: &Path) {
    run(Command::new("cp").arg("-R").arg(from).arg(to));
}

fn assert_same_bundle(o: &Path, n: &Path) {
    for f in [
        "Contents/_CodeSignature/CodeResources",
        "Contents/MacOS/main",
        "Contents/MacOS/helper",
    ] {
        assert!(
            !exact() || fs::read(o.join(f)).unwrap() == fs::read(n.join(f)).unwrap(),
            "{f} differs from codesign"
        );
    }
    run(Command::new("codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(n));
}

fn bundle_case(name: &str, clang_args: &[&str]) {
    let d = tmpdir(name);
    let exe = build(&d, "exe", clang_args);
    let src = make_bundle(&d.join("src"), &exe, &exe);
    let (o, n) = (d.join("o/T.app"), d.join("n/T.app"));
    fs::create_dir_all(d.join("o")).unwrap();
    fs::create_dir_all(d.join("n")).unwrap();
    clone_tree(&src, &o);
    clone_tree(&src, &n);
    run(Command::new("codesign")
        .args(["--force", "--sign", "-"])
        .arg(o.join("Contents/MacOS/helper")));
    run(Command::new("codesign")
        .args(["--force", "--sign", "-"])
        .arg(&o));
    let r = sign_bundle(&n, &BundleOptions::default()).unwrap();
    assert_eq!(r.nested_signed, 1);
    assert_same_bundle(&o, &n);

    // Precomputed digests give the same bytes without reading the files.
    let digests = nxb_pack::bundle::bundle_digests(&src, 2).unwrap();
    let tsv: String = digests
        .iter()
        .map(|(p, s, d)| TsvDigests::line(p, *s, d) + "\n")
        .collect();
    let source = TsvDigests::parse(&tsv).unwrap();
    let p = d.join("p/T.app");
    fs::create_dir_all(d.join("p")).unwrap();
    clone_tree(&src, &p);
    let r = sign_bundle(
        &p,
        &BundleOptions {
            digests: &source,
            ..BundleOptions::default()
        },
    )
    .unwrap();
    assert_eq!(r.hashed_files, 0);
    assert!(r.reused_digests > 10);
    assert_same_bundle(&o, &p);

    // Re-signing a signed bundle matches codesign re-signing it.
    run(Command::new("codesign")
        .args(["--force", "--sign", "-"])
        .arg(o.join("Contents/MacOS/helper")));
    run(Command::new("codesign")
        .args(["--force", "--sign", "-"])
        .arg(&o));
    sign_bundle(&n, &BundleOptions::default()).unwrap();
    assert_same_bundle(&o, &n);
    fs::remove_dir_all(&d).unwrap();
}

#[test]
fn bundle_matches_codesign() {
    if tools() {
        bundle_case("bundle", &[]);
    }
}

#[test]
fn bundle_with_sha1_matches_codesign() {
    if tools() {
        bundle_case(
            "bundle-sha1",
            &["-arch", "x86_64", "-mmacosx-version-min=10.9"],
        );
    }
}

#[test]
fn non_code_in_nested_location_is_refused() {
    if !tools() {
        return;
    }
    let d = tmpdir("refused");
    let exe = build(&d, "exe", &[]);
    let app = make_bundle(&d.join("src"), &exe, &exe);
    fs::write(app.join("Contents/MacOS/script.txt"), "x").unwrap();
    let e = sign_bundle(&app, &BundleOptions::default()).unwrap_err();
    assert!(matches!(e, nxb_pack::Error::Refused(_)), "{e}");
    fs::remove_dir_all(&d).unwrap();
}

fn write(p: &Path, data: &str) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, data).unwrap();
}

fn info_plist(exe: &str, id: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n\t<key>CFBundleExecutable</key>\n\t<string>{exe}</string>\n\t<key>CFBundleIdentifier</key>\n\t<string>{id}</string>\n</dict>\n</plist>\n"
    )
}

fn same_files(o: &Path, n: &Path, files: &[&str]) {
    for f in files {
        assert!(
            !exact() || fs::read(o.join(f)).unwrap() == fs::read(n.join(f)).unwrap(),
            "{f} differs from codesign"
        );
    }
    run(Command::new("codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(n));
}

/// macOS bundle with a helper app and a versioned framework, signed inside out.
#[test]
fn nested_macos_bundles_match_codesign() {
    if !tools() {
        return;
    }
    let d = tmpdir("nested");
    let exe = build(&d, "exe", &[]);
    let src = d.join("src/N.app");
    let c = src.join("Contents");
    write(&c.join("Info.plist"), &info_plist("main", "org.test.n"));
    write(&c.join("Resources/a.txt"), "a");
    fs::create_dir_all(c.join("MacOS")).unwrap();
    fs::copy(&exe, c.join("MacOS/main")).unwrap();
    let h = c.join("Helpers/H.app/Contents");
    write(&h.join("Info.plist"), &info_plist("H", "org.test.h"));
    fs::create_dir_all(h.join("MacOS")).unwrap();
    fs::copy(&exe, h.join("MacOS/H")).unwrap();
    let f = c.join("Frameworks/F.framework");
    write(
        &f.join("Versions/A/Resources/Info.plist"),
        &info_plist("F", "org.test.F"),
    );
    write(&f.join("Versions/A/Resources/r.txt"), "r");
    let lib = build(
        &d,
        "libF",
        &[
            "-dynamiclib",
            "-install_name",
            "@rpath/F.framework/Versions/A/F",
        ],
    );
    fs::copy(&lib, f.join("Versions/A/F")).unwrap();
    std::os::unix::fs::symlink("A", f.join("Versions/Current")).unwrap();
    std::os::unix::fs::symlink("Versions/Current/F", f.join("F")).unwrap();
    std::os::unix::fs::symlink("Versions/Current/Resources", f.join("Resources")).unwrap();
    let (o, n) = (d.join("o/N.app"), d.join("n/N.app"));
    for p in [&o, &n] {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        clone_tree(&src, p);
    }
    for t in [
        "Contents/Helpers/H.app",
        "Contents/Frameworks/F.framework",
        "",
    ] {
        run(Command::new("codesign")
            .args(["--force", "--sign", "-"])
            .arg(o.join(t)));
    }
    let r = sign_bundle(&n, &BundleOptions::default()).unwrap();
    assert_eq!(r.nested_signed, 2);
    same_files(
        &o,
        &n,
        &[
            "Contents/_CodeSignature/CodeResources",
            "Contents/MacOS/main",
            "Contents/Helpers/H.app/Contents/MacOS/H",
            "Contents/Helpers/H.app/Contents/_CodeSignature/CodeResources",
            "Contents/Frameworks/F.framework/Versions/A/F",
            "Contents/Frameworks/F.framework/Versions/A/_CodeSignature/CodeResources",
        ],
    );
    fs::remove_dir_all(&d).unwrap();
}

/// iOS shallow app with an embedded framework and entitlements (the IPA shape).
#[test]
fn shallow_ios_bundle_matches_codesign() {
    let Some(ios) = sdk("iphoneos") else {
        eprintln!("skipping: no iOS SDK");
        return;
    };
    if !tools() {
        return;
    }
    let d = tmpdir("shallow");
    let exe = build(
        &d,
        "exe",
        &["-target", "arm64-apple-ios17.0", "-isysroot", &ios],
    );
    let lib = build(
        &d,
        "lib",
        &[
            "-target",
            "arm64-apple-ios17.0",
            "-isysroot",
            &ios,
            "-dynamiclib",
        ],
    );
    let src = d.join("src/S.app");
    write(&src.join("Info.plist"), &info_plist("S", "org.test.s"));
    write(&src.join("PkgInfo"), "APPL????");
    write(&src.join("msl/b-1/x.metallib"), "m");
    write(&src.join("en.lproj/L.strings"), "l");
    write(
        &src.join("Res.bundle/Info.plist"),
        "<plist version=\"1.0\"><dict/></plist>",
    );
    fs::copy(&exe, src.join("S")).unwrap();
    let fw = src.join("Frameworks/G.framework");
    write(&fw.join("Info.plist"), &info_plist("G", "org.test.s.G"));
    write(&fw.join("dm/t.tsv"), "t");
    fs::copy(&lib, fw.join("G")).unwrap();
    let ent = d.join("ent.plist");
    write(
        &ent,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><plist version=\"1.0\"><dict><key>com.apple.developer.kernel.increased-memory-limit</key><true/></dict></plist>",
    );
    let (o, n) = (d.join("o/S.app"), d.join("n/S.app"));
    for p in [&o, &n] {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        clone_tree(&src, p);
    }
    run(Command::new("codesign")
        .args(["--force", "--sign", "-"])
        .arg(o.join("Frameworks/G.framework")));
    run(Command::new("codesign")
        .args(["--force", "--sign", "-", "--entitlements"])
        .arg(&ent)
        .arg(&o));
    let opts = BundleOptions {
        entitlements: Some(fs::read(&ent).unwrap()),
        ..BundleOptions::default()
    };
    let r = sign_bundle(&n, &opts).unwrap();
    assert_eq!(r.nested_signed, 1);
    same_files(
        &o,
        &n,
        &[
            "_CodeSignature/CodeResources",
            "S",
            "Frameworks/G.framework/G",
            "Frameworks/G.framework/_CodeSignature/CodeResources",
        ],
    );
    fs::remove_dir_all(&d).unwrap();
}

/// appstage: staging a file set and signing it equals copying the same files and running
/// codesign, with digests given for part of the files and hashed while staging for the rest.
#[test]
fn appstage_matches_codesign() {
    use nxb_pack::appstage::{Content, StageEntry, StageOptions, StagePlan, stage_and_sign};
    if !tools() {
        return;
    }
    let d = tmpdir("appstage");
    let exe = build(&d, "exe", &[]);
    let src = make_bundle(&d.join("src"), &exe, &exe);
    let mut plan = StagePlan::default();
    let mut stack = vec![src.clone()];
    while let Some(dir) = stack.pop() {
        for e in fs::read_dir(&dir).unwrap() {
            let e = e.unwrap();
            let ft = e.file_type().unwrap();
            if ft.is_dir() {
                stack.push(e.path());
            } else if ft.is_file() {
                let dest = e
                    .path()
                    .strip_prefix(&src)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string();
                plan.entries.push(StageEntry {
                    dest,
                    content: Content::File(e.path()),
                    digests: None,
                });
            }
        }
    }
    // Digests known for half of the resource files (the build-store case).
    let known = nxb_pack::bundle::bundle_digests(&src, 2).unwrap();
    for (i, e) in plan.entries.iter_mut().enumerate() {
        if i % 2 == 0 {
            if let Some((_, _, dg)) = known
                .iter()
                .find(|(p, _, _)| format!("Contents/{p}") == e.dest)
            {
                e.digests = Some(*dg);
            }
        }
    }
    let o = d.join("o/T.app");
    for e in &plan.entries {
        let dst = o.join(&e.dest);
        fs::create_dir_all(dst.parent().unwrap()).unwrap();
        if let Content::File(p) = &e.content {
            fs::copy(p, &dst).unwrap();
        }
    }
    run(Command::new("codesign")
        .args(["--force", "--sign", "-"])
        .arg(o.join("Contents/MacOS/helper")));
    run(Command::new("codesign")
        .args(["--force", "--sign", "-"])
        .arg(&o));
    let n = d.join("n/T.app");
    let r = stage_and_sign(
        &plan,
        &n,
        &StageOptions {
            threads: 3,
            ..StageOptions::default()
        },
    )
    .unwrap();
    assert!(r.sign.reused_digests > 3);
    assert_eq!(r.sign.hashed_files, 0);
    same_files(
        &o,
        &n,
        &[
            "Contents/_CodeSignature/CodeResources",
            "Contents/MacOS/main",
            "Contents/MacOS/helper",
        ],
    );
    assert!(stage_and_sign(&plan, &n, &StageOptions::default()).is_err()); // never overwrites
    fs::remove_dir_all(&d).unwrap();
}
