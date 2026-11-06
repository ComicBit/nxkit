//! Part of the translator port (see docs/translator-port.md).
//! `ResourceReservations`, `ResourceReservationCounts`, `IoDefinition` and `IoUsage`.

use super::props::{ResourceQueries, SetBindingPair};
use crate::accessor::Accessor;
use crate::ir::{IoVariable, StorageKind};
use std::collections::HashMap;

/// `TfeBuffersCount`.
pub const TFE_BUFFERS_COUNT: i32 = 4;
/// `MaxVertexBufferTextures`.
pub const MAX_VERTEX_BUFFER_TEXTURES: i32 = 32;
const TEXTURE_SET_INDEX: i32 = 2;

/// `IoDefinition`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IoDefinition {
    pub storage_kind: StorageKind,
    pub io_variable: IoVariable,
    pub location: i32,
    pub component: i32,
}

impl IoDefinition {
    pub fn new(
        storage_kind: StorageKind,
        io_variable: IoVariable,
        location: i32,
        component: i32,
    ) -> Self {
        IoDefinition {
            storage_kind,
            io_variable,
            location,
            component,
        }
    }
}

/// `FeatureFlags` bits `IoUsage` reads (`RtLayer`, `ViewportIndex`, `ViewportMask`): the C# values.
pub mod feature_flags {
    pub const RT_LAYER: u32 = 1 << 5;
    pub const VIEWPORT_INDEX: u32 = 1 << 7;
    pub const VIEWPORT_MASK: u32 = 1 << 8;
}

/// `IoUsage`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IoUsage {
    pub used_features: u32,
    pub clip_distances_written: u8,
    pub user_defined_map: i32,
}

impl IoUsage {
    pub fn uses_rt_layer(&self) -> bool {
        self.used_features & feature_flags::RT_LAYER != 0
    }
    pub fn uses_viewport_index(&self) -> bool {
        self.used_features & feature_flags::VIEWPORT_INDEX != 0
    }
    pub fn uses_viewport_mask(&self) -> bool {
        self.used_features & feature_flags::VIEWPORT_MASK != 0
    }
    /// `Combine`.
    pub fn combine(&self, o: &IoUsage) -> IoUsage {
        IoUsage {
            used_features: self.used_features | o.used_features,
            clip_distances_written: self.clip_distances_written | o.clip_distances_written,
            user_defined_map: self.user_defined_map | o.user_defined_map,
        }
    }
}

/// `ResourceReservationCounts`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceReservationCounts {
    pub reserved_constant_buffers: i32,
    pub reserved_storage_buffers: i32,
    pub reserved_textures: i32,
    pub reserved_images: i32,
}

impl ResourceReservationCounts {
    pub fn new(is_transform_feedback_emulated: bool, vertex_as_compute: bool) -> Self {
        let r = ResourceReservations::new_counts(is_transform_feedback_emulated, vertex_as_compute);
        ResourceReservationCounts {
            reserved_constant_buffers: r.reserved_constant_buffers,
            reserved_storage_buffers: r.reserved_storage_buffers,
            reserved_textures: r.reserved_textures,
            reserved_images: r.reserved_images,
        }
    }
}

/// `ResourceReservations`.
#[derive(Clone, Debug, Default)]
pub struct ResourceReservations {
    pub vertex_info_constant_buffer_binding: i32,
    pub vertex_output_storage_buffer_binding: i32,
    pub geometry_vertex_output_storage_buffer_binding: i32,
    pub geometry_index_output_storage_buffer_binding: i32,
    pub index_buffer_texture_binding: i32,
    pub topology_remap_buffer_texture_binding: i32,
    pub reserved_constant_buffers: i32,
    pub reserved_storage_buffers: i32,
    pub reserved_textures: i32,
    pub reserved_images: i32,
    pub geometry_input_fixed: bool,
    pub input_size_per_invocation: i32,
    pub output_size_per_invocation: i32,
    tfe_buffer_sb_base_binding: i32,
    vertex_buffer_texture_base_binding: i32,
    offsets: HashMap<IoDefinition, i32>,
}

impl ResourceReservations {
    /// `OutputSizeInBytesPerInvocation`.
    pub fn output_size_in_bytes_per_invocation(&self) -> i32 {
        self.output_size_per_invocation * 4
    }

