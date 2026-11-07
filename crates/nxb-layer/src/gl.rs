//! OpenGL games: GLSL sources to MSL functions `gl_<key>_v` and `gl_<key>_f` and rows of `gl_shaders.tsv`, the contract of
//! the reference's `dm/gfx/tools/gl_shaders.py`.
//!
//! A source goes through `crate::glsl_fix::es100` (GLSL ES 1.00 only), `crate::glsl_pp::preprocess` and
//! `crate::glsl_fix::fixups`. glslang compiles it to SPIR-V under relaxed Vulkan rules, which gather the loose uniforms into
//! one block, `gl_DefaultUniformBlock`. Then the locations and bindings are numbered, the combined image samplers split and
//! naga writes the MSL. The Metal slots are the bindings. A vertex shader's clip-space depth goes from OpenGL's −w to w to
//! Metal's 0 to w. Nothing is flipped, the host renders in OpenGL's orientation. Varyings are matched by name, as OpenGL
//! links them: `[[user(locN)]]` becomes `[[user(<name>)]]`.

use std::collections::BTreeSet;
use std::sync::Mutex;

use glslang::{
    Compiler, CompilerOptions, ShaderInput, ShaderMessage, ShaderOptions, ShaderSource, ShaderStage, SourceLanguage,
    SpirvVersion, Target, VulkanVersion,
};
use naga::{AddressSpace, ArraySize, Module, ScalarKind, TypeInner};

use crate::spirv::{
    self, DEC_BINDING, DEC_BUILT_IN, DEC_LOCATION, OP_ENTRY_POINT, OP_TYPE_FLOAT, OP_TYPE_IMAGE, OP_TYPE_INT, OP_TYPE_MATRIX,
    OP_TYPE_SAMPLED_IMAGE, OP_TYPE_VECTOR, OP_VARIABLE, SC_INPUT, SC_OUTPUT, SC_UNIFORM_CONSTANT, Spv,
};
use crate::{Shader, glsl_fix, glsl_pp, io, msl, slots};

/// The glslang crate and the glslang source it builds.
pub const GLSLANG: &str = "0.9.1 (glslang-sys 0.9.2+e1b562a)";

/// The first comment line of `gl_shaders.tsv`.
pub const TABLE_HEAD: &str = "# GL shaders (dm/gfx/tools/gl_shaders.py): key = sha256(GLSL source)[:16], Metal function gl_<key>_v / gl_<key>_f; a: vertex inputs, u: default uniform block members (offset:type:count:stride), b: the block (slot:size), t: combined samplers (slot:type), o: fragment outputs";

/// The types a uniform member may have.
const VEC_TYPES: &[&str] =
    &["float", "vec2", "vec3", "vec4", "int", "ivec2", "ivec3", "ivec4", "uint", "uvec2", "uvec3", "uvec4", "bool", "mat2", "mat3", "mat4"];

/// glslang runs one compile at a time. Its process-wide state is set up once, and the crate does not promise more.
static GLSLANG_LOCK: Mutex<()> = Mutex::new(());

/// A source as the game passes it to OpenGL: the bytes up to the first NUL.
pub fn text_of(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

/// The table key of a source: the first 16 hex digits of the SHA-256 of its text, which the host hashes as well.
pub fn key_of(src: &[u8]) -> String {
    crate::vk::key_of(text_of(src))
}

/// The stage of an MVGL archive's shader file, `_vp.shad` vertex and `_fp.shad` fragment, or None.
pub fn mvgl_stage(name: &str) -> Option<&'static str> {
    if name.ends_with("_vp.shad") {
        Some("vert")
    } else if name.ends_with("_fp.shad") {
        Some("frag")
    } else {
        None
    }
}

fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| char::from(c)).collect()
}

/// The text glslang reads: ES 1.00 rewritten, preprocessed and fixed. Also the names of the `bool` uniforms.
fn prepare(src: &[u8], stage: &str) -> Result<(String, Vec<String>), String> {
    let t = latin1(text_of(src));
    let t = match glsl_fix::version_line(&t) {
        Some((v, _)) if v != 100 => t,
        Some((_, r)) => glsl_fix::es100(&format!("{}{}", &t[..r.start], &t[r.end..]), stage),
        None => glsl_fix::es100(&t, stage),
    };
    let pre = glsl_pp::preprocess(&t).map_err(|e| format!("preprocessor: {e}"))?;
    let bools = glsl_fix::bool_uniforms(&pre);
    Ok((glsl_fix::fixups(&pre)?, bools))
}

