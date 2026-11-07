//! The pairs of a variation: a vertex shader or a mesh shader with its pixel shader.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use nxb_split::shaders::{BahsVar, Found, Profile, skey, var_pairs};

fn found(place: &str, stage: &'static str, tag: u8) -> Found {
    Found {
        place: place.into(),
        stage,
        ctl: vec![tag; 0x40],
        code: vec![tag; 0x40],
    }
}

fn key(tag: u8) -> String {
    skey(&Profile::default(), &[tag; 0x40], &[tag; 0x40])
}

#[test]
fn a_mesh_shader_pairs_with_the_pixel_shader_of_its_variation() {
    let found = [
        found("a.bfsha#BNSH@0x10#var0", "vs", 1),
        found("a.bfsha#BNSH@0x10#var0", "ps", 2),
        found("a.bfsha#BNSH@0x10#var1", "ms", 3),
        found("a.bfsha#BNSH@0x10#var1", "ps", 4),
        found("a.bfsha#BNSH@0x10#var2", "ms", 5),
    ];
    let pairs = var_pairs(&Profile::default(), &found, &BahsVar::default());
    let want = [(key(1), key(2), false), (key(3), key(4), false)];
    assert_eq!(pairs, want.into_iter().collect());
}