    /// The first constructor (counts only).
    pub fn new_counts(is_transform_feedback_emulated: bool, vertex_as_compute: bool) -> Self {
        let mut r = ResourceReservations {
            reserved_constant_buffers: 1,
            ..Default::default()
        };
        if is_transform_feedback_emulated {
            r.tfe_buffer_sb_base_binding = r.reserved_storage_buffers;
            r.reserved_storage_buffers = TFE_BUFFERS_COUNT;
        }
        if vertex_as_compute {
            r.vertex_info_constant_buffer_binding = r.reserved_constant_buffers;
            r.reserved_constant_buffers += 1;
            r.vertex_output_storage_buffer_binding = r.reserved_storage_buffers;
            r.reserved_storage_buffers += 1;
            r.geometry_vertex_output_storage_buffer_binding = r.reserved_storage_buffers;
            r.reserved_storage_buffers += 1;
            r.geometry_index_output_storage_buffer_binding = r.reserved_storage_buffers;
            r.reserved_storage_buffers += 1;
            r.index_buffer_texture_binding = r.reserved_textures;
            r.topology_remap_buffer_texture_binding = r.reserved_textures + 1;
            r.vertex_buffer_texture_base_binding = r.reserved_textures + 2;
            r.reserved_textures += 2 + MAX_VERTEX_BUFFER_TEXTURES;
        }
        r
    }

    /// The second constructor (with the IO offset maps).
    pub fn new(
        acc: &mut Accessor,
        is_transform_feedback_emulated: bool,
        vertex_as_compute: bool,
        vac_input: Option<IoUsage>,
        vac_output: IoUsage,
    ) -> Self {
        let mut r = Self::new_counts(is_transform_feedback_emulated, vertex_as_compute);
        if vertex_as_compute {
            r.geometry_input_fixed = acc.query_geometry_input_fixed();
            if let Some(input) = vac_input {
                r.input_size_per_invocation = if r.geometry_input_fixed {
                    r.fill_geometry_input_map()
                } else {
                    r.fill_io_offset_map(acc, StorageKind::Input, &input)
                };
            }
            r.output_size_per_invocation =
                r.fill_io_offset_map(acc, StorageKind::Output, &vac_output);
        }
        r
    }

    fn add(&mut self, kind: StorageKind, var: IoVariable, loc: i32, comp: i32, offset: i32) {
        // `Dictionary.Add` throws on a duplicate, the maps are built from distinct keys
        self.offsets
            .insert(IoDefinition::new(kind, var, loc, comp), offset);
    }

    fn fill_geometry_input_map(&mut self) -> i32 {
        let k = StorageKind::Input;
        for c in 0..4 {
            self.add(k, IoVariable::Position, 0, c, c);
        }
        self.add(k, IoVariable::PointSize, 0, 0, 4);
        for c in 0..8 {
            self.add(k, IoVariable::ClipDistance, 0, c, 5 + c);
        }
        self.add(k, IoVariable::Layer, 0, 0, 13);
        self.add(k, IoVariable::ViewportIndex, 0, 0, 14);
        for a in 0..32 {
            for c in 0..4 {
                self.add(k, IoVariable::UserDefined, a, c, 16 + 4 * a + c);
            }
        }
        144
    }

    fn fill_io_offset_map(
        &mut self,
        acc: &mut Accessor,
        kind: StorageKind,
        usage: &IoUsage,
    ) -> i32 {
        let mut offset = 0;
        for c in 0..4 {
            self.add(kind, IoVariable::Position, 0, c, offset);
            offset += 1;
        }
        self.add(kind, IoVariable::PointSize, 0, 0, offset);
        offset += 1;
        let mut clip = usage.clip_distances_written as u32;
        while clip != 0 {
            let index = clip.trailing_zeros() as i32;
            self.add(kind, IoVariable::ClipDistance, 0, index, offset);
            offset += 1;
            clip &= !(1 << index);
        }
        if usage.uses_rt_layer() {
            self.add(kind, IoVariable::Layer, 0, 0, offset);
            offset += 1;
        }
        if usage.uses_viewport_index()
            && acc.query_host_supports_viewport_index_vertex_tessellation()
        {
            // (sic) the C# stores VertexIndex here
            self.add(kind, IoVariable::VertexIndex, 0, 0, offset);
            offset += 1;
        }
        if usage.uses_viewport_mask() && acc.query_host_supports_viewport_mask() {
            self.add(kind, IoVariable::ViewportMask, 0, 0, offset);
            offset += 1;
        }
        let mut map = usage.user_defined_map as u32;
        while map != 0 {
            let loc = map.trailing_zeros() as i32;
            for c in 0..4 {
                self.add(kind, IoVariable::UserDefined, loc, c, offset);
                offset += 1;
            }
            map &= !(1 << loc);
        }
        offset
    }