pub(crate) fn compile(text: &str, stage: &str) -> Result<Vec<u32>, String> {
    let st = if stage == "vert" { ShaderStage::Vertex } else { ShaderStage::Fragment };
    let compiler = Compiler::acquire().ok_or("glslang does not start")?;
    let source = ShaderSource::from(text.to_string());
    let opts = CompilerOptions {
        source_language: SourceLanguage::GLSL,
        target: Target::Vulkan { version: VulkanVersion::Vulkan1_0, spirv_version: SpirvVersion::SPIRV1_0 },
        version_profile: None,
        messages: ShaderMessage::DEFAULT | ShaderMessage::VULKAN_RULES | ShaderMessage::SPV_RULES,
    };
    let _guard = GLSLANG_LOCK.lock().map_err(|_| "glslang lock")?;
    let input = ShaderInput::new(&source, st, &opts, None::<&[(&str, Option<&str>)]>, None).map_err(|e| format!("glslang: {e}"))?;
    let mut b = glslang::Shader::new(compiler, input);
    b.options(ShaderOptions::VULKAN_RULES_RELAXED);
    let shader = b.parse().map_err(|e| format!("glslang: {}", errors(&e.to_string())))?;
    shader.compile().map_err(|e| format!("glslang: {}", errors(&e.to_string())))
}

/// The `ERROR` lines of a glslang log, at most 400 characters.
fn errors(log: &str) -> String {
    let e: Vec<&str> = log.lines().filter(|l| l.contains("ERROR") && !l.contains("compilation")).collect();
    let s = if e.is_empty() { log.trim().to_string() } else { e.join(" | ") };
    s.chars().take(400).collect()
}

/// The GLSL name of a SPIR-V scalar, vector or matrix type.
fn type_name(s: &Spv, ty: u32) -> String {
    let Some(d) = s.def(ty) else { return "?".into() };
    let scalar = |d: &spirv::Inst| match d.op {
        OP_TYPE_FLOAT => "",
        OP_TYPE_INT if d.w.get(2) == Some(&1) => "i",
        OP_TYPE_INT => "u",
        _ => "b",
    };
    match d.op {
        OP_TYPE_FLOAT => "float".into(),
        OP_TYPE_INT if d.w.get(2) == Some(&1) => "int".into(),
        OP_TYPE_INT => "uint".into(),
        OP_TYPE_VECTOR => {
            let c = s.def(d.w[1]).map_or("", scalar);
            format!("{c}vec{}", d.w[2])
        }
        OP_TYPE_MATRIX => {
            let rows = s.def(d.w[1]).and_then(|v| v.w.get(2)).copied().unwrap_or(0);
            if rows == d.w[2] { format!("mat{rows}") } else { format!("mat{}x{rows}", d.w[2]) }
        }
        20 => "bool".into(),
        _ => "?".into(),
    }
}

/// The GLSL name of a combined image sampler type.
fn sampler_name(s: &Spv, sampled: u32) -> String {
    let Some(img) = s.def(sampled).and_then(|d| d.w.get(1)).and_then(|&i| s.def(i)).filter(|d| d.op == OP_TYPE_IMAGE) else {
        return "?".into();
    };
    let (dim, depth, arrayed, ms) = (img.w[2], img.w[3], img.w[4], img.w[5]);
    let prefix = match s.def(img.w[1]) {
        Some(d) if d.op == OP_TYPE_INT && d.w.get(2) == Some(&1) => "i",
        Some(d) if d.op == OP_TYPE_INT => "u",
        _ => "",
    };
    let base = match dim {
        0 => "1D",
        1 => "2D",
        2 => "3D",
        3 => "Cube",
        4 => "2DRect",
        5 => "Buffer",
        _ => "?",
    };
    let ms = if ms == 1 { "MS" } else { "" };
    let arr = if arrayed == 1 { "Array" } else { "" };
    let shadow = if depth == 1 { "Shadow" } else { "" };
    format!("{prefix}sampler{base}{ms}{arr}{shadow}")
}

