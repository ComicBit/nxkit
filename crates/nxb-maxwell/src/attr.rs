//! `AttributeConsts`, `FeatureFlags`, `AttributeUsage` (the parts the decoder fills).

pub mod consts {
    pub const PRIMITIVE_ID: i32 = 0x060;
    pub const LAYER: i32 = 0x064;
    pub const VIEWPORT_INDEX: i32 = 0x068;
    pub const POSITION_X: i32 = 0x070;
    pub const POSITION_Y: i32 = 0x074;
    pub const FRONT_COLOR_DIFFUSE_R: i32 = 0x280;
    pub const BACK_COLOR_DIFFUSE_R: i32 = 0x2a0;
    pub const CLIP_DISTANCE0: i32 = 0x2c0;
    pub const FOG_COORD: i32 = 0x2e8;
    pub const INSTANCE_ID: i32 = 0x2f8;
    pub const VERTEX_ID: i32 = 0x2fc;
    pub const TEX_COORD_BASE: i32 = 0x300;
    pub const TEX_COORD_END: i32 = TEX_COORD_BASE + 10 * 16;
    pub const VIEWPORT_MASK: i32 = 0x3a0;
    pub const FRONT_FACING: i32 = 0x3fc;
    pub const USER_ATTRIBUTES_COUNT: i32 = 32;
    pub const USER_ATTRIBUTE_BASE: i32 = 0x80;
    pub const USER_ATTRIBUTE_END: i32 = USER_ATTRIBUTE_BASE + USER_ATTRIBUTES_COUNT * 16;
    pub const USER_ATTRIBUTE_PER_PATCH_BASE: i32 = 0x18;
    pub const USER_ATTRIBUTE_PER_PATCH_END: i32 = 0x200;
    /// `Constants.AllAttributesMask` (16 attributes)
    pub const ALL_ATTRIBUTES_MASK: i32 = 0xffff;
    pub const MAX_ATTRIBUTES: i32 = 16;
}

/// `FeatureFlags`.
pub mod feature {
    pub const NONE: i32 = 0;
    pub const FRAG_COORD_XY: i32 = 1 << 1;
    pub const BINDLESS: i32 = 1 << 2;
    pub const INSTANCE_ID: i32 = 1 << 3;
    pub const DRAW_PARAMETERS: i32 = 1 << 4;
    pub const RT_LAYER: i32 = 1 << 5;
    pub const SHUFFLE: i32 = 1 << 6;
    pub const VIEWPORT_INDEX: i32 = 1 << 7;
    pub const VIEWPORT_MASK: i32 = 1 << 8;
    pub const FIXED_FUNC_ATTR: i32 = 1 << 9;
    pub const LOCAL_MEMORY: i32 = 1 << 10;
    pub const SHARED_MEMORY: i32 = 1 << 11;
    pub const STORE: i32 = 1 << 12;
    pub const VTG_AS_COMPUTE: i32 = 1 << 13;
}

/// `ShaderStage`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Compute,
    Vertex,
    TessellationControl,
    TessellationEvaluation,
    Geometry,
    Fragment,
}

impl Stage {
    /// `IsVtg`: vertex, tessellation and geometry stages.
    pub fn is_vtg(self) -> bool {
        matches!(
            self,
            Stage::Vertex
                | Stage::TessellationControl
                | Stage::TessellationEvaluation
                | Stage::Geometry
        )
    }
}

/// `AttributeUsage`.
#[derive(Clone, Debug, Default)]
pub struct AttributeUsage {
    pub used_input_attributes: i32,
    pub used_output_attributes: i32,
    pub used_input_attributes_per_patch: Vec<i32>,
    pub used_output_attributes_per_patch: Vec<i32>,
    pub this_used_input_attributes: i32,
    pub this_input_components: u128,
}

fn add_set(v: &mut Vec<i32>, x: i32) {
    if !v.contains(&x) {
        v.push(x);
    }
}

impl AttributeUsage {
    /// `GetFreeUserAttribute` (`SetNextStage` is never called: the next stage's used inputs are empty).
    pub fn get_free_user_attribute(&self, is_output: bool, index: i32) -> i32 {
        let mut use_mask: i32 = if is_output {
            0
        } else {
            self.this_used_input_attributes
        };
        let mut index = index;
        let mut bit = -1;
        while use_mask != -1 {
            bit = (!use_mask).trailing_zeros() as i32;
            if bit == 32 {
                bit = -1;
                break;
            } else if index < 1 {
                break;
            }
            use_mask |= 1 << bit;
            index -= 1;
        }
        bit
    }

    pub fn set_input_user_attribute(&mut self, index: i32, component: i32) {
        let mask = 1 << index;
        self.used_input_attributes |= mask;
        self.this_used_input_attributes |= mask;
        self.this_input_components |= 1u128 << (index * 4 + component);
    }

    pub fn set_input_user_attribute_per_patch(&mut self, index: i32) {
        add_set(&mut self.used_input_attributes_per_patch, index);
    }

    pub fn set_output_user_attribute(&mut self, index: i32) {
        self.used_output_attributes |= 1 << index;
    }

    pub fn set_output_user_attribute_per_patch(&mut self, index: i32) {
        add_set(&mut self.used_output_attributes_per_patch, index);
    }

    pub fn set_all_input_user_attributes(&mut self) {
        self.used_input_attributes |= consts::ALL_ATTRIBUTES_MASK;
        self.this_input_components |= !0u128 >> (128 - consts::MAX_ATTRIBUTES * 4);
    }

    pub fn set_all_output_user_attributes(&mut self) {
        self.used_output_attributes |= consts::ALL_ATTRIBUTES_MASK;
    }
}
