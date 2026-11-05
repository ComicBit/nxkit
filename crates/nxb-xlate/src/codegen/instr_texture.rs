//! Texture and image instructions of `Instructions` (sampling, image load and store, queries).

use super::context::*;
use super::instr_helpers::{get_swizzled_result, mark_non_uniform};
use super::spirv_enums::{image_operands_mask as iom, storage_class};
use super::spirv_op::op;
use crate::attr::Stage;
use crate::spv::{Inst as SInst, Lit};
use crate::structured::ast::*;
use crate::structured::types::{AggType, format_component_type};
use crate::types::SamplerType;

type R<T> = Result<T, String>;

const FLAG_GATHER: u32 = 1 << 1;
const FLAG_DERIVATIVES: u32 = 1 << 2;
const FLAG_INT_COORDS: u32 = 1 << 3;
const FLAG_LOD_BIAS: u32 = 1 << 4;
const FLAG_LOD_LEVEL: u32 = 1 << 5;
const FLAG_OFFSET: u32 = 1 << 6;
const FLAG_OFFSETS: u32 = 1 << 7;

/// `SamplerType.Dimensions`.
pub(super) fn dimensions(t: SamplerType) -> R<i32> {
    Ok(match t.0 & SamplerType::MASK {
        1 | 2 => 1,
        3 => 2,
        4 | 5 => 3,
        _ => return Err(format!("Invalid sampler type \"{}\".", t.0)),
    })
}

fn vector_type(o: &OpData, scalar: AggType) -> AggType {
    let c = (o.index as u32).count_ones();
    let mut t = scalar;
    match c {
        2 => t |= AggType::VECTOR2,
        3 => t |= AggType::VECTOR3,
        4 => t |= AggType::VECTOR4,
        _ => {}
    }
    t
}

/// A source reader: `Src(type)` with a running index.
pub(super) struct Srcs<'a> {
    pub(super) o: &'a OpData,
    pub(super) i: usize,
}

impl Srcs<'_> {
    pub(super) fn next(&mut self, ctx: &mut CodeGenContext, ty: AggType) -> R<SInst> {
        let s = *self.o.sources.get(self.i).ok_or("missing texture source")?;
        self.i += 1;
        ctx.get(ty, s)
    }
}

pub(super) fn tex(o: &OpData) -> R<&AstTex> {
    o.tex
        .as_ref()
        .ok_or_else(|| "not a texture operation".to_string())
}

pub(super) fn assemble_coords(
    ctx: &mut CodeGenContext,
    s: &mut Srcs,
    count: i32,
    ty: AggType,
) -> R<SInst> {
    if count > 1 {
        let mut elems = Vec::new();
        for _ in 0..count {
            elems.push(s.next(ctx, ty)?);
        }
        let et = ctx.ty(ty)?;
        let vt = ctx.type_vector(et, Lit::I32(count), false);
        Ok(ctx.composite_construct(vt, &elems))
    } else {
        s.next(ctx, ty)
    }
}

pub fn image_load(ctx: &mut CodeGenContext, o: &OpData) -> R<OpResult> {
    let t = tex(o)?.clone();
    let component_type = format_component_type(t.format);
    let is_array = t.ty.0 & SamplerType::ARRAY != 0;
    let mut s = Srcs { o, i: 0 };
    let decl = *ctx
        .images
        .get(&(t.set, t.binding))
        .ok_or("image not declared")?;
    let mut image = decl.image;
    let is_indexed = decl.is_indexed;
    if is_indexed {
        let idx = s.next(ctx, AggType::S32)?;
        image = ctx.access_chain(decl.image_pointer_type, image, &[idx]);
        mark_non_uniform(ctx, image);
    }
    image = ctx.load(decl.image_type, image, None);
    if is_indexed {
        mark_non_uniform(ctx, image);
    }
    let p_count = dimensions(t.ty)? + is_array as i32;
    let p_coords = assemble_coords(ctx, &mut s, p_count, AggType::S32)?;
    let ict = ctx.ty(component_type)?;
    let v4 = ctx.type_vector(ict, Lit::I32(4), false);
    let swizzled_result_type = vector_type(o, component_type);
    let texel = ctx.image_read(v4, image, p_coords, iom::MaskNone, &[]);
    let result = get_swizzled_result(ctx, texel, swizzled_result_type, o.index)?;
    Ok(OpResult::new(component_type, result))
}

