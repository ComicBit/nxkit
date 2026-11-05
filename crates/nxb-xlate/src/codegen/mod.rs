//! Part of the translator port (see docs/translator-port.md).

pub mod props;
pub mod props_text;
#[rustfmt::skip]
pub mod spirv_cap;
#[rustfmt::skip]
pub mod spirv_enums;
#[rustfmt::skip]
pub mod spirv_op;
pub mod cap_names;
pub mod context;
pub mod declarations;
pub mod generator;
pub mod instr_helpers;
pub mod instr_image_atomic;
pub mod instr_texture;
pub mod instructions;
#[cfg(test)]
pub mod testutil;
#[rustfmt::skip]
mod spirv_build_0;
#[cfg(test)]
mod tests;
