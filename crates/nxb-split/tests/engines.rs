//! Small engine formats through `scan_blob`: synthetic files, and the rows (place, stage, SHA-256 prefixes of control
//! section and code area, then the program variations) the reference `scan_blob` gives for them.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use nxb_model::sha256;
use nxb_split::shaders::{BahsVar, scan_blob};

fn rows(d: &[u8]) -> String {
    let (mut found, mut var) = (Vec::new(), BahsVar::new());
    scan_blob(d, "/x", &mut found, &mut var, 0).unwrap();
    let mut s: String = found
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
        .collect();
    for (k, v) in &var {
        s.push_str(&format!("var\t{k}\t{v}\n"));
    }
    s
}

macro_rules! case {
    ($name:ident, $file:literal) => {
        #[test]
        fn $name() {
            let d = include_bytes!(concat!("data/eng_", $file, ".bin"));
            let want = include_str!(concat!("data/eng_", $file, ".exp"));
            assert_eq!(rows(d), want);
        }
    };
}

case!(mshd_pairs, "mshd");
case!(sead_primitive_drawer, "seadprim");
case!(ndsr_pack_and_its_variation, "ndsr");
case!(platinum_shader_object, "pgso");
case!(platinum_binding_table_form, "pgsized");
case!(platinum_dat_archive, "pgdat");

#[test]
fn monolith_heads_are_read_now() {
    // a head with no known version holds nothing (as the reference), it is no longer an unsupported container
    let mut out = Vec::new();
    scan_blob(&b"HCPS...."[..], "/x", &mut out, &mut BahsVar::new(), 0).unwrap();
    assert!(out.is_empty());
}