pub fn image_store(ctx: &mut CodeGenContext, o: &OpData) -> R<OpResult> {
    let t = tex(o)?.clone();
    let is_array = t.ty.0 & SamplerType::ARRAY != 0;
    let mut s = Srcs { o, i: 0 };
    let decl = *ctx
        .images
        .get(&(t.set, t.binding))
        .ok_or("image not declared")?;
    let mut image = decl.image;
    let is_indexed = decl.is_indexed;
    if is_indexed {
        let idx = s.next(ctx, AggType::S32)?;
        image = ctx.access_chain(decl.image_pointer_type, image, &[idx]);
        mark_non_uniform(ctx, image);
    }
    image = ctx.load(decl.image_type, image, None);
    if is_indexed {
        mark_non_uniform(ctx, image);
    }
    let p_count = dimensions(t.ty)? + is_array as i32;
    let p_coords = assemble_coords(ctx, &mut s, p_count, AggType::S32)?;
    let component_type = format_component_type(t.format);
    let mut c_elems = Vec::new();
    for _ in 0..4 {
        if s.i < o.sources.len() {
            c_elems.push(s.next(ctx, component_type)?);
        } else {
            let e = match component_type {
                AggType::S32 => {
                    let ty = ctx.type_s32();
                    ctx.constant(ty, Lit::I32(0))
                }
                AggType::U32 => {
                    let ty = ctx.type_u32();
                    ctx.constant(ty, Lit::U32(0))
                }
                _ => {
                    let ty = ctx.type_fp32();
                    ctx.constant(ty, Lit::f32(0.0))
                }
            };
            c_elems.push(e);
        }
    }
    let ct = ctx.ty(component_type)?;
    let vt = ctx.type_vector(ct, Lit::I32(4), false);
    let texel = ctx.composite_construct(vt, &c_elems);
    ctx.image_write(image, p_coords, texel, iom::MaskNone, &[]);
    Ok(OpResult::INVALID)
}

fn sampled_image_load(
    ctx: &mut CodeGenContext,
    t: &AstTex,
    decl: SamplerDecl,
    s: &mut Srcs,
) -> R<SInst> {
    let mut image = decl.image;
    let image_indexed = decl.is_indexed;
    if image_indexed {
        let idx = s.next(ctx, AggType::S32)?;
        image = ctx.access_chain(decl.sampled_image_pointer_type, image, &[idx]);
        mark_non_uniform(ctx, image);
    }
    if t.is_separate() {
        image = ctx.load(decl.image_type, image, None);
        if image_indexed {
            mark_non_uniform(ctx, image);
        }
        let sd = *ctx
            .samplers
            .get(&(t.sampler_set, t.sampler_binding))
            .ok_or("sampler not declared")?;
        let mut sampler = sd.image;
        let sampler_indexed = sd.is_indexed;
        if sampler_indexed {
            let idx = s.next(ctx, AggType::S32)?;
            sampler = ctx.access_chain(sd.sampled_image_pointer_type, sampler, &[idx]);
            mark_non_uniform(ctx, sampler);
        }
        sampler = ctx.load(sd.image_type, sampler, None);
        if sampler_indexed {
            mark_non_uniform(ctx, sampler);
        }
        image = ctx.sampled_image(decl.sampled_image_type, image, sampler);
        if image_indexed || sampler_indexed {
            mark_non_uniform(ctx, image);
        }
    } else {
        image = ctx.load(decl.sampled_image_type, image, None);
        if image_indexed {
            mark_non_uniform(ctx, image);
        }
    }
    Ok(image)
}

pub fn lod(ctx: &mut CodeGenContext, o: &OpData) -> R<OpResult> {
    let t = tex(o)?.clone();
    let mut s = Srcs { o, i: 0 };
    let decl = *ctx
        .samplers
        .get(&(t.set, t.binding))
        .ok_or("sampler not declared")?;
    let image = sampled_image_load(ctx, &t, decl, &mut s)?;
    let p_count = dimensions(t.ty)?;
    let p_coords = assemble_coords(ctx, &mut s, p_count, AggType::FP32)?;
    let f = ctx.type_fp32();
    let rt = ctx.type_vector(f, Lit::I32(2), false);
    let packed = ctx.image_query_lod(rt, image, p_coords);
    let tf = ctx.type_fp32();
    let result = ctx.composite_extract(tf, packed, &[Lit::I32(o.index)]);
    Ok(OpResult::new(AggType::FP32, result))
}

