//! Capability numbers used by the generator (the `Capability` enum of the SPIR-V grammar).

use super::spirv_cap::capability as c;

pub const SHADER: u32 = c::Shader;
pub const GEOMETRY: u32 = c::Geometry;
pub const FLOAT64: u32 = c::Float64;
pub const INT64: u32 = c::Int64;
pub const GROUP_NON_UNIFORM_BALLOT: u32 = c::GroupNonUniformBallot;
pub const GROUP_NON_UNIFORM_SHUFFLE: u32 = c::GroupNonUniformShuffle;
pub const GROUP_NON_UNIFORM_VOTE: u32 = c::GroupNonUniformVote;
pub const IMAGE_BUFFER: u32 = c::ImageBuffer;
pub const IMAGE_GATHER_EXTENDED: u32 = c::ImageGatherExtended;
pub const IMAGE_QUERY: u32 = c::ImageQuery;
pub const SAMPLED_BUFFER: u32 = c::SampledBuffer;
pub const SAMPLE_RATE_SHADING: u32 = c::SampleRateShading;
pub const DRAW_PARAMETERS: u32 = c::DrawParameters;
pub const PHYSICAL_STORAGE_BUFFER_ADDRESSES: u32 = c::PhysicalStorageBufferAddresses;
pub const SHADER_VIEWPORT_INDEX_LAYER_EXT: u32 = c::ShaderViewportIndexLayerEXT;
pub const SHADER_VIEWPORT_MASK_NV: u32 = c::ShaderViewportMaskNV;
pub const FRAGMENT_SHADER_PIXEL_INTERLOCK_EXT: u32 = c::FragmentShaderPixelInterlockEXT;
pub const SHADER_NON_UNIFORM: u32 = c::ShaderNonUniform;
pub const SAMPLED_IMAGE_ARRAY_NON_UNIFORM_INDEXING: u32 = c::SampledImageArrayNonUniformIndexing;
pub const STORAGE_IMAGE_ARRAY_NON_UNIFORM_INDEXING: u32 = c::StorageImageArrayNonUniformIndexing;
