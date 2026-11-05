//! Translation context: header, definitions, host answers, resources.

#[cfg(test)]
mod calls_test;
pub mod context;
pub mod core;
pub mod definitions;
pub mod emit_shader;
pub mod header;
pub mod host;
pub mod pipeline;
pub mod props;
pub mod props_fmt;
pub mod resources;
pub mod resources_bind;
pub mod resources_res;
