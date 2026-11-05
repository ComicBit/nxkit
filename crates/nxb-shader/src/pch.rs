//! The precompiled header of the shader compiles, as `dmsh_pch.py` makes it: the two includes every translated source
//! starts with, plus the texture class instantiations that cost most of a compile's front end. It changes no output
//! (checked on [`PROBE`] whenever it is built), so it is not part of the compile record.

/// Sampled texture kinds whose instantiations the header holds.
const SAMPLED: [&str; 9] = [
    "texture2d",
    "texture2d_array",
    "texture3d",
    "texturecube",
    "texturecube_array",
    "depth2d",
    "depth2d_array",
    "depthcube",
    "depthcube_array",
];

/// Written texture kinds whose instantiations the header holds.
const WRITTEN: [&str; 3] = ["texture2d", "texture2d_array", "texture3d"];

/// The header text, byte-equal to `dmsh_pch.HEADER`.
pub fn header() -> String {
    let mut h = String::from("#include <metal_stdlib>\n#include <simd/simd.h>\n");
    let mut classes = Vec::new();
    for k in SAMPLED {
        for c in ["threadgroup", "device"] {
            classes.push(format!(
                "metal::{k}<float, metal::access::sample, metal::memory_coherence_{c}>"
            ));
        }
    }
    for k in WRITTEN {
        classes.push(format!(
            "metal::{k}<float, metal::access::write, metal::memory_coherence_threadgroup>"
        ));
    }
    for c in classes {
        h.push_str(&format!("static_assert(sizeof({c}) > 0);\n"));
    }
    h
}

/// The probe shader: compiled with and without the header, the two results must be identical.
pub const PROBE: &str = "#include <metal_stdlib>
#include <simd/simd.h>
using namespace metal;
struct set0 { texture2d<float> a [[id(0)]]; texture2d_array<float> b [[id(1)]]; texture3d<float> c [[id(2)]]; texturecube<float> d [[id(3)]]; depth2d_array<float> e [[id(4)]]; sampler s [[id(5)]]; };
struct main0_in { float4 uv [[user(locn0)]]; };
fragment float4 main0(main0_in in [[stage_in]], constant set0& t [[buffer(0)]])
{
    return t.a.sample(t.s, in.uv.xy) + t.b.sample(t.s, in.uv.xy, 0u) + t.c.sample(t.s, in.uv.xyz) + t.d.sample(t.s, in.uv.xyz) + t.e.sample(t.s, in.uv.xy, 0u);
}
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_shape() {
        let h = header();
        assert!(h.starts_with("#include <metal_stdlib>\n#include <simd/simd.h>\nstatic_assert(sizeof(metal::texture2d<float, metal::access::sample, metal::memory_coherence_threadgroup>) > 0);\n"));
        assert_eq!(h.matches("static_assert").count(), 21);
    }

    /// Byte-equal to the installed engine's `dmsh_pch.HEADER` and `PROBE` (sha256 taken from Python).
    #[test]
    fn equal_to_python() {
        assert_eq!(
            nxb_model::sha256(header().as_bytes()).to_hex(),
            "5491da27c4b37e672231ff9863a6474908119ba8ed8387d8774d9716c983ba78"
        );
        assert_eq!(
            nxb_model::sha256(PROBE.as_bytes()).to_hex(),
            "4422269fa61c23e1b8e666b98f4fce1ba39285306e45b66ab53ab61e616103f8"
        );
    }
}
