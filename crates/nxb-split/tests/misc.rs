//! Needle, TFXO (also as a zlib stream in an image) and gzip files against the reference (`.exp` files, made by a
//! script that ran the reference functions on the synthetic `.bin` files).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use nxb_model::sha256;
use nxb_split::shaders::{BahsVar, Found, scan_blob};

fn rows(found: &[Found]) -> String {
    found
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
fn needle_container() {
    let d = include_bytes!("data/misc_needle.bin");
    let mut f = Vec::new();
    scan_blob(d, "/x", &mut f, &mut BahsVar::new(), 0).unwrap();
    assert_eq!(rows(&f), include_str!("data/misc_needle.exp"));
}

#[test]
fn tfxo_file() {
    let d = include_bytes!("data/misc_tfxo.bin");
    let mut f = Vec::new();
    scan_blob(d, "/x", &mut f, &mut BahsVar::new(), 0).unwrap();
    assert_eq!(rows(&f), include_str!("data/misc_tfxo.exp"));
}

#[test]
fn tfxo_zlib_stream_in_an_image() {
    let d = include_bytes!("data/misc_tfxostream.bin");
    let f = nxb_split::misc::tfxo_streams(d, "main").unwrap();
    assert_eq!(rows(&f), include_str!("data/misc_tfxostream.exp"));
}

#[test]
fn gzip_members_and_damage() {
    let d = include_bytes!("data/misc_gz.bin");
    let r = nxb_split::misc::ungzip(d, "/x").unwrap();
    let want = format!("ok\t{}\t{}\n", r.len(), &sha256(&r).to_hex()[..16]);
    assert_eq!(want, include_str!("data/misc_gz.exp"));
    let e = nxb_split::misc::ungzip(include_bytes!("data/misc_gzbad.bin"), "/x").unwrap_err();
    let text = format!("[RED] {e}");
    assert!(
        text.starts_with(
            include_str!("data/misc_gzbad.exp")
                .split(" (")
                .next()
                .unwrap()
        ),
        "{text}"
    );
}

#[test]
fn main_with_a_stray_control_section_needs_a_module_header() {
    let mut img = vec![0x55u8; 0x2000];
    img[0x100..0x104].copy_from_slice(&[0x34, 0x12, 0x76, 0x98]);
    img[0x120..0x124].copy_from_slice(&0x850u32.to_le_bytes());
    // the reference reads the relocations of the module next (struct.error on this image: no module header)
    assert!(nxb_split::misc::main_extras(&img, "main", &[]).is_err());
    assert!(
        nxb_split::misc::main_extras(&[0x55u8; 0x400], "main", &[])
            .unwrap()
            .is_empty()
    );
}

/// Real `main` (env `NXB_MAIN_NSO` names an NSO): the extra scans find nothing and refuse nothing in the reference title's.
#[test]
fn real_main_needs_no_extras() {
    let Some(p) = std::env::var_os("NXB_MAIN_NSO") else {
        return;
    };
    let img = nxb_split::nso::image(&std::fs::read(p).unwrap()).unwrap();
    let mut found = Vec::new();
    scan_blob(&img, "main", &mut found, &mut BahsVar::new(), 0).unwrap();
    let extra = nxb_split::misc::main_extras(&img, "main", &found).unwrap();
    assert!(extra.is_empty());
}
