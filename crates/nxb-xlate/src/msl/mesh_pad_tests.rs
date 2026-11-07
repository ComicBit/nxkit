//! Mesh outputs for the pixel shaders that read them.
#![cfg(test)]

use super::mesh_tests::{mesh, ms_ops, run, run_paired};
use super::{StageInputs, Varying};

fn reads(locations: &[u32]) -> StageInputs {
    StageInputs {
        varyings: locations
            .iter()
            .map(|l| Varying {
                location: *l,
                msl_type: "float4".into(),
                interpolation: String::new(),
            })
            .collect(),
    }
}

#[test]
fn a_location_the_pixel_shader_reads_and_the_mesh_does_not_write_is_padded() {
    // Location 1 is a vertex output and 2 a primitive output, so only 5 is padded.
    let msl = run_paired(
        &mesh("ms"),
        &ms_ops(),
        |_| {},
        &[reads(&[1, 2, 5]), reads(&[5])],
    )
    .expect("translate")
    .libraries[0]
        .msl
        .clone();
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
fn a_mesh_with_no_partners_has_no_pad() {
    let msl = run(&mesh("ms"), &ms_ops()).expect("translate").libraries[0]
        .msl
        .clone();
    assert!(!msl.contains("dm_pad"), "{msl}");
}
