#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::*;

pub(crate) const VERT: &str = "#version 450
layout(set = 0, binding = 1) uniform U { mat4 mvp; vec4 tint[2]; } u;
layout(push_constant) uniform P { vec4 off; } p;
layout(location = 0) in vec4 pos;
layout(location = 1) in vec2 uv;
layout(location = 0) out vec2 v_uv[2];
void main() { v_uv[0] = uv; v_uv[1] = uv * 2.0; gl_Position = u.mvp * pos + p.off + u.tint[1]; }
";

pub(crate) const FRAG: &str = "#version 450
layout(set = 0, binding = 2) uniform sampler2D tex;
layout(location = 0) in vec2 v_uv[2];
layout(location = 0) out vec4 c0;
layout(location = 1) out vec4 c1;
void main() { c0 = texture(tex, v_uv[0]); c1 = vec4(min(v_uv[1].x, 0.5)); }
";

pub(crate) fn spirv_of(src: &str, stage: &str) -> Vec<u8> {
    gl::compile(src, stage).unwrap().iter().flat_map(|w| w.to_le_bytes()).collect()
}

fn shader(src: &str, stage: &str) -> Shader {
    match vk::translate(&spirv_of(src, stage)).unwrap() {
        vk::Outcome::Shader(s) => s,
        vk::Outcome::Skipped(st) => panic!("skipped {st}"),
    }
}

#[test]
fn vertex_shader_has_buffer_and_push_constant_slots() {
    let s = shader(VERT, "vert");
    assert_eq!(s.stage, "vert");
    assert_eq!(s.key.len(), 16);
    assert_eq!(s.fields, "u:0.1=0 p:0.0=1");
    assert!(s.msl.contains(&format!("vertex vk_{}", s.key)), "{}", s.msl);
    assert!(s.msl_dual.is_none());
}

#[test]
fn fragment_shader_has_combined_sampler_and_dual_source_variant() {
    let s = shader(FRAG, "frag");
    assert_eq!(s.fields, "c:0.2=0");
    assert!(s.msl.contains("[[color(1)]]"), "{}", s.msl);
    assert!(s.msl.contains("metal::precise::min"), "{}", s.msl);
    let d = s.msl_dual.expect("dual-source variant");
    assert!(d.contains("[[color(0), index(1)]]"), "{d}");
    assert!(d.contains(&format!("vk_{}_d", s.key)), "{d}");
}

#[test]
fn table_sorts_rows_and_keeps_the_first_of_a_key() {
    let mk = |key: &str, stage: &'static str| Shader {
        key: key.into(),
        stage,
        fields: format!("f{key}"),
        msl: String::new(),
        msl_dual: None,
    };
    let rows = vec![
        (mk("bbbbbbbbbbbbbbbb", "frag"), "/b".to_string()),
        (mk("aaaaaaaaaaaaaaaa", "vert"), "/a".to_string()),
        (mk("bbbbbbbbbbbbbbbb", "frag"), "/dup".to_string()),
    ];
    let t = table("# head", "tool 1", &rows);
    assert_eq!(t, "# head\n# tool 1\naaaaaaaaaaaaaaaa\tvert\tfaaaaaaaaaaaaaaaa\t/a\nbbbbbbbbbbbbbbbb\tfrag\tfbbbbbbbbbbbbbbbb\t/b\n");
}
