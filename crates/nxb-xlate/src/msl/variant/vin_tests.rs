//! Tests of the libraries `_bv` and `_bp`.

use super::vertex_tests::{entry, int_use, layer_program, metal_compile};
use super::vtest::{V, simple};
use crate::msl::{Library, Translated};
use nxb_pyjson::Value;

fn lib<'a>(t: &'a Translated, suffix: &str) -> Option<&'a Library> {
    t.libraries.iter().find(|l| l.suffix == suffix)
}

fn extra<'a>(t: &'a Translated, k: &str) -> Option<&'a Value> {
    let r = t.libraries[0].reflection.as_ref()?;
    r.extra.iter().find(|(n, _)| n == k).map(|(_, v)| v)
}

#[test]
fn vin_adds_the_shadows_of_an_integer_input() {
    let t = int_use().translate();
    let l = lib(&t, "_bv").expect("_bv");
    for want in [
        "struct main0_in_v",
        "float4 loc0_vf [[attribute(16)]];",
        "int4 loc0_vo [[attribute(17)]];",
        "vertex main0_out main0(main0_in_v dm_in [[stage_in]]",
        "main0_in in = {};",
        "in.loc0 = dm_in.loc0 | as_type<uint4>(dm_in.loc0_vf) | as_type<uint4>(dm_in.loc0_vo);",
    ] {
        assert!(l.msl.contains(want), "missing {want}\n{}", l.msl);
    }
    assert!(l.msl.contains(" main0_z("));
    assert!(!l.msl.contains("main0_gs"));
    let v = extra(&t, "vin").expect("vin");
    assert_eq!(v.get("entry_z").and_then(Value::as_str), Some("main0_z"));
    let s = v.get("shadow").expect("shadow");
    assert_eq!(
        nxb_pyjson::dumps(s),
        r#"[{"location": 0, "float": 16, "sint": 17}]"#
    );
}

#[test]
fn no_vin_without_an_integer_input() {
    let t = simple().translate();
    assert!(lib(&t, "_bv").is_none());
    assert!(extra(&t, "vin").is_none());
}

#[test]
fn no_vin_and_no_pull_for_an_input_at_attribute_16() {
    let mut b = V::new(&[(16, 2)]);
    b.ret();
    let t = b.translate();
    assert_eq!(t.libraries.len(), 1);
}

#[test]
fn pull_reads_the_inputs_from_the_layout_block() {
    let t = int_use().translate();
    let l = lib(&t, "_bp").expect("_bp");
    for want in [
        "struct dm_vlayout",
        "uint4 dm_fetch(constant dm_vlayout &L",
        "struct main0_in\n{\n    uint4 loc0;\n};",
        "constant dm_vlayout& dm_lay [[buffer(16)]]",
        "uint dm_vid [[vertex_id]]",
        "uint dm_iid [[instance_id]]",
        "uint dm_bi [[base_instance]]",
        "main0_in in;",
        "in.loc0 = as_type<uint4>(dm_fetch(dm_lay, 0u, dm_vid, dm_iid, dm_bi));",
    ] {
        assert!(l.msl.contains(want), "missing {want}\n{}", l.msl);
    }
    assert!(!entry(&l.msl, "main0").contains("[[stage_in]]"));
    assert!(!l.msl.contains("[[attribute("));
    let p = extra(&t, "pull").expect("pull");
    assert_eq!(
        nxb_pyjson::dumps(p),
        r#"{"entry": "main0", "entry_z": "main0_z", "slot": 16, "inputs": [0]}"#
    );
}

#[test]
fn pull_reuses_the_builtins_of_the_shader() {
    let t = layer_program().translate();
    let l = lib(&t, "_bp").expect("_bp");
    assert!(
        l.msl
            .contains("dm_fetch(dm_lay, 0u, dm_vid, gl_InstanceIndex, dm_bi)"),
        "{}",
        l.msl
    );
    assert!(!l.msl.contains("uint dm_iid"));
}

#[test]
fn a_float_input_is_pulled_but_has_no_shadow() {
    let t = simple().translate();
    assert!(lib(&t, "_bp").is_some());
}

/// Run by hand: `cargo test -p nxb-xlate -- --ignored metal`.
#[test]
#[ignore]
fn metal_compiles_the_input_libraries() {
    metal_compile("vin_int", &int_use().translate());
    metal_compile("vin_layer", &layer_program().translate());
    metal_compile("vin_simple", &simple().translate());
}
