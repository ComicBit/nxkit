//! Tests of the JIT capture's parts that run without a game: the stamp, the profile keys, the folders and `uses_jit`.

use super::*;
use crate::events::Mode;
use crate::ios::oracle::scratch;

fn game(d: &Path, tsv: &str) -> Game {
    let g = d.join("0100000000001000");
    std::fs::create_dir_all(&g).unwrap();
    std::fs::write(g.join("game.tsv"), tsv).unwrap();
    Game::open(&g).unwrap()
}

fn quiet() -> Progress {
    Progress::new(Mode::Off, Box::new(std::io::sink()))
}

#[test]
fn no_main_uses_no_jit() {
    let d = scratch("jit-exefs");
    assert!(!uses_jit(&d.join("ExeFS")).unwrap());
    std::fs::create_dir_all(d.join("ExeFS")).unwrap();
    assert!(!uses_jit(&d.join("ExeFS")).unwrap());
    std::fs::write(d.join("ExeFS/main"), b"not an NSO").unwrap();
    assert!(uses_jit(&d.join("ExeFS")).is_err());
}

#[test]
fn an_import_of_create_jit_environment_uses_jit() {
    let d = scratch("jit-import");
    let main = d.join("ExeFS/main");
    std::fs::create_dir_all(d.join("ExeFS")).unwrap();
    let sym = "_ZN2nn3jit20CreateJitEnvironmentEPNS0_14JitEnvironmentERKNS0_16JitConfigurationE";
    for (name, defined, want) in [
        (sym, false, true),
        (sym, true, false),
        (
            "_ZN2nn3jit18DestroyJitEnvironmentEPNS0_14JitEnvironmentE",
            false,
            false,
        ),
    ] {
        std::fs::write(&main, test_main(name, defined)).unwrap();
        assert_eq!(
            uses_jit(&d.join("ExeFS")).unwrap(),
            want,
            "{name} {defined}"
        );
    }
}

/// `json.dumps` of the script's stamp (ensure_ascii, hidden modules included), from Python 3.14.
#[test]
fn the_stamp_is_the_scripts() {
    let d = scratch("jit-stamp");
    let p = d.join("patched");
    std::fs::create_dir_all(&p).unwrap();
    for (n, s) in [
        ("b.patched.bin", 3),
        (".h.patched.bin", 1),
        ("a.patched.bin", 2),
        ("x.bin", 9),
    ] {
        std::fs::write(p.join(n), vec![0u8; s]).unwrap();
    }
    let g = game(
        &d,
        "jit_key_data\t4:0-7,24-27\njit_route\t5:A \u{e9}\u{1f600}\n",
    );
    assert_eq!(
        stamp(&p, &g),
        r#"{"modules": [[".h.patched.bin", 1], ["a.patched.bin", 2], ["b.patched.bin", 3]], "jit_key_data": "4:0-7,24-27", "jit_out_code": "", "jit_route": "5:A \u00e9\ud83d\ude00"}"#
    );
    assert!(stamp(&d.join("none"), &g).starts_with(r#"{"modules": [], "#));
}

#[test]
fn a_stale_stamp_clears_the_captures() {
    let d = scratch("jit-clear");
    let caps = d.join("jit_capture");
    let p = quiet();
    clear_stale(&caps, "one", &p).unwrap();
    assert_eq!(
        std::fs::read_to_string(caps.join("stamp.json")).unwrap(),
        "one"
    );
    std::fs::create_dir_all(caps.join("play")).unwrap();
    std::fs::write(caps.join("play/a.jit"), b"x").unwrap();
    clear_stale(&caps, "one", &p).unwrap();
    assert!(caps.join("play/a.jit").is_file());
    clear_stale(&caps, "two", &p).unwrap();
    assert!(!caps.join("play").exists());
    assert_eq!(
        std::fs::read_to_string(caps.join("stamp.json")).unwrap(),
        "two"
    );
    std::fs::create_dir_all(caps.join("boot1")).unwrap();
    std::fs::remove_file(caps.join("stamp.json")).unwrap();
    clear_stale(&caps, "two", &p).unwrap();
    assert!(!caps.join("boot1").exists());
}

#[test]
fn the_profile_keys() {
    let d = scratch("jit-keys");
    let g = game(
        &d,
        "jit_key_data\t4:0-7,24-27 3:0x1f\njit_out_code\t4:entries=4,count=0x20004,size=1,size=0x20000\n",
    );
    let k = keys(&g).unwrap();
    let mut m4 = [0u8; 32];
    m4[..8].fill(0xff);
    m4[24..28].fill(0xff);
    let mut m3 = [0u8; 32];
    m3[31] = 0xff;
    assert_eq!(
        k.masks,
        std::collections::BTreeMap::from([(3, m3), (4, m4)])
    );
    assert_eq!(
        k.out_codes,
        std::collections::BTreeMap::from([(4, (4, 0x20004, 0x20000))])
    );
    assert_eq!(
        keys(&game(&d.join("none"), "name\tx\n")).unwrap(),
        Keys::default()
    );
    for bad in [
        "jit_key_data\t4:0-32\n",
        "jit_key_data\t4\n",
        "jit_key_data\t4:1:2\n",
        "jit_key_data\tx:1\n",
        "jit_out_code\t4:entries=4,count=1\n",
        "jit_out_code\t4:entries\n",
    ] {
        assert!(keys(&game(&d.join("bad"), bad)).is_err(), "{bad}");
    }
}

#[test]
fn the_capture_folders_and_an_empty_image() {
    let d = scratch("jit-image");
    let caps = d.join("jit_capture");
    for x in ["boot2", "boot1", "boot1.partial", ".hidden", "play"] {
        std::fs::create_dir_all(caps.join(x)).unwrap();
    }
    std::fs::write(caps.join("stamp.json"), "{}").unwrap();
    let dirs = capture_dirs(&caps);
    let names: Vec<String> = dirs
        .iter()
        .map(|x| x.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, [".hidden", "boot1", "boot2", "play"]);
    let log = d.join("logs/jit_capture.log");
    let e = image(
        &dirs,
        &Keys::default(),
        &d.join("jit.partial"),
        &d.join("jit"),
        &log,
    )
    .unwrap_err();
    assert!(e.contains("no captures"), "{e}");
    assert_eq!(
        std::fs::read_to_string(&log).unwrap(),
        "[RED] no captures\n"
    );
    assert!(!d.join("jit").exists());
    std::fs::write(caps.join("boot1/a.jit"), b"DMJ4").unwrap();
    let e = image(
        &dirs,
        &Keys::default(),
        &d.join("jit.partial"),
        &d.join("jit"),
        &log,
    )
    .unwrap_err();
    assert!(e.contains("a.jit: not a capture of this format"), "{e}");
}