pub fn query_samples(ctx: &mut CodeGenContext, o: &OpData) -> R<OpResult> {
    let t = tex(o)?.clone();
    let mut s = Srcs { o, i: 0 };
    let decl = *ctx
        .samplers
        .get(&(t.set, t.binding))
        .ok_or("sampler not declared")?;
    let mut image = sampled_image_load(ctx, &t, decl, &mut s)?;
    image = ctx.image(decl.image_type, image);
    let ts = ctx.type_s32();
    Ok(OpResult::new(
        AggType::S32,
        ctx.image_query_samples(ts, image),
    ))
}

pub fn query_size(ctx: &mut CodeGenContext, o: &OpData) -> R<OpResult> {
    let t = tex(o)?.clone();
    let mut s = Srcs { o, i: 0 };
    let decl = *ctx
        .samplers
        .get(&(t.set, t.binding))
        .ok_or("sampler not declared")?;
    let mut image = sampled_image_load(ctx, &t, decl, &mut s)?;
    image = ctx.image(decl.image_type, image);
    if o.index == 3 {
        let ts = ctx.type_s32();
        return Ok(OpResult::new(
            AggType::S32,
            ctx.image_query_levels(ts, image),
        ));
    }
    let ty = *ctx
        .samplers_types
        .get(&(t.set, t.binding))
        .ok_or("sampler type missing")?;
    let has_lod = ty.0 & SamplerType::MULTISAMPLE == 0 && ty != SamplerType::TEXTURE_BUFFER;
    let mut dims = if ty.0 & SamplerType::MASK == SamplerType::TEXTURE_CUBE.0 {
        2
    } else {
        dimensions(ty)?
    };
    if ty.0 & SamplerType::ARRAY != 0 {
        dims += 1;
    }
    let rt = if dims == 1 {
        ctx.type_s32()
    } else {
        let ts = ctx.type_s32();
        ctx.type_vector(ts, Lit::I32(dims), false)
    };
    let mut result;
    if has_lod {
        let src = *o.sources.get(s.i).ok_or("missing lod source")?;
        let lod = ctx.get_s32(src)?;
        result = ctx.image_query_size_lod(rt, image, lod);
    } else {
        result = ctx.image_query_size(rt, image);
    }
    if dims != 1 {
        let ts = ctx.type_s32();
        result = ctx.composite_extract(ts, result, &[Lit::I32(o.index)]);
    }
    Ok(OpResult::new(AggType::S32, result))
}

