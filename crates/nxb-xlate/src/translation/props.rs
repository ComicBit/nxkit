//! Part of the translator port (see docs/translator-port.md).
//! `ShaderProperties` and the definitions it holds (`StructuredIr/*Definition.cs`, `StructureType`, `AggregateType`,
//! `BufferLayout`), the descriptors the driver reads (`TextureDescriptor`, `BufferDescriptor`) and the small enums
//! of the resource manager. Dictionaries are insertion ordered (.NET `Dictionary` without removals).

use crate::accessor::{Accessor, SetBinding};
use crate::types::SamplerType;

/// Queries of `IGpuAccessor` the resource manager makes that `accessor.rs` does not answer yet, the answers of `dmsh_xlate`
/// (no extra sets, formatted image loads not supported, no viewport index or mask in vertex and tessellation stages).
pub trait ResourceQueries {
    fn create_extra_set(&mut self) -> i32;
    fn register_texture(&mut self, handle: i32, cbuf_slot: i32);
    fn query_host_supports_image_load_formatted(&self) -> bool;
    fn query_host_supports_viewport_index_vertex_tessellation(&self) -> bool;
    fn query_host_supports_viewport_mask(&self) -> bool;
}

impl ResourceQueries for Accessor<'_> {
    fn create_extra_set(&mut self) -> i32 {
        -1
    }
    fn register_texture(&mut self, _handle: i32, _cbuf_slot: i32) {}
    fn query_host_supports_image_load_formatted(&self) -> bool {
        true
    }
    fn query_host_supports_viewport_index_vertex_tessellation(&self) -> bool {
        true
    }
    fn query_host_supports_viewport_mask(&self) -> bool {
        true
    }
}

/// `SetBindingPair`.
pub type SetBindingPair = SetBinding;

/// An insertion ordered map (a .NET `Dictionary` that never removes): overwriting keeps the position.
#[derive(Clone, Debug)]
pub struct OrdMap<K, V> {
    items: Vec<(K, V)>,
}

impl<K, V> Default for OrdMap<K, V> {
    fn default() -> Self {
        OrdMap { items: Vec::new() }
    }
}

impl<K: PartialEq, V> OrdMap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    pub fn get(&self, k: &K) -> Option<&V> {
        self.items.iter().find(|(a, _)| a == k).map(|(_, v)| v)
    }
    pub fn get_mut(&mut self, k: &K) -> Option<&mut V> {
        self.items.iter_mut().find(|(a, _)| a == k).map(|(_, v)| v)
    }
    pub fn contains_key(&self, k: &K) -> bool {
        self.get(k).is_some()
    }
    /// `dict[k] = v`.
    pub fn insert(&mut self, k: K, v: V) {
        match self.items.iter_mut().find(|(a, _)| *a == k) {
            Some(slot) => slot.1 = v,
            None => self.items.push((k, v)),
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = &(K, V)> {
        self.items.iter()
    }
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.items.iter().map(|(_, v)| v)
    }
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.items.iter().map(|(k, _)| k)
    }
}

/// `AggregateType` (flags enum).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AggregateType(pub u32);

impl AggregateType {
    pub const INVALID: AggregateType = AggregateType(0);
    pub const VOID: AggregateType = AggregateType(1);
    pub const BOOL: AggregateType = AggregateType(2);
    pub const FP32: AggregateType = AggregateType(3);
    pub const FP64: AggregateType = AggregateType(4);
    pub const S32: AggregateType = AggregateType(5);
    pub const U32: AggregateType = AggregateType(6);
    pub const ELEMENT_TYPE_MASK: AggregateType = AggregateType(0xff);
    pub const ELEMENT_COUNT_SHIFT: u32 = 8;
    pub const ELEMENT_COUNT_MASK: AggregateType = AggregateType(3 << 8);
    pub const SCALAR: AggregateType = AggregateType(0);
    pub const VECTOR2: AggregateType = AggregateType(1 << 8);
    pub const VECTOR3: AggregateType = AggregateType(2 << 8);
    pub const VECTOR4: AggregateType = AggregateType(3 << 8);
    pub const ARRAY: AggregateType = AggregateType(1 << 10);

    /// `a | b`.
    pub const fn or(self, o: AggregateType) -> AggregateType {
        AggregateType(self.0 | o.0)
    }

    /// `a & b`.
    pub const fn and(self, o: AggregateType) -> AggregateType {
        AggregateType(self.0 & o.0)
    }

    pub const fn has(self, o: AggregateType) -> bool {
        self.0 & o.0 == o.0
    }

