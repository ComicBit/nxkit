//! `ResourceManager.GetTextureOrImageBinding`.

use super::props::*;
use super::resources::*;
use crate::accessor::Accessor;
use crate::ir::Inst;
use crate::types::SamplerType;

impl ResourceManager {
    /// `GetTextureOrImageBinding(inst, type, format, flags, cbufSlot, handle, arrayLength, separate)`.
    /// `format` is an index into [`crate::types::TEXTURE_FORMATS`].
    #[allow(clippy::too_many_arguments)]
    pub fn get_texture_or_image_binding(
        &mut self,
        acc: &mut Accessor,
        inst: Inst,
        ty: SamplerType,
        format: usize,
        flags: TextureFlags,
        cbuf_slot: i32,
        handle: i32,
        array_length: i32,
        separate: bool,
    ) -> Result<SetBindingPair, String> {
        let inst = Inst(inst.0 & 0xffff);
        let is_image = matches!(inst, Inst::ImageAtomic | Inst::ImageLoad | Inst::ImageStore);
        let is_write = matches!(inst, Inst::ImageAtomic | Inst::ImageStore);
        let accurate_type = !matches!(
            inst,
            Inst::Lod | Inst::TextureQuerySamples | Inst::TextureQuerySize
        );
        let int_coords =
            is_image || flags.has_flag(TextureFlags::INT_COORDS) || inst == Inst::TextureQuerySize;
        let coherent = flags.has_flag(TextureFlags::COHERENT);
        let format = if is_image { format } else { 0 };
        let r = self.get_texture_or_image_binding_inner(
            cbuf_slot,
            handle,
            array_length,
            ty,
            format,
            is_image,
            int_coords,
            is_write,
            accurate_type,
            coherent,
            separate,
            acc,
        )?;
        super::props::ResourceQueries::register_texture(acc, handle, cbuf_slot);
        Ok(r)
    }

    #[allow(clippy::too_many_arguments)]
    fn get_texture_or_image_binding_inner(
        &mut self,
        cbuf_slot: i32,
        handle: i32,
        array_length: i32,
        ty: SamplerType,
        format: usize,
        is_image: bool,
        int_coords: bool,
        write: bool,
        accurate_type: bool,
        coherent: bool,
        separate: bool,
        acc: &mut Accessor,
    ) -> Result<SetBindingPair, String> {
        let dimensions = if ty == SamplerType::NONE {
            0
        } else {
            dimensions(ty)?
        };
        let mut usage_flags = TextureUsageFlags::NONE;
        if int_coords {
            usage_flags |= TextureUsageFlags::NEEDS_SCALE_VALUE;
            let can_scale = self.stage.supports_render_scale()
                && array_length == 1
                && !write
                && dimensions == 2;
            if !can_scale {
                usage_flags |= TextureUsageFlags::RES_SCALE_UNSUPPORTED;
            }
        }
        if write {
            usage_flags |= TextureUsageFlags::IMAGE_STORE;
        }
        if coherent {
            usage_flags |= TextureUsageFlags::IMAGE_COHERENT;
        }

        // arrays also key on the type: handles in one buffer may have different types
        let key_type = if array_length > 1 {
            ty
        } else {
            SamplerType::NONE
        };
        let info = TextureInfo {
            cbuf_slot,
            handle,
            array_length,
            separate,
            ty: key_type,
            format,
        };
        let mut meta = TextureMeta {
            set: 0,
            binding: 0,
            accurate_type,
            ty,
            usage_flags,
        };

        let existing = if is_image {
            self.used_images.get(&info)
        } else {
            self.used_textures.get(&info)
        }
        .copied();
        let (set_index, binding);
        if let Some(ex) = existing {
            let merged = merge_texture_meta(meta, ex);
            if is_image {
                self.used_images.insert(info, merged);
            } else {
                self.used_textures.insert(info, merged);
            }
            set_index = ex.set;
            binding = ex.binding;
        } else {
            let extra = if array_length > 1 {
                super::props::ResourceQueries::create_extra_set(acc)
            } else {
                -1
            };
            if array_length > 1 && extra >= 0 {
                // an extra set reserved for the array: the binding is always 0
                set_index = extra;
                binding = 0;
            } else {
                let is_buffer = (ty.0 & SamplerType::MASK) == SamplerType::TEXTURE_BUFFER.0;
                let sb = if is_image {
                    acc.create_image_binding(array_length, is_buffer)
                } else {
                    acc.create_texture_binding(array_length, is_buffer)
                };
                set_index = sb.set;
                binding = sb.binding;
            }
            meta.set = set_index;
            meta.binding = binding;
            if is_image {
                self.used_images.insert(info, meta);
            } else {
                self.used_textures.insert(info, meta);
            }
        }

        let mut prefix = (if is_image { "i" } else { "t" }).to_string();
        if array_length != 1 && ty != SamplerType::NONE {
            prefix.push_str(&short_type_name(ty)?);
        }
        let name_suffix = if is_image {
            let f = to_glsl_format(format);
            if cbuf_slot < 0 {
                format!("{prefix}_tcb_{handle:X}_{f}")
            } else {
                format!("{prefix}_cb{cbuf_slot}_{handle:X}_{f}")
            }
        } else if ty == SamplerType::NONE {
            if cbuf_slot < 0 {
                format!("s_tcb_{handle:X}")
            } else {
                format!("s_cb{cbuf_slot}_{handle:X}")
            }
        } else if cbuf_slot < 0 {
            format!("{prefix}_tcb_{handle:X}")
        } else {
            format!("{prefix}_cb{cbuf_slot}_{handle:X}")
        };

        // the definition uses the new meta's type and flags, not the merged ones (as the C# does)
        let definition = TextureDefinition {
            set: set_index,
            binding,
            array_length,
            separate,
            name: format!("{}_{}", self.stage_prefix, name_suffix),
            ty: meta.ty,
            format: info.format,
            flags: meta.usage_flags,
        };
        if is_image {
            self.properties.add_or_update_image(definition);
        } else {
            self.properties.add_or_update_texture(definition);
        }
        Ok(SetBindingPair {
            set: set_index,
            binding,
        })
    }
}
