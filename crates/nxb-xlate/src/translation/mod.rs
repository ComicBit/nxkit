//! Translation context: header, definitions, host answers, resources.

#[cfg(test)]
mod calls_test;
pub mod context;
pub mod core;
pub mod definitions;
pub mod emit_shader;
#[cfg(test)]
pub mod emit_test_util;
#[cfg(test)]
mod emit_tests;
pub mod geometry;
#[cfg(test)]
mod geometry_tests;
pub mod header;
pub mod host;
pub mod pipeline;
pub mod props;
pub mod props_fmt;
pub mod resources;
pub mod resources_bind;
pub mod resources_res;
pub mod vac;