    /// `AggregateTypeExtensions.GetSizeInBytes`.
    pub fn size_in_bytes(self) -> i32 {
        let mut size = match self.0 & 0xff {
            2 | 3 | 5 | 6 => 4,
            4 => 8,
            _ => 0,
        };
        match self.0 & Self::ELEMENT_COUNT_MASK.0 {
            x if x == Self::VECTOR2.0 => size *= 2,
            x if x == Self::VECTOR3.0 => size *= 3,
            x if x == Self::VECTOR4.0 => size *= 4,
            _ => {}
        }
        size
    }
}

/// `BufferLayout`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferLayout {
    Std140,
    Std430,
}

/// `StructureField`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructureField {
    pub ty: AggregateType,
    pub name: String,
    pub array_length: i32,
}

impl StructureField {
    pub fn new(ty: AggregateType, name: &str, array_length: i32) -> Self {
        StructureField {
            ty,
            name: name.to_string(),
            array_length,
        }
    }
}

/// `StructureType`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructureType {
    pub fields: Vec<StructureField>,
}

/// `BufferDefinition`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BufferDefinition {
    pub layout: BufferLayout,
    pub set: i32,
    pub binding: i32,
    pub name: String,
    pub ty: StructureType,
}

/// `MemoryDefinition`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryDefinition {
    pub name: String,
    pub ty: AggregateType,
    pub array_length: i32,
}

impl MemoryDefinition {
    pub fn new(name: &str, ty: AggregateType, array_length: i32) -> Self {
        MemoryDefinition {
            name: name.to_string(),
            ty,
            array_length,
        }
    }
}

/// `TextureUsageFlags`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct TextureUsageFlags(pub u32);

impl TextureUsageFlags {
    pub const NONE: TextureUsageFlags = TextureUsageFlags(0);
    pub const RES_SCALE_UNSUPPORTED: TextureUsageFlags = TextureUsageFlags(1);
    pub const NEEDS_SCALE_VALUE: TextureUsageFlags = TextureUsageFlags(2);
    pub const IMAGE_STORE: TextureUsageFlags = TextureUsageFlags(4);
    pub const IMAGE_COHERENT: TextureUsageFlags = TextureUsageFlags(8);
}

impl std::ops::BitOr for TextureUsageFlags {
    type Output = TextureUsageFlags;
    fn bitor(self, o: Self) -> Self {
        TextureUsageFlags(self.0 | o.0)
    }
}

impl std::ops::BitOrAssign for TextureUsageFlags {
    fn bitor_assign(&mut self, o: Self) {
        self.0 |= o.0;
    }
}

/// `BufferUsageFlags`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct BufferUsageFlags(pub u8);

impl BufferUsageFlags {
    pub const NONE: BufferUsageFlags = BufferUsageFlags(0);
    pub const WRITE: BufferUsageFlags = BufferUsageFlags(1);
}

/// `TextureFlags` of a texture operation (the bits the resource manager and its callers test).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct TextureFlags(pub u32);

impl TextureFlags {
    pub const NONE: TextureFlags = TextureFlags(0);
    pub const BINDLESS: TextureFlags = TextureFlags(1);
    pub const GATHER: TextureFlags = TextureFlags(1 << 1);
    pub const DERIVATIVES: TextureFlags = TextureFlags(1 << 2);
    pub const INT_COORDS: TextureFlags = TextureFlags(1 << 3);
    pub const LOD_BIAS: TextureFlags = TextureFlags(1 << 4);
    pub const LOD_LEVEL: TextureFlags = TextureFlags(1 << 5);
    pub const OFFSET: TextureFlags = TextureFlags(1 << 6);
    pub const OFFSETS: TextureFlags = TextureFlags(1 << 7);
    pub const COHERENT: TextureFlags = TextureFlags(1 << 8);
    pub const ATOMIC_BOUNDS_MASK: TextureFlags = TextureFlags(3 << 20);
    pub const ATOMIC_DISCARD_RESULT: TextureFlags = TextureFlags(1 << 22);
    pub const ATOMIC_METAL: TextureFlags = TextureFlags(1 << 23);
    pub const ATOMIC_PREDICATE_RESULT: TextureFlags = TextureFlags(1 << 24);
    pub const ATOMIC_MASK: TextureFlags = TextureFlags(15 << 16);

    /// `flags.HasFlag(f)`.
    pub fn has_flag(self, f: TextureFlags) -> bool {
        self.0 & f.0 == f.0
    }
}

/// `ShaderStage` (the numbers index the stage prefixes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShaderStage {
    Compute = 0,
    Vertex = 1,
    TessellationControl = 2,
    TessellationEvaluation = 3,
    Geometry = 4,
    Fragment = 5,
}

impl ShaderStage {
    /// `ShaderStageExtensions.SupportsRenderScale`.
    pub fn supports_render_scale(self) -> bool {
        matches!(
            self,
            ShaderStage::Vertex | ShaderStage::Fragment | ShaderStage::Compute
        )
    }
}