#[allow(clippy::needless_late_init)]
pub fn texture_sample(ctx: &mut CodeGenContext, o: &OpData) -> R<OpResult> {
    let t = tex(o)?.clone();
    let flags = t.flags;
    let is_gather = flags & FLAG_GATHER != 0;
    let has_derivatives = flags & FLAG_DERIVATIVES != 0;
    let int_coords = flags & FLAG_INT_COORDS != 0;
    let mut has_lod_bias = flags & FLAG_LOD_BIAS != 0;
    let mut has_lod_level = flags & FLAG_LOD_LEVEL != 0;
    let has_offset = flags & FLAG_OFFSET != 0;
    let has_offsets = flags & FLAG_OFFSETS != 0;
    let is_array = t.ty.0 & SamplerType::ARRAY != 0;
    let is_multisample = t.ty.0 & SamplerType::MULTISAMPLE != 0;
    let is_shadow = t.ty.0 & SamplerType::SHADOW != 0;
    let mut s = Srcs { o, i: 0 };
    let decl = *ctx
        .samplers
        .get(&(t.set, t.binding))
        .ok_or("sampler not declared")?;
    let mut image = sampled_image_load(ctx, &t, decl, &mut s)?;
    let coords_count = dimensions(t.ty)?;
    let mut p_count = coords_count;
    let mut array_index_elem = -1;
    if is_array {
        array_index_elem = p_count;
        p_count += 1;
    }
    let coord_type = if int_coords {
        AggType::S32
    } else {
        AggType::FP32
    };

    // AssemblePVector
    let mut p_coords = if p_count > 1 {
        let mut elems = Vec::new();
        for index in 0..p_count {
            if array_index_elem == index {
                let mut e = s.next(ctx, AggType::S32)?;
                if !int_coords {
                    let tf = ctx.type_fp32();
                    e = ctx.convert_s_to_f(tf, e);
                }
                elems.push(e);
            } else {
                elems.push(s.next(ctx, coord_type)?);
            }
        }
        let et = if int_coords {
            ctx.type_s32()
        } else {
            ctx.type_fp32()
        };
        let vt = ctx.type_vector(et, Lit::I32(p_count), false);
        ctx.composite_construct(vt, &elems)
    } else {
        s.next(ctx, coord_type)?
    };
    if !int_coords {
        let nf_fp32 = ctx.type_fp32();
        let nf_type = if p_count > 1 {
            ctx.type_vector(nf_fp32, Lit::I32(p_count), false)
        } else {
            nf_fp32
        };
        let mut nf_zero = ctx.constant(nf_fp32, Lit::f32(0.0));
        if p_count > 1 {
            let elems = vec![nf_zero; p_count as usize];
            nf_zero = ctx.constant_composite(nf_type, &elems);
        }
        let nf_bool = if p_count > 1 {
            let tb = ctx.type_bool(false);
            ctx.type_vector(tb, Lit::I32(p_count), false)
        } else {
            ctx.type_bool(false)
        };
        let nan = ctx.is_nan(nf_bool, p_coords);
        p_coords = ctx.select(nf_type, nan, nf_zero, p_coords);
    }
    let assemble_derivatives = |ctx: &mut CodeGenContext, s: &mut Srcs, count: i32| -> R<SInst> {
        assemble_coords(ctx, s, count, AggType::FP32)
    };
    let mut d_ref = None;
    if is_shadow {
        d_ref = Some(s.next(ctx, AggType::FP32)?);
    }
    let mut derivatives: Vec<SInst> = Vec::new();
    if has_derivatives {
        derivatives.push(assemble_derivatives(ctx, &mut s, coords_count)?);
        derivatives.push(assemble_derivatives(ctx, &mut s, coords_count)?);
    }
    let mut sample = None;
    let mut lod = None;
    if is_multisample {
        sample = Some(s.next(ctx, AggType::S32)?);
    } else if has_lod_level {
        lod = Some(s.next(ctx, coord_type)?);
    }
    let mut offsets_const = true;
    let mut assemble_offset = |ctx: &mut CodeGenContext, s: &mut Srcs, count: i32| -> R<SInst> {
        if count > 1 {
            let mut elems = Vec::new();
            for _ in 0..count {
                elems.push(s.next(ctx, AggType::S32)?);
            }
            let ts = ctx.type_s32();
            let vt = ctx.type_vector(ts, Lit::I32(count), false);
            if elems
                .iter()
                .any(|e| ctx.m.opcode(*e) != op::OpConstant as u16)
            {
                offsets_const = false;
                return Ok(ctx.composite_construct(vt, &elems));
            }
            Ok(ctx.constant_composite(vt, &elems))
        } else {
            let elem = s.next(ctx, AggType::S32)?;
            if ctx.m.opcode(elem) != op::OpConstant as u16 {
                offsets_const = false;
            }
            Ok(elem)
        }
    };
    let mut offsets: Vec<SInst> = Vec::new();
    if has_offset {
        offsets.push(assemble_offset(ctx, &mut s, coords_count)?);
    } else if has_offsets {
        for _ in 0..4 {
            offsets.push(assemble_offset(ctx, &mut s, coords_count)?);
        }
    }
    let mut lod_bias = None;
    if has_lod_bias {
        lod_bias = Some(s.next(ctx, AggType::FP32)?);
    }
    if !is_gather
        && !int_coords
        && !is_multisample
        && !has_lod_level
        && !has_derivatives
        && ctx.definitions.stage()? != Stage::Fragment
    {
        // implicit LOD is only valid on fragment: the LOD bias becomes the explicit LOD
        lod = Some(match lod_bias {
            Some(b) => b,
            None => {
                let tf = ctx.type_fp32();
                ctx.constant(tf, Lit::f32(0.0))
            }
        });
        lod_bias = None;
        has_lod_bias = false;
        has_lod_level = true;
    }
    let mut comp_idx = None;
    if is_gather && !is_shadow {
        comp_idx = Some(s.next(ctx, AggType::S32)?);
    }
    let mut operands_list: Vec<SInst> = Vec::new();
    let mut mask = iom::MaskNone;
    if has_lod_bias {
        mask |= iom::Bias;
        operands_list.push(lod_bias.ok_or("lod bias")?);
    }
    if !is_multisample && has_lod_level {
        mask |= iom::Lod;
        operands_list.push(lod.ok_or("lod")?);
    }
    if has_derivatives {
        mask |= iom::Grad;
        operands_list.push(derivatives[0]);
        operands_list.push(derivatives[1]);
    }
    if has_offset {
        mask |= if offsets_const {
            iom::ConstOffset
        } else {
            iom::Offset
        };
        operands_list.push(offsets[0]);
    } else if has_offsets && offsets_const {
        mask |= iom::ConstOffsets;
        let ts = ctx.type_s32();
        let v2 = ctx.type_vector(ts, Lit::I32(2), false);
        let tu = ctx.type_u32();
        let four = ctx.constant(tu, Lit::I32(4));
        let arrayv2 = ctx.type_array(v2, four, false);
        operands_list.push(ctx.constant_composite(arrayv2, &offsets[..4]));
    }
    if is_multisample {
        mask |= iom::Sample;
        operands_list.push(sample.ok_or("sample")?);
    }
    let color_is_vector = is_gather || !is_shadow;
    let result_type = if color_is_vector {
        let tf = ctx.type_fp32();
        ctx.type_vector(tf, Lit::I32(4), false)
    } else {
        ctx.type_fp32()
    };
    if int_coords {
        image = ctx.image(decl.image_type, image);
    }
    let operands = operands_list;
    let result;
    if int_coords {
        result = ctx.image_fetch(result_type, image, p_coords, mask, &operands);
    } else if is_gather && has_offsets && !offsets_const {
        let mut texels = Vec::new();
        for off in offsets.iter().take(4) {
            let mut gather_operands = operands.clone();
            gather_operands.push(*off);
            let gather_mask = mask | iom::Offset;
            let texel4 = if is_shadow {
                ctx.image_dref_gather(
                    result_type,
                    image,
                    p_coords,
                    d_ref.ok_or("dref")?,
                    gather_mask,
                    &gather_operands,
                )
            } else {
                ctx.image_gather(
                    result_type,
                    image,
                    p_coords,
                    comp_idx.ok_or("component")?,
                    gather_mask,
                    &gather_operands,
                )
            };
            let tf = ctx.type_fp32();
            texels.push(ctx.composite_extract(tf, texel4, &[Lit::I32(3)]));
        }
        result = ctx.composite_construct(result_type, &texels);
    } else if is_gather {
        result = if is_shadow {
            ctx.image_dref_gather(
                result_type,
                image,
                p_coords,
                d_ref.ok_or("dref")?,
                mask,
                &operands,
            )
        } else {
            ctx.image_gather(
                result_type,
                image,
                p_coords,
                comp_idx.ok_or("component")?,
                mask,
                &operands,
            )
        };
    } else if is_shadow {
        let d = d_ref.ok_or("dref")?;
        result = if has_lod_level {
            ctx.image_sample_dref_explicit_lod(result_type, image, p_coords, d, mask, &operands)
        } else {
            ctx.image_sample_dref_implicit_lod(result_type, image, p_coords, d, mask, &operands)
        };
    } else if has_derivatives || has_lod_level {
        result = ctx.image_sample_explicit_lod(result_type, image, p_coords, mask, &operands);
    } else {
        result = ctx.image_sample_implicit_lod(result_type, image, p_coords, mask, &operands);
    }
    let mut swizzled_result_type = AggType::FP32;
    let mut result = result;
    if color_is_vector {
        swizzled_result_type = vector_type(o, swizzled_result_type);
        result = get_swizzled_result(ctx, result, swizzled_result_type, o.index)?;
    }
    let _ = storage_class::Image;
    Ok(OpResult::new(swizzled_result_type, result))
}
