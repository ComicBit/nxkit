//! Unreal Engine 3 shader caches and packages with zlib chunks against the reference.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use nxb_model::sha256;
use nxb_split::ue3;

fn rows(d: &[u8]) -> String {
    let body = if d.starts_with(&[0xc1, 0x83, 0x2a, 0x9e]) {
        ue3::body(d, "/x").unwrap()
    } else {
        d.to_vec()
    };
    ue3::shaders(&body, "/x")
        .unwrap()
        .iter()
        .map(|f| {
            format!(
                "{}\t{}\t{}\t{}\n",
                f.place,
                f.stage,
                &sha256(&f.ctl).to_hex()[..16],
                &sha256(&f.code).to_hex()[..16]
            )
        })
        .collect()
}

#[test]
fn global_cache() {
    let d = include_bytes!("data/ue3_cache.bin");
    assert!(ue3::is_ue3(d));
    assert_eq!(rows(d), include_str!("data/ue3_cache.exp"));
}

#[test]
fn package_with_zlib_and_stored_blocks() {
    let d = include_bytes!("data/ue3_package.bin");
    assert!(ue3::is_ue3(d));
    assert_eq!(rows(d), include_str!("data/ue3_package.exp"));
}

#[test]
fn oodle_blocks_are_unsupported() {
    let mut d = include_bytes!("data/ue3_package.bin").to_vec();
    // the first block's zlib header byte becomes something else
    let at = d.windows(2).position(|w| w == [0x78, 0x9c]).unwrap();
    d[at] = 0x8c;
    assert!(matches!(
        ue3::body(&d, "/x"),
        Err(nxb_split::SplitError::Unsupported(_))
    ));
}
