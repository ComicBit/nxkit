//! Answers of `IGpuAccessor` that the job accessor does not override (the interface defaults).

pub const SUBGROUP_SIZE: i32 = 32;
pub const STORAGE_BUFFER_OFFSET_ALIGNMENT: i32 = 16;
pub const SUPPORTS_SHADER_BALLOT: bool = true;
pub const SUPPORTS_SHADER_BARRIER_DIVERGENCE: bool = true;
pub const SUPPORTS_FRAGMENT_SHADER_INTERLOCK: bool = true;
pub const SUPPORTS_FRAGMENT_SHADER_ORDERING_INTEL: bool = false;
pub const SUPPORTS_GEOMETRY_SHADER: bool = true;
pub const SUPPORTS_LAYER_VERTEX_TESSELLATION: bool = true;
pub const SUPPORTS_NON_CONSTANT_TEXTURE_OFFSET: bool = true;
pub const SUPPORTS_SCALED_VERTEX_FORMATS: bool = true;
pub const SUPPORTS_SEPARATE_SAMPLER: bool = true;
pub const SUPPORTS_SHADER_NON_UNIFORM_INDEXING: bool = false;
pub const SUPPORTS_TRANSFORM_FEEDBACK: bool = true;
pub const SUPPORTS_VIEWPORT_MASK: bool = true;
pub const TRANSFORM_FEEDBACK_ENABLED: bool = false;
pub const HAS_FRONT_FACING_BUG: bool = false;
pub const HAS_CONSTANT_BUFFER_DRAW_PARAMETERS: bool = false;
/// `QueryConstantBufferUse`.
pub const CONSTANT_BUFFER_USE: u32 = 0;
