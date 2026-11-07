//! Shaders of games that draw with Vulkan or OpenGL instead of NVN, translated to MSL for the host's layers.
//!
//! The host's Vulkan layer looks up `vk_<key>` functions and reads `vk_shaders.tsv`, its OpenGL layer `gl_<key>_v` and
//! `gl_<key>_f` with `gl_shaders.tsv`. This crate writes the MSL and the table rows. [`vk`] takes SPIR-V modules, [`gl`]
//! takes GLSL sources, which glslang compiles to SPIR-V first. Both then share naga's SPIR-V reader and MSL writer. The crate
//! does no I/O: the caller finds the inputs, compiles the MSL with Metal's tools and writes the files.

pub mod gl;
mod glsl_fix;
mod glsl_pp;
mod io;
mod msl;
mod mvgl;
mod slots;
mod spirv;
pub mod vk;

pub use mvgl::{doboz, mvgl_entries, nso_glsl};
pub use spirv::is_spirv;

use std::collections::BTreeSet;

use naga::valid::{Capabilities, ModuleInfo, ValidationFlags, Validator};

/// One translated shader stage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shader {
    /// The table key, 16 hex digits.
    pub key: String,
    /// `vert` or `frag`.
    pub stage: &'static str,
    /// The table's third column: the resources, or for OpenGL the inputs, uniforms, samplers and outputs.
    pub fields: String,
    /// The MSL source with one entry point.
    pub msl: String,
    /// Vulkan only: the dual-source variant `vk_<key>_d` of a fragment shader with an output at location 1.
    pub msl_dual: Option<String>,
}

/// The naga version and the revision of this crate's passes, for the table's second line and for cache keys. A change to the
/// passes that changes the output raises the revision.
pub fn translator_id() -> String {
    format!("nxb-layer 1, naga 30.0.1, glslang {}", gl::GLSLANG)
}

/// Reads SPIR-V `words` into naga's IR, splits array varyings, runs `pass`, names the entry point `name` and validates the
/// module. `flip_y` negates the clip-space Y of a vertex shader.
pub(crate) fn to_ir(
    words: &[u32],
    flip_y: bool,
    name: &str,
    pass: impl FnOnce(&mut naga::Module) -> Result<(), String>,
) -> Result<(naga::Module, ModuleInfo), String> {
    let o = naga::front::spv::Options {
        adjust_coordinate_space: flip_y,
        strict_capabilities: false,
        block_ctx_dump_prefix: None,
    };
    let mut m = naga::front::spv::Frontend::new(words.iter().copied(), &o)
        .parse()
        .map_err(|e| format!("SPIR-V reader: {e}"))?;
    if m.entry_points.len() != 1 {
        return Err(format!(
            "{} entry points (one expected)",
            m.entry_points.len()
        ));
    }
    io::flatten_varyings(&mut m)?;
    pass(&mut m)?;
    if let Some(ep) = m.entry_points.first_mut() {
        ep.name = name.to_string();
    }
    // a split combined image sampler keeps one binding for its image and its sampler, which the MSL binding map serves as
    // one texture and one sampler slot, so the binding collision check is off
    let info = Validator::new(
        ValidationFlags::all() - ValidationFlags::BINDINGS,
        Capabilities::all(),
    )
    .validate(&m)
    .map_err(|e| {
        let e = e.into_inner();
        let mut t = format!("validation: {e}");
        let mut src = std::error::Error::source(&e);
        while let Some(c) = src {
            t += &format!(": {c}");
            src = c.source();
        }
        t
    })?;
    Ok((m, info))
}

/// A tool's table: `head`, `# <tool>`, then one row per (key, stage) in that order, `key<TAB>stage<TAB>fields<TAB>where`. The
/// first shader of a (key, stage) wins.
pub(crate) fn table(head: &str, tool: &str, shaders: &[(Shader, String)]) -> String {
    let mut rows: Vec<&(Shader, String)> = Vec::new();
    let mut seen = BTreeSet::new();
    for r in shaders {
        if seen.insert((r.0.key.as_str(), r.0.stage)) {
            rows.push(r);
        }
    }
    rows.sort_by(|a, b| (&a.0.key, a.0.stage).cmp(&(&b.0.key, b.0.stage)));
    let mut out = format!("{head}\n# {tool}\n");
    for (s, w) in rows {
        out += &format!("{}\t{}\t{}\t{w}\n", s.key, s.stage, s.fields);
    }
    out
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
