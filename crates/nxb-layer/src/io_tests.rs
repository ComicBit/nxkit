#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use naga::{Binding, Interpolation};

#[test]
fn array_varyings_become_one_float_varying_per_element_with_interpolation() {
    let mut s = crate::spirv::Spv::parse(&crate::tests::spirv_of(crate::tests::FRAG, "frag")).unwrap();
    crate::spirv::split_combined(&mut s).unwrap();
    let (m, _) = crate::to_ir(&s.words(), false, "f", |_| Ok(())).unwrap();
    let args = &m.entry_points[0].function.arguments;
    let locs: Vec<(u32, Option<Interpolation>)> = args
        .iter()
        .filter_map(|a| match a.binding {
            Some(Binding::Location { location, interpolation, .. }) => Some((location, interpolation)),
            _ => None,
        })
        .collect();
    assert_eq!(locs, vec![(0, Some(Interpolation::Perspective)), (1, Some(Interpolation::Perspective))]);
}

#[test]
fn clip_z_maps_the_depth_range() {
    let words = crate::gl::compile(crate::tests::VERT, "vert").unwrap();
    let (m, info) = crate::to_ir(&words, false, "v", super::clip_z).unwrap();
    let src = crate::msl::write(&m, &info, &crate::slots::assign(&m, &Default::default()).unwrap(), "v").unwrap();
    assert!(src.contains("0.5"), "{src}");
}