/// `TextureDefinition`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureDefinition {
    pub set: i32,
    pub binding: i32,
    pub array_length: i32,
    pub separate: bool,
    pub name: String,
    pub ty: SamplerType,
    /// index into [`crate::types::TEXTURE_FORMATS`]
    pub format: usize,
    pub flags: TextureUsageFlags,
}

impl TextureDefinition {
    /// The short constructor (`TextureDefinition(set, binding, name, type)`).
    pub fn simple(set: i32, binding: i32, name: &str, ty: SamplerType) -> Self {
        TextureDefinition {
            set,
            binding,
            array_length: 1,
            separate: false,
            name: name.to_string(),
            ty,
            format: 0,
            flags: TextureUsageFlags::NONE,
        }
    }

    /// `SetFlag`.
    pub fn set_flag(&self, flag: TextureUsageFlags) -> Self {
        let mut d = self.clone();
        d.flags |= flag;
        d
    }
}

/// `TextureDescriptor`: what the driver reads of a texture or image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureDescriptor {
    pub set: i32,
    pub binding: i32,
    pub ty: SamplerType,
    pub format: usize,
    pub cbuf_slot: i32,
    pub handle_index: i32,
    pub array_length: i32,
    pub separate: bool,
    pub flags: TextureUsageFlags,
}

/// `BufferDescriptor`. `slot`, `sb_cb_slot` and `sb_cb_offset` are already truncated to byte, byte and ushort.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferDescriptor {
    pub set: i32,
    pub binding: i32,
    pub slot: u8,
    pub sb_cb_slot: u8,
    pub sb_cb_offset: u16,
    pub flags: BufferUsageFlags,
}

impl BufferDescriptor {
    /// The constant buffer constructor.
    pub fn cbuf(set: i32, binding: i32, slot: i32) -> Self {
        BufferDescriptor {
            set,
            binding,
            slot: slot as u8,
            sb_cb_slot: 0,
            sb_cb_offset: 0,
            flags: BufferUsageFlags::NONE,
        }
    }

    /// The storage buffer constructor.
    pub fn sbuf(
        set: i32,
        binding: i32,
        slot: i32,
        sb_cb_slot: i32,
        sb_cb_offset: i32,
        flags: BufferUsageFlags,
    ) -> Self {
        BufferDescriptor {
            set,
            binding,
            slot: slot as u8,
            sb_cb_slot: sb_cb_slot as u8,
            sb_cb_offset: sb_cb_offset as u16,
            flags,
        }
    }
}

/// `SupportBuffer` constants.
pub mod support_buffer {
    use super::*;

    pub const BINDING: i32 = 0;
    pub const FRAGMENT_IS_BGRA_COUNT: i32 = 8;
    pub const RENDER_SCALE_MAX_COUNT: i32 = 1 + 64 + 8;
    /// `sizeof(Vector4<float>)`
    pub const FIELD_SIZE: i32 = 16;
    pub const FRAGMENT_ALPHA_TEST_OFFSET: i32 = 0;
    pub const FRAGMENT_IS_BGRA_OFFSET: i32 = 16;
    pub const VIEWPORT_INVERSE_OFFSET: i32 = 16 + 8 * 16;
    pub const VIEWPORT_SIZE_OFFSET: i32 = VIEWPORT_INVERSE_OFFSET + 16;
    pub const FRAGMENT_RENDER_SCALE_COUNT_OFFSET: i32 = VIEWPORT_SIZE_OFFSET + 16;
    pub const GRAPHICS_RENDER_SCALE_OFFSET: i32 = FRAGMENT_RENDER_SCALE_COUNT_OFFSET + 16;
    pub const COMPUTE_RENDER_SCALE_OFFSET: i32 = GRAPHICS_RENDER_SCALE_OFFSET + FIELD_SIZE;
    pub const TFE_OFFSET_OFFSET: i32 = GRAPHICS_RENDER_SCALE_OFFSET + 73 * 16;
    pub const TFE_VERTEX_COUNT_OFFSET: i32 = TFE_OFFSET_OFFSET + 16;
    pub const REQUIRED_SIZE: i32 = TFE_VERTEX_COUNT_OFFSET + 16;

