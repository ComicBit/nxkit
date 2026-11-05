//! Parity on a real container (skipped unless NXB_NUP_REAL names a `.nup` and NXB_NUP_REF the engine's extraction of
//! the same version, `<work>/extract/v<version>`). Reads the package in place, writes no image (digest only) and
//! compares: every ExeFS module and the control data byte for byte, and the image's SHA-256 with the SHA-256 of the
//! engine's `RomFS.img` (streamed).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::Read;
use std::path::PathBuf;

#[test]
fn real_nup_matches_the_engine() {
    let (Some(nup), Some(reference)) = (
        std::env::var_os("NXB_NUP_REAL"),
        std::env::var_os("NXB_NUP_REF"),
    ) else {
        return;
    };
    let (nup, reference) = (PathBuf::from(nup), PathBuf::from(reference));
    let work = std::env::var_os("NXB_TEST_TMP")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("nxb-nup-real");
    let _ = std::fs::remove_dir_all(&work);
    let version = reference
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_prefix('v'))
        .and_then(|v| v.parse().ok());
    let r = nxb_extract::nup::NupRequest {
        nup,
        version,
        work: work.clone(),
        threads: 8,
        mem_limit: 2 << 30,
        discard_images: true,
        digests: None,
        skip_images: false,
    };
    let images = nxb_extract::nup::extract_nup(&r).unwrap();
    let out = work.join("extract").join(reference.file_name().unwrap());
    for e in std::fs::read_dir(reference.join("ExeFS")).unwrap() {
        let e = e.unwrap();
        assert_eq!(
            std::fs::read(e.path()).unwrap(),
            std::fs::read(out.join("ExeFS").join(e.file_name())).unwrap(),
            "{:?}",
            e.file_name()
        );
    }
    for f in ["control.nacp", "icon.jpg"] {
        assert_eq!(
            std::fs::read(reference.join(f)).ok(),
            std::fs::read(out.join(f)).ok(),
            "{f}"
        );
    }
    let mut h = nxb_model::Sha256::new();
    let mut file = std::fs::File::open(reference.join("RomFS.img")).unwrap();
    let mut buf = vec![0u8; 16 << 20];
    loop {
        let n = file.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    assert_eq!(images[0].digest.file.sha256, h.finalize(), "RomFS.img");
    let _ = std::fs::remove_dir_all(&work);
}
