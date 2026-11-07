//! Mesh outputs for the pixel shaders that read them.
#![cfg(test)]

use super::mesh_tests::{mesh, ms_ops, run, run_paired};
use super::{StageInputs, Varying};

/// A pixel shader that reads the components (one bit each) of the locations.
pub(super) fn reads(locations: &[(u32, u8)]) -> StageInputs {
    StageInputs {
        varyings: locations
            .iter()
            .map(|(l, read)| Varying {
                location: *l,
                msl_type: "float4".into(),
                interpolation: String::new(),
                read: *read,
            })
            .collect(),
    }
}

fn paired(partners: &[StageInputs]) -> String {
    run_paired(&mesh("ms"), &ms_ops(), |_| {}, partners)
        .expect("translate")
        .libraries[0]
        .msl
        .clone()
}

#[test]
fn a_location_the_pixel_shader_reads_and_the_mesh_does_not_write_is_padded() {
    // Location 1 is a vertex output and 2 a primitive output, so only 5 is padded.
    let msl = paired(&[reads(&[(1, 1), (2, 1), (5, 0xf)]), reads(&[(5, 0xf)])]);
    assert!(
        msl.contains("    float4 dm_pad5 [[user(locn5)]];\n};"),
        "{msl}"
    );
    assert!(
        msl.contains("        v.dm_pad5 = float4(0.0, 0.0, 0.0, 1.0);\n"),
        "{msl}"
    );
    assert_eq!(msl.matches("[[user(locn1)]]").count(), 1, "{msl}");
    assert_eq!(msl.matches("[[user(locn2)]]").count(), 1, "{msl}");
    // One member and one start in each of the two entry functions.
    assert_eq!(msl.matches("dm_pad").count(), 3, "{msl}");
}

#[test]
fn outputs_are_as_wide_as_the_pixel_shaders_read() {
    // The mesh writes x and z of location 1 and x of location 2. Location 5 is not written.
    let msl = paired(&[reads(&[(1, 1), (2, 1), (5, 0x7)])]);
    assert!(msl.contains("    float3 a1 [[user(locn1)]];"), "{msl}");
    assert!(
        msl.contains("v.a1 = float3(as_type<float>(dm_mem[b + 128u]), 0.0, as_type<float>(dm_mem[b + 160u]));"),
        "{msl}"
    );
    assert!(msl.contains("    float p2 [[user(locn2)]];"), "{msl}");
    assert!(msl.contains("    float dm_pad5 [[user(locn5)]];"), "{msl}");
    assert!(msl.contains("v.dm_pad5 = 0.0;"), "{msl}");
}

#[test]
fn a_w_that_is_read_and_not_written_is_one() {
    let msl = paired(&[reads(&[(1, 0xf), (2, 0x9), (5, 0x8)])]);
    assert!(msl.contains("    float4 a1 [[user(locn1)]];"), "{msl}");
    assert!(
        msl.contains("v.a1 = float4(as_type<float>(dm_mem[b + 128u]), 0.0, as_type<float>(dm_mem[b + 160u]), 1.0);"),
        "{msl}"
    );
    assert!(msl.contains("    float4 p2 [[user(locn2)]];"), "{msl}");
    assert!(msl.contains("    float4 dm_pad5 [[user(locn5)]];"), "{msl}");
}

#[test]
fn every_output_is_a_float4_while_the_partners_are_not_known() {
    let msl = paired(&[]);
    assert!(msl.contains("    float4 a1 [[user(locn1)]];"), "{msl}");
    assert!(msl.contains("    float4 p2 [[user(locn2)]];"), "{msl}");
}

#[test]
fn a_mesh_with_no_partners_has_no_pad() {
    let msl = run(&mesh("ms"), &ms_ops()).expect("translate").libraries[0]
        .msl
        .clone();
    assert!(!msl.contains("dm_pad"), "{msl}");
}