/// The entry point's input or output variables without a built-in: (name, location, type).
fn interface(s: &Spv, class: u32) -> Vec<(String, u32, u32)> {
    let listed: BTreeSet<u32> =
        s.insts.iter().filter(|i| i.op == OP_ENTRY_POINT).flat_map(|i| i.w.iter().copied()).collect();
    let mut v: Vec<(String, u32, u32)> = s
        .insts
        .iter()
        .filter(|i| i.op == OP_VARIABLE && i.w.get(2) == Some(&class) && listed.contains(&i.w[1]))
        .filter(|i| s.decoration(i.w[1], DEC_BUILT_IN).is_none())
        .filter_map(|i| {
            let ty = s.def(i.w[0]).and_then(|p| p.w.get(2)).copied()?;
            Some((s.name(i.w[1]), s.decoration(i.w[1], DEC_LOCATION)?, ty))
        })
        .collect();
    v.sort_by_key(|x| x.1);
    v
}

/// The default uniform block members: `u:name=offset:type:count:stride`, structs by their dotted member names.
fn members(m: &Module, ty: naga::Handle<naga::Type>, prefix: &str, base: u32, bools: &[String], f: &mut Vec<String>) -> Result<(), String> {
    let TypeInner::Struct { members: ref list, .. } = m.types[ty].inner else { return Err("the default block is not a struct".into()) };
    for mb in list {
        let own = mb.name.clone().unwrap_or_default();
        let name = format!("{prefix}{own}");
        let (inner, count, stride) = match m.types[mb.ty].inner {
            TypeInner::Array { base: b, size, stride } => {
                let n = match size {
                    ArraySize::Constant(n) => n.get(),
                    _ => return Err(format!("uniform {name}: an array without a size")),
                };
                if matches!(m.types[b].inner, TypeInner::Array { .. }) {
                    return Err(format!("uniform {name}: arrays of arrays"));
                }
                (&m.types[b].inner, n, stride)
            }
            ref other => (other, 0, 0),
        };
        if let TypeInner::Struct { .. } = inner {
            if count > 0 {
                return Err(format!("uniform {name}: an array of structs is not implemented"));
            }
            members(m, mb.ty, &format!("{name}."), base + mb.offset, bools, f)?;
            continue;
        }
        let mut t = glsl_of(inner);
        if t == "uint" && bools.contains(&own) {
            t = "bool".into();
        }
        if !VEC_TYPES.contains(&t.as_str()) {
            return Err(format!("uniform {name}: type {t} (structs are not implemented)"));
        }
        f.push(format!("u:{name}={}:{t}:{count}:{stride}", base + mb.offset));
    }
    Ok(())
}

fn glsl_of(t: &TypeInner) -> String {
    let p = |k: ScalarKind| match k {
        ScalarKind::Sint => "i",
        ScalarKind::Uint => "u",
        ScalarKind::Bool => "b",
        _ => "",
    };
    match *t {
        TypeInner::Scalar(s) => match s.kind {
            ScalarKind::Float if s.width == 4 => "float".into(),
            ScalarKind::Sint => "int".into(),
            ScalarKind::Uint => "uint".into(),
            ScalarKind::Bool => "bool".into(),
            _ => format!("{:?}{}", s.kind, s.width * 8),
        },
        TypeInner::Vector { size, scalar } => format!("{}vec{}", p(scalar.kind), size as u8),
        TypeInner::Matrix { columns, rows, .. } if columns as u8 == rows as u8 => format!("mat{}", columns as u8),
        TypeInner::Matrix { columns, rows, .. } => format!("mat{}x{}", columns as u8, rows as u8),
        ref other => format!("{other:?}").split([' ', '{']).next().unwrap_or("?").to_string(),
    }
}

