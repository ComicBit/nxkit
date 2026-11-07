#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::*;

#[test]
fn finds_the_version_line() {
    let (v, r) = version_line("// c\n#version 100\nvoid main(){}").unwrap();
    assert_eq!(v, 100);
    assert_eq!(r, 5..18);
    assert!(version_line("void main(){}").is_none());
}

#[test]
fn es100_becomes_es310() {
    let out = es100(
        "precision mediump float;\nattribute vec4 p;\nvarying lowp vec2 uv;\nvoid main(){ gl_FragColor = vec4(sample); }\n",
        "frag",
    );
    assert!(out.starts_with("#version 310 es\n"), "{out}");
    assert!(!out.contains("mediump") && !out.contains("lowp"), "{out}");
    assert!(
        out.contains("in vec4 p;") && out.contains("in vec2 uv;"),
        "{out}"
    );
    assert!(
        out.contains("out vec4 gm_FragColor;") && out.contains("gm_FragColor = vec4(gm_sample)"),
        "{out}"
    );
}

#[test]
fn varying_is_an_output_of_a_vertex_shader() {
    assert!(es100("varying vec2 uv;\n", "vert").contains("out vec2 uv;"));
}

#[test]
fn texture_offset_with_a_variable_offset_becomes_texture() {
    let out = fixups("c = textureOffset(s, uv, ivec2(k, 0));").unwrap();
    assert!(
        out.contains("texture(s, (uv) + vec2(ivec2(k, 0)) / vec2(textureSize(s, 0)))"),
        "{out}"
    );
    let constant = fixups("c = textureOffset(s, uv, ivec2(1, 0));").unwrap();
    assert!(
        constant.contains("textureOffset(s, uv, ivec2(1, 0))"),
        "{constant}"
    );
}

#[test]
fn finds_bool_uniforms() {
    assert_eq!(
        bool_uniforms("uniform bool a;\nuniform highp bool b ;\nuniform float c;\n"),
        vec!["a".to_string(), "b".to_string()]
    );
}
