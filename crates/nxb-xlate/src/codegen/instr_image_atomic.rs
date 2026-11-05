//! `Instructions.GenerateImageAtomic` (the Metal-bounded form: out of range coordinates leave memory alone).

use super::context::*;
use super::instr_helpers::mark_non_uniform;
use super::instr_texture::{Srcs, assemble_coords, dimensions, tex};
use super::spirv_enums::{selection_control_mask, storage_class};
use crate::spv::Lit;
use crate::structured::ast::*;
use crate::structured::types::{AggType, format_component_type};
use crate::types::SamplerType;

type R<T> = Result<T, String>;

const ATOMIC_MASK: u32 = 15 << 16;
const BOUNDS_MASK: u32 = 3 << 20;
const DISCARD_RESULT: u32 = 1 << 22;
const PREDICATE_RESULT: u32 = 1 << 24;
const FORMAT_R32_UINT: usize = 11;
const FORMAT_R32_SINT: usize = 12;

pub fn image_atomic(ctx: &mut CodeGenContext, o: &OpData) -> R<OpResult> {
    let t = tex(o)?.clone();
    let component_type = format_component_type(t.format);
    let is_array = t.ty.0 & SamplerType::ARRAY != 0;
    let mut s = Srcs { o, i: 0 };
    let decl = *ctx
        .images
        .get(&(t.set, t.binding))
        .ok_or("image not declared")?;
    let mut image = decl.image;
    let bounded = ctx.host.metal_image_atomics;
    if bounded && (t.flags & BOUNDS_MASK != 0 || t.flags & PREDICATE_RESULT != 0) {
        return Err(
            "Metal image atomics require IGN without a predicate result; TRAP semantics are not implemented."
                .into(),
        );
    }
    let ty = t.ty;
    if bounded
        && (decl.is_indexed
            || (ty != SamplerType::TEXTURE_BUFFER
                && ty != SamplerType::TEXTURE_2D
                && ty.0 != (SamplerType::TEXTURE_2D.0 | SamplerType::ARRAY)
                && ty != SamplerType::TEXTURE_3D)
            || (t.format != FORMAT_R32_UINT && t.format != FORMAT_R32_SINT))
    {
        return Err(
            "Metal image atomics support non-indexed 32-bit buffer/2D/2D-array/3D images only."
                .into(),
        );
    }
    let result_type = ctx.ty(component_type)?;
    let image_pointer_type = ctx.type_pointer(storage_class::Image, result_type, false);
    if decl.is_indexed {
        let idx = s.next(ctx, AggType::S32)?;
        image = ctx.access_chain(image_pointer_type, image, &[idx]);
        mark_non_uniform(ctx, image);
    }
    let p_count = dimensions(t.ty)? + is_array as i32;
    let p_coords = assemble_coords(ctx, &mut s, p_count, AggType::S32)?;
    let value = s.next(ctx, component_type)?;

    let mut merge = None;
    let mut valid = None;
    let mut invalid = None;
    if bounded {
        let uint_type = ctx.type_u32();
        let coord_type = if p_count == 1 {
            uint_type
        } else {
            ctx.type_vector(uint_type, Lit::I32(p_count), false)
        };
        let bool_type = ctx.type_bool(false);
        let compare_type = if p_count == 1 {
            bool_type
        } else {
            ctx.type_vector(bool_type, Lit::I32(p_count), false)
        };
        let coords = ctx.bitcast(coord_type, p_coords);
        let loaded = ctx.load(decl.image_type, image, None);
        let extent = ctx.image_query_size(coord_type, loaded);
        let mut inside = ctx.u_less_than(compare_type, coords, extent);
        if p_count != 1 {
            inside = ctx.all(bool_type, inside);
        }
        let v = ctx.label();
        let m = ctx.label();
        let inv = if t.flags & DISCARD_RESULT != 0 {
            m
        } else {
            ctx.label()
        };
        ctx.selection_merge(m, selection_control_mask::MaskNone);
        ctx.branch_conditional(inside, v, inv, &[]);
        ctx.add_label(v);
        merge = Some(m);
        valid = Some(v);
        invalid = Some(inv);
    }
    let u32t = ctx.type_u32();
    let c0 = ctx.constant(u32t, Lit::I32(0));
    let pointer = ctx.image_texel_pointer(image_pointer_type, image, p_coords, c0);
    let u32t = ctx.type_u32();
    let one = ctx.constant(u32t, Lit::I32(1));
    let u32t = ctx.type_u32();
    let zero = ctx.constant(u32t, Lit::I32(0));
    let signed = component_type == AggType::S32;
    let mut result = match t.flags & ATOMIC_MASK {
        m if m == 1 << 16 => {
            if signed {
                ctx.atomic_s_min(result_type, pointer, one, zero, value)
            } else {
                ctx.atomic_u_min(result_type, pointer, one, zero, value)
            }
        }
        m if m == 2 << 16 => {
            if signed {
                ctx.atomic_s_max(result_type, pointer, one, zero, value)
            } else {
                ctx.atomic_u_max(result_type, pointer, one, zero, value)
            }
        }
        m if m == 3 << 16 => ctx.atomic_i_increment(result_type, pointer, one, zero),
        m if m == 4 << 16 => ctx.atomic_i_decrement(result_type, pointer, one, zero),
        m if m == 5 << 16 => ctx.atomic_and(result_type, pointer, one, zero, value),
        m if m == 6 << 16 => ctx.atomic_or(result_type, pointer, one, zero, value),
        m if m == 7 << 16 => ctx.atomic_xor(result_type, pointer, one, zero, value),
        m if m == 8 << 16 => ctx.atomic_exchange(result_type, pointer, one, zero, value),
        m if m == 9 << 16 => {
            let cmp = s.next(ctx, component_type)?;
            ctx.atomic_compare_exchange(result_type, pointer, one, zero, zero, cmp, value)
        }
        _ => ctx.atomic_i_add(result_type, pointer, one, zero, value),
    };
    if bounded {
        let (merge, valid, invalid) = (
            merge.ok_or("merge")?,
            valid.ok_or("valid")?,
            invalid.ok_or("invalid")?,
        );
        ctx.branch(merge);
        if t.flags & DISCARD_RESULT != 0 {
            ctx.add_label(merge);
            result = ctx.constant(result_type, Lit::I32(0));
        } else {
            ctx.add_label(invalid);
            ctx.branch(merge);
            ctx.add_label(merge);
            let z = ctx.constant(result_type, Lit::I32(0));
            result = ctx.phi(result_type, &[result, valid, z, invalid]);
        }
    }
    Ok(OpResult::new(component_type, result))
}
