//! Vulkan games: SPIR-V modules from the RomFS to MSL functions `vk_<key>` and rows of `vk_shaders.tsv`, the contract of
//! the reference's `dm/gfx/tools/vk_shaders.py`.

use std::collections::BTreeSet;

use crate::spirv::{self, DEC_BINDING, DEC_SET, Spv};
use crate::{Shader, msl, slots};

/// The first comment line of `vk_shaders.tsv`.
pub const TABLE_HEAD: &str = "# Vulkan shaders (dm/gfx/tools/vk_shaders.py): key = sha256(SPIR-V)[:16], Metal function vk_<key>; resources kind:set.binding=slot (u uniform buffer, b storage buffer, t image, s sampler, c combined, p push constants)";

/// What became of one SPIR-V module.
#[derive(Clone, Debug)]
pub enum Outcome {
    /// A vertex or fragment shader in MSL.
    Shader(Shader),
    /// A stage Metal has no counterpart for, by the short name `vk_shaders.py` reports (`geom`, `tesc`, `tese`, `comp` and so on).
    Skipped(&'static str),
}

/// The short stage name of a SPIR-V execution model.
fn stage_name(model: u32) -> &'static str {
    match model {
        0 => "vert",
        1 => "tesc",
        2 => "tese",
        3 => "geom",
        4 => "frag",
        5 => "comp",
        5267 | 5364 => "task",
        5268 | 5365 => "mesh",
        5313..=5318 => "rt",
        _ => "other",
    }
}

/// The table key of a module: the first 16 hex digits of the SHA-256 of the game's bytes, which the host hashes as well.
pub fn key_of(bytes: &[u8]) -> String {
    nxb_model::sha256(bytes).to_hex()[..16].to_string()
}

/// Translates one SPIR-V module of a Vulkan game. The vertex shader's clip-space Y is negated, as Vulkan's points down and
/// Metal's points up. Its depth range stays as it is. An error is a module the host cannot use.
pub fn translate(bytes: &[u8]) -> Result<Outcome, String> {
    let key = key_of(bytes);
    let mut s = Spv::parse(bytes)?;
    spirv::fix_strides(&mut s);
    let stage = stage_name(s.execution_model()?);
    if stage != "vert" && stage != "frag" {
        return Ok(Outcome::Skipped(stage));
    }
    let twins = spirv::split_combined(&mut s)?;
    let combined: BTreeSet<(u32, u32)> = twins
        .keys()
        .filter_map(|&v| Some((s.decoration(v, DEC_SET).unwrap_or(0), s.decoration(v, DEC_BINDING)?)))
        .collect();
    let name = format!("vk_{key}");
    let (m, info) = crate::to_ir(&s.words(), stage == "vert", &name, |_| Ok(()))?;
    let slots = slots::assign(&m, &combined)?;
    let src = msl::write(&m, &info, &slots, &name)?;
    let dual = if stage == "frag" { msl::dual_source(&src, &name) } else { None };
    let res = slots.tokens(&info, 0).join(" ");
    Ok(Outcome::Shader(Shader { key, stage, fields: res, msl: src, msl_dual: dual }))
}

/// Builds the tool's table from the translated shaders and their RomFS paths: two comment lines, then one row per key in key
/// order, `key<TAB>stage<TAB>resources<TAB>/path`. The first shader of a key wins.
pub fn table(shaders: &[(Shader, String)], tool: &str) -> String {
    crate::table(TABLE_HEAD, tool, shaders)
}