    /// The output offsets in the order of `Offsets` (a .NET `Dictionary` without removals enumerates in insertion order,
    /// and the offsets are assigned in increasing order).
    pub fn output_offsets(&self) -> Vec<(IoDefinition, i32)> {
        let mut v: Vec<(IoDefinition, i32)> = self
            .offsets
            .iter()
            .filter(|(d, _)| d.storage_kind == StorageKind::Output)
            .map(|(d, o)| (*d, *o))
            .collect();
        v.sort_by_key(|(_, o)| *o);
        v
    }

    /// `IsVectorOrArrayVariable`.
    pub fn is_vector_or_array_variable(v: IoVariable) -> bool {
        v == IoVariable::ClipDistance || v == IoVariable::Position
    }

    pub fn get_tfe_buffer_storage_buffer_binding(&self, buffer_index: i32) -> i32 {
        self.tfe_buffer_sb_base_binding + buffer_index
    }

    pub fn get_vertex_buffer_texture_binding(&self, va_location: i32) -> i32 {
        self.vertex_buffer_texture_base_binding + va_location
    }

    pub fn get_vertex_buffer_texture_set_and_binding(&self, va_location: i32) -> SetBindingPair {
        SetBindingPair {
            set: TEXTURE_SET_INDEX,
            binding: self.get_vertex_buffer_texture_binding(va_location),
        }
    }

    pub fn get_index_buffer_texture_set_and_binding(&self) -> SetBindingPair {
        SetBindingPair {
            set: TEXTURE_SET_INDEX,
            binding: self.index_buffer_texture_binding,
        }
    }

    pub fn get_topology_remap_buffer_texture_set_and_binding(&self) -> SetBindingPair {
        SetBindingPair {
            set: TEXTURE_SET_INDEX,
            binding: self.topology_remap_buffer_texture_binding,
        }
    }

    /// `TryGetOffset(kind, UserDefined, location, component)`.
    pub fn try_get_offset_user(
        &self,
        kind: StorageKind,
        location: i32,
        component: i32,
    ) -> Option<i32> {
        self.try_get_offset(kind, IoVariable::UserDefined, location, component)
    }

    /// `TryGetOffset(kind, variable, location, component)`.
    pub fn try_get_offset(
        &self,
        kind: StorageKind,
        var: IoVariable,
        location: i32,
        component: i32,
    ) -> Option<i32> {
        self.offsets
            .get(&IoDefinition::new(kind, var, location, component))
            .copied()
    }

    /// `TryGetOffset(kind, variable, component)`.
    pub fn try_get_offset_component(
        &self,
        kind: StorageKind,
        var: IoVariable,
        component: i32,
    ) -> Option<i32> {
        self.try_get_offset(kind, var, 0, component)
    }

    /// `TryGetOffset(kind, variable)`.
    pub fn try_get_offset_var(&self, kind: StorageKind, var: IoVariable) -> Option<i32> {
        self.try_get_offset(kind, var, 0, 0)
    }

    /// `Offsets` (unordered: only looked up in the C#).
    pub fn offsets(&self) -> &HashMap<IoDefinition, i32> {
        &self.offsets
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts() {
        let c = ResourceReservationCounts::new(false, false);
        assert_eq!(
            (
                c.reserved_constant_buffers,
                c.reserved_storage_buffers,
                c.reserved_textures
            ),
            (1, 0, 0)
        );
        let c = ResourceReservationCounts::new(true, true);
        assert_eq!(
            (
                c.reserved_constant_buffers,
                c.reserved_storage_buffers,
                c.reserved_textures
            ),
            (2, 7, 34)
        );
        let r = ResourceReservations::new_counts(true, true);
        assert_eq!(r.vertex_output_storage_buffer_binding, 4);
        assert_eq!(r.get_vertex_buffer_texture_binding(3), 5);
    }
}