/// Translates one GLSL source as `stage` (`vert` or `frag`).
pub fn translate(src: &[u8], stage: &'static str) -> Result<Shader, String> {
    let key = key_of(src);
    let (text, bools) = prepare(src, stage)?;
    let words = compile(&text, stage)?;
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let mut s = Spv::parse(&bytes)?;
    spirv::auto_map(&mut s);
    let mut f = Vec::new();
    if stage == "vert" {
        for (n, l, ty) in interface(&s, SC_INPUT) {
            f.push(format!("a:{n}={l}:{}", type_name(&s, ty)));
        }
    } else {
        for (n, l, _) in interface(&s, SC_OUTPUT) {
            f.push(format!("o:{n}={l}"));
        }
    }
    let names = spirv::varying_names(&s, if stage == "vert" { SC_OUTPUT } else { SC_INPUT });
    let samplers: Vec<(u32, String, u32, String)> = s
        .insts
        .iter()
        .filter(|i| i.op == OP_VARIABLE && i.w.get(2) == Some(&SC_UNIFORM_CONSTANT))
        .filter_map(|i| {
            let ty = s.def(i.w[0]).and_then(|p| p.w.get(2)).copied()?;
            let d = s.def(ty)?;
            match d.op {
                OP_TYPE_SAMPLED_IMAGE => {
                    Some((i.w[1], s.name(i.w[1]), s.decoration(i.w[1], DEC_BINDING).unwrap_or(0), sampler_name(&s, ty)))
                }
                _ => None,
            }
        })
        .collect();
    spirv::split_combined(&mut s)?;
    let combined: BTreeSet<(u32, u32)> = samplers.iter().map(|x| (0, x.2)).collect();
    let name = format!("gl_{key}_{}", &stage[..1]);
    let is_vert = stage == "vert";
    let (m, info) = crate::to_ir(&s.words(), false, &name, |m| if is_vert { io::clip_z(m) } else { Ok(()) })?;
    for (_, g) in m.global_variables.iter() {
        match g.space {
            AddressSpace::Uniform => {
                let tn = m.types[g.ty].name.clone().unwrap_or_default();
                if tn != "gl_DefaultUniformBlock" {
                    return Err(format!("uniform block {tn} is not the default block"));
                }
                let b = g.binding.map_or(0, |b| b.binding);
                let size = match m.types[g.ty].inner {
                    TypeInner::Struct { span, .. } => span,
                    _ => 0,
                };
                f.push(format!("b:{b}:{size}"));
                members(&m, g.ty, "", 0, &bools, &mut f)?;
            }
            AddressSpace::Storage { .. } => return Err("ssbos are not implemented".into()),
            AddressSpace::Immediate => return Err("push_constants are not implemented".into()),
            AddressSpace::Handle if !combined.contains(&g.binding.map_or((9, 9), |b| (b.group, b.binding))) => {
                return Err("separate_images are not implemented".into());
            }
            _ => {}
        }
    }
    for (_, n, b, t) in &samplers {
        f.push(format!("t:{n}={b}:{t}"));
    }
    let slots = slots::by_binding(&m, &combined)?;
    let mut src = msl::write(&m, &info, &slots, &name)?;
    for (l, n) in names {
        src = src.replace(&format!("[[user(loc{l})"), &format!("[[user({n})")).replace(&format!("[[user(loc{l}),"), &format!("[[user({n}),"));
    }
    Ok(Shader { key, stage, fields: f.join(" "), msl: src, msl_dual: None })
}

/// What one source gives: the stages that translate, or the error of each stage when none does.
pub fn translate_source(src: &[u8], stage: Option<&'static str>) -> Result<Vec<Shader>, String> {
    let stages: &[&'static str] = match stage {
        Some("vert") => &["vert"],
        Some(_) => &["frag"],
        None => &["vert", "frag"],
    };
    let mut out = Vec::new();
    let mut errs = Vec::new();
    for &st in stages {
        match translate(src, st) {
            Ok(s) => out.push(s),
            Err(e) => errs.push(format!("{st}: {e}")),
        }
    }
    if out.is_empty() { Err(errs.join(" / ")) } else { Ok(out) }
}

/// Builds the tool's table: two comment lines, then one row per (key, stage) in that order,
/// `key<TAB>stage<TAB>fields<TAB>where`. The first shader of a (key, stage) wins.
pub fn table(shaders: &[(Shader, String)], tool: &str) -> String {
    crate::table(TABLE_HEAD, tool, shaders)
}