    /// `SupportBuffer.GetStructureType`.
    pub fn structure_type() -> StructureType {
        StructureType {
            fields: vec![
                StructureField::new(AggregateType::U32, "alpha_test", 1),
                StructureField::new(
                    AggregateType::ARRAY.or(AggregateType::U32),
                    "is_bgra",
                    FRAGMENT_IS_BGRA_COUNT,
                ),
                StructureField::new(
                    AggregateType::VECTOR4.or(AggregateType::FP32),
                    "viewport_inverse",
                    1,
                ),
                StructureField::new(
                    AggregateType::VECTOR4.or(AggregateType::FP32),
                    "viewport_size",
                    1,
                ),
                StructureField::new(AggregateType::S32, "frag_scale_count", 1),
                StructureField::new(
                    AggregateType::ARRAY.or(AggregateType::FP32),
                    "render_scale",
                    RENDER_SCALE_MAX_COUNT,
                ),
                StructureField::new(
                    AggregateType::VECTOR4.or(AggregateType::S32),
                    "tfe_offset",
                    1,
                ),
                StructureField::new(AggregateType::S32, "tfe_vertex_count", 1),
            ],
        }
    }
}

/// `ShaderProperties`.
#[derive(Clone, Debug, Default)]
pub struct ShaderProperties {
    /// keyed by binding
    pub constant_buffers: OrdMap<i32, BufferDefinition>,
    /// keyed by binding
    pub storage_buffers: OrdMap<i32, BufferDefinition>,
    /// keyed by (set, binding)
    pub textures: OrdMap<(i32, i32), TextureDefinition>,
    /// keyed by (set, binding)
    pub images: OrdMap<(i32, i32), TextureDefinition>,
    /// the ids are the positions
    pub local_memories: Vec<MemoryDefinition>,
    pub shared_memories: Vec<MemoryDefinition>,
    pub uses_physical_pointers: bool,
}

impl ShaderProperties {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_or_update_constant_buffer(&mut self, d: BufferDefinition) {
        self.constant_buffers.insert(d.binding, d);
    }

    pub fn add_or_update_storage_buffer(&mut self, d: BufferDefinition) {
        self.storage_buffers.insert(d.binding, d);
    }

    pub fn add_or_update_texture(&mut self, d: TextureDefinition) {
        self.textures.insert((d.set, d.binding), d);
    }

    pub fn add_or_update_image(&mut self, d: TextureDefinition) {
        self.images.insert((d.set, d.binding), d);
    }

    pub fn add_local_memory(&mut self, d: MemoryDefinition) -> i32 {
        self.local_memories.push(d);
        self.local_memories.len() as i32 - 1
    }

    pub fn add_shared_memory(&mut self, d: MemoryDefinition) -> i32 {
        self.shared_memories.push(d);
        self.shared_memories.len() as i32 - 1
    }

    /// `GetTextureFormat`: an index into [`crate::types::TEXTURE_FORMATS`].
    pub fn get_texture_format(acc: &mut Accessor, handle: i32, cbuf_slot: i32) -> usize {
        if acc.query_host_supports_image_load_formatted() {
            return 0;
        }
        let mut format = acc.query_texture_format(handle, cbuf_slot);
        if format == 0 {
            acc.log(&format!("Unknown format for texture {handle}."));
            format = 25; // R8G8B8A8Unorm
        }
        format
    }

    /// `GetTextureFormatAtomic`. `strict` is the Metal path: an `Err` for an unsupported format.
    pub fn get_texture_format_atomic(
        acc: &mut Accessor,
        handle: i32,
        cbuf_slot: i32,
        strict: bool,
    ) -> Result<usize, String> {
        let mut format = acc.query_texture_format(handle, cbuf_slot);
        // R32Uint = 11, R32Sint = 12
        if format != 11 && format != 12 {
            let name = crate::types::TEXTURE_FORMATS[format];
            if strict {
                return Err(format!(
                    "Metal image atomics require an R32 integer descriptor format, got {name}."
                ));
            }
            acc.log(&format!("Unsupported format for texture {handle}: {name}."));
            format = 12;
        }
        Ok(format)
    }
}

pub use super::props_fmt::{dimensions, short_type_name, to_glsl_format};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordmap_keeps_first_position() {
        let mut m = OrdMap::new();
        m.insert(3, "a");
        m.insert(1, "b");
        m.insert(3, "c");
        assert_eq!(m.keys().copied().collect::<Vec<_>>(), vec![3, 1]);
        assert_eq!(m.get(&3), Some(&"c"));
    }

    #[test]
    fn names_and_sizes() {
        assert_eq!(
            short_type_name(SamplerType::TEXTURE_2D.with(SamplerType::ARRAY)).unwrap(),
            "2da"
        );
        assert_eq!(
            short_type_name(SamplerType::TEXTURE_CUBE.with(SamplerType::SHADOW)).unwrap(),
            "cubes"
        );
        assert_eq!(
            AggregateType::VECTOR4
                .or(AggregateType::FP32)
                .size_in_bytes(),
            16
        );
        assert_eq!(to_glsl_format(25), "rgba8");
        assert_eq!(support_buffer::structure_type().fields.len(), 8);
    }
}
