//! `pack_ipa.py` against `nxb_pack::ipa`: the same bytes, the same index, and the reuse of an IPA the other side
//! wrote. Oracle: `NXB_ORACLE_ENGINE` (the engine folder) and `NXB_ORACLE_PYTHON`.

#![allow(clippy::unwrap_used)]
use std::path::{Path, PathBuf};
use std::process::Command;

fn tree(root: &Path) -> Vec<(String, PathBuf)> {
    let d = root.join("src");
    std::fs::create_dir_all(d.join("sub/deep")).unwrap();
    let text: Vec<u8> = (0..300_000u32).map(|i| (i % 7) as u8 + b'a').collect();
    std::fs::write(d.join("a.txt"), &text).unwrap();
    std::fs::write(
        d.join("sub/b.bin"),
        (0..200_000u32)
            .map(|i| i.wrapping_mul(2654435761u32).wrapping_shr(13) as u8)
            .collect::<Vec<u8>>(),
    )
    .unwrap();
    std::fs::write(d.join("sub/deep/c.dat"), &text[..5000]).unwrap();
    std::fs::write(d.join("sub/deep/ü.txt"), b"unicode name").unwrap();
    std::fs::write(d.join("sub/empty"), b"").unwrap();
    std::os::unix::fs::symlink("a.txt", d.join("link")).unwrap();
    let app = root.join("app");
    std::fs::create_dir_all(&app).unwrap();
    std::fs::write(app.join("exe"), vec![7u8; 100_000]).unwrap();
    vec![("Payload/data".into(), d), ("Payload/App.app".into(), app)]
}

fn py(eng: &Path, python: &Path, out: &Path, maps: &[(String, PathBuf)]) -> String {
    let mut c = Command::new(python);
    c.arg("-I")
        .arg(eng.join("dm/ios/tools/pack_ipa.py"))
        .arg(out)
        .args(["--store", ".dat"]);
    for (a, s) in maps {
        c.arg(format!("{a}={}", s.display()));
    }
    let o = c.output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn rs(out: &Path, maps: &[(String, PathBuf)]) -> nxb_pack::ipa::Report {
    let o = nxb_pack::ipa::PackOptions {
        store: vec![".dat".into()],
        skip: Default::default(),
        skip_suffix: vec![],
        compress: nxb_pack::ipa::ZLIB,
        reuse: true,
    };
    nxb_pack::ipa::pack_report(out, maps, &o).unwrap()
}

fn idx_without_ipa(p: &Path) -> String {
    let t = std::fs::read_to_string(format!("{}.idx", p.display())).unwrap();
    let a = t.find("\"ipa\"").unwrap();
    let b = t[a..].find("], ").unwrap() + a;
    format!("{}{}", &t[..a], &t[b..])
}

#[test]
fn bytes_index_and_reuse_match_the_script() {
    let (Some(eng), Some(python)) = (
        std::env::var_os("NXB_ORACLE_ENGINE"),
        std::env::var_os("NXB_ORACLE_PYTHON"),
    ) else {
        return;
    };
    let (eng, python) = (PathBuf::from(eng), PathBuf::from(python));
    let root = std::env::temp_dir().join(format!("nxb-ipa-reuse-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let maps = tree(&root);
    let (a, b) = (root.join("a.ipa"), root.join("b.ipa"));
    py(&eng, &python, &a, &maps);
    let r = rs(&b, &maps);
    assert_eq!(r.kept, 0);
    assert_eq!(
        std::fs::read(&a).unwrap(),
        std::fs::read(&b).unwrap(),
        "full pack"
    );
    assert_eq!(idx_without_ipa(&a), idx_without_ipa(&b), "index");
    // unchanged: the script keeps everything of the IPA the Rust side wrote, and the other way round
    let out = py(&eng, &python, &b, &maps);
    assert!(
        out.contains("keeping ") && !out.contains("keeping 0 bytes"),
        "{out}"
    );
    let r = rs(&a, &maps);
    assert!(r.kept > 0, "{r:?}");
    assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    // one file changed in the middle: both sides repack to the same bytes, the Rust side keeps a prefix
    std::thread::sleep(std::time::Duration::from_millis(50));
    std::fs::write(
        root.join("src/sub/b.bin"),
        (0..210_000u32)
            .map(|i| (i % 251) as u8)
            .collect::<Vec<u8>>(),
    )
    .unwrap();
    let r = rs(&a, &maps);
    assert!(r.kept > 0 && r.reused > 0, "{r:?}");
    py(&eng, &python, &b, &maps);
    assert_eq!(
        std::fs::read(&a).unwrap(),
        std::fs::read(&b).unwrap(),
        "after a change"
    );
    let c = root.join("c.ipa");
    let r = rs(&c, &maps);
    assert_eq!(r.kept, 0);
    assert_eq!(
        std::fs::read(&a).unwrap(),
        std::fs::read(&c).unwrap(),
        "a fresh pack"
    );
    let _ = std::fs::remove_dir_all(&root);
}
