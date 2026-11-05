//! GLSLC outputs: the fixtures are synthetic files (made by a script that also ran the reference `glslc_shaders` on
//! them to give the `.exp` rows: place, stage, SHA-256 prefixes of control section and code area, key).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use nxb_model::sha256;
use nxb_split::glslc::glslc_shaders;
use nxb_split::shaders::{Profile, skey};

fn rows(d: &[u8]) -> String {
    let p = Profile::default();
    glslc_shaders(d, "/x", false, &[])
        .unwrap()
        .iter()
        .map(|f| {
            format!(
                "{}\t{}\t{}\t{}\t{}\n",
                f.place,
                f.stage,
                &sha256(&f.ctl).to_hex()[..16],
                &sha256(&f.code).to_hex()[..16],
                skey(&p, &f.ctl, &f.code)
            )
        })
        .collect()
}

macro_rules! case {
    ($name:ident, $file:literal) => {
        #[test]
        fn $name() {
            let d = include_bytes!(concat!("data/glslc_", $file, ".bin"));
            let want = include_str!(concat!("data/glslc_", $file, ".exp"));
            assert_eq!(rows(d), want);
        }
    };
}

case!(single_vertex, "a_vs");
case!(pixel_and_headerless_compute, "b_ps_and_cs");
case!(headerless_compute, "c_cs");
case!(program_of_two_stages, "d_program");
case!(nvn2_output, "e_nvn2");
case!(decoy_magic_gives_nothing, "f_decoy");

#[test]
fn need_names_the_file() {
    let e = glslc_shaders(b"nothing", "/y", true, &[]).unwrap_err();
    assert_eq!(e.to_string(), "/y: no GLSLC output");
}
