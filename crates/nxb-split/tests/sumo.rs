//! Sumo's containers and Phyre's effects against the reference (`.exp`: place, stage, SHA-256 prefixes).
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

macro_rules! blob {
    ($name:ident, $file:literal) => {
        #[test]
        fn $name() {
            let d = include_bytes!(concat!("data/sumo_", $file, ".bin"));
            let mut f = Vec::new();
            scan_blob(d, "/x", &mut f, &mut BahsVar::new(), 0).unwrap();
            assert_eq!(rows(&f), include_str!(concat!("data/sumo_", $file, ".exp")));
        }
    };
}

blob!(nvnsh_records, "nvnsh");
blob!(ninty_nx_descriptor, "ninty");
blob!(mux_container_with_two_streams, "mux");

#[test]
fn phyre_code_before_control_section() {
    let d = include_bytes!("data/sumo_phyre.bin");
    let f = nxb_split::sumo::phyre_shaders(d, "/x");
    assert_eq!(rows(&f), include_str!("data/sumo_phyre.exp"));
}
