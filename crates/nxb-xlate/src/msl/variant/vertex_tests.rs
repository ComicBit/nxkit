//! End to end tests of the vertex variants on small vertex programs.

use super::vtest::{partner, simple};

#[test]
fn pad_writes_the_outputs_the_pixel_shader_reads() {
    let v = simple();
    let t = v.translate_with(&[partner(&[(1, "float4", ""), (4, "float4", "flat")])]);
    let lib = &t.libraries[0];
    assert!(lib.msl.contains("float4 dm_pad4 [[user(locn4, flat)]];"));
    assert!(!lib.msl.contains("dm_pad1"));
    assert!(
        lib.msl
            .contains("    out.dm_pad4 = float4(0.0, 0.0, 0.0, 1.0);")
    );
    let r = lib.reflection.as_ref().expect("reflection");
    assert_eq!(r.vs_pad, vec![4]);
}

#[test]
fn no_pad_without_partners() {
    let t = simple().translate();
    let r = t.libraries[0].reflection.as_ref().expect("reflection");
    assert!(r.vs_pad.is_empty());
    assert!(!t.libraries[0].msl.contains("dm_pad"));
}
