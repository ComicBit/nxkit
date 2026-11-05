//! `Instructions`: one SPIR-V generator per IR instruction (dispatch and the simple ones).

use super::context::*;
use super::instr_helpers::*;
use super::spirv_enums::{memory_semantics_mask as sem, scope};
use crate::ir::{Inst, OperandType};
use crate::spv::{Inst as SInst, Lit};
use crate::structured::ast::*;
use crate::structured::types::AggType;

type R<T> = Result<T, String>;

const DEFAULT_MEMORY_SEMANTICS: u32 = sem::ImageMemory
    | sem::AtomicCounterMemory
    | sem::WorkgroupMemory
    | sem::UniformMemory
    | sem::AcquireRelease;

fn src(o: &OpData, i: usize) -> R<A> {
    o.sources
        .get(i)
        .copied()
        .ok_or_else(|| "missing source".to_string())
}

/// `Instructions.Generate`.
pub fn generate(ctx: &mut CodeGenContext, o: &OpData) -> R<OpResult> {
    use CodeGenContext as C;
    let inst = Inst(o.inst.0 & Inst::MASK);
    match inst {
        Inst::Absolute => unary(ctx, o, C::glsl_f_abs, Some(C::glsl_s_abs)),
        Inst::Add => binary(ctx, o, C::f_add, Some(C::i_add)),
        Inst::AtomicAdd
        | Inst::AtomicAnd
        | Inst::AtomicCompareAndSwap
        | Inst::AtomicMinS32
        | Inst::AtomicMinU32
        | Inst::AtomicMaxS32
        | Inst::AtomicMaxU32
        | Inst::AtomicOr
        | Inst::AtomicSwap
        | Inst::AtomicXor => atomic_memory(ctx, o, inst),
        Inst::Ballot => {
            let source = src(o, 0)?;
            let u = ctx.type_u32();
            let uvec4 = ctx.type_vector(u, Lit::I32(4), false);
            let tu = ctx.type_u32();
            let execution = ctx.constant(tu, Lit::I32(scope::Subgroup as i32));
            let pred = ctx.get(AggType::BOOL, source)?;
            let mask_vector = ctx.group_non_uniform_ballot(uvec4, execution, pred);
            let tu = ctx.type_u32();
            let mask = ctx.composite_extract(tu, mask_vector, &[Lit::I32(o.index)]);
            Ok(OpResult::new(AggType::U32, mask))
        }
        Inst::Barrier => {
            let cur = ctx.current_block.ok_or("no current block")?;
            if !ctx.host.supports_shader_barrier_divergence
                && (ctx.ast.block_type(cur) != Some(BlockType::Main)
                    || ctx.may_have_returned
                    || !ctx.is_main_function)
            {
                return Ok(OpResult::INVALID);
            }
            control_barrier(
                ctx,
                scope::Workgroup,
                sem::WorkgroupMemory | sem::AcquireRelease,
            );
            Ok(OpResult::INVALID)
        }
        Inst::SubgroupBarrier => {
            control_barrier(
                ctx,
                scope::Subgroup,
                sem::WorkgroupMemory | sem::AcquireRelease,
            );
            Ok(OpResult::INVALID)
        }
        Inst::BitCount => unary_s32(ctx, o, C::bit_count),
        Inst::BitfieldExtractS32 => {
            let (s1, s2, s3) = (src(o, 0)?, src(o, 1)?, src(o, 2)?);
            let t = ctx.type_s32();
            let a = ctx.get_s32(s1)?;
            let b = ctx.get_u32(s2)?;
            let c = ctx.get_u32(s3)?;
            let r = ctx.bit_field_s_extract(t, a, b, c);
            Ok(OpResult::new(AggType::S32, r))
        }
        Inst::BitfieldExtractU32 => ternary_u32(ctx, o, C::bit_field_u_extract),
        Inst::BitfieldInsert => {
            let (s1, s2, s3, s4) = (src(o, 0)?, src(o, 1)?, src(o, 2)?, src(o, 3)?);
            let t = ctx.type_u32();
            let a = ctx.get_u32(s1)?;
            let b = ctx.get_u32(s2)?;
            let c = ctx.get_u32(s3)?;
            let d = ctx.get_u32(s4)?;
            let r = ctx.bit_field_insert(t, a, b, c, d);
            Ok(OpResult::new(AggType::U32, r))
        }
        Inst::BitfieldReverse => unary_s32(ctx, o, C::bit_reverse),
        Inst::BitwiseAnd => binary_s32(ctx, o, C::bitwise_and),
        Inst::BitwiseExclusiveOr => binary_s32(ctx, o, C::bitwise_xor),
        Inst::BitwiseNot => unary_s32(ctx, o, C::not),
        Inst::BitwiseOr => binary_s32(ctx, o, C::bitwise_or),
        Inst::Call => generate_call(ctx, o),
        Inst::Ceiling => unary(ctx, o, C::glsl_ceil, None),
        Inst::Clamp => ternary(ctx, o, C::glsl_f_clamp, Some(C::glsl_s_clamp)),
        Inst::ClampU32 => ternary_u32(ctx, o, C::glsl_u_clamp),
        Inst::Comment => Ok(OpResult::INVALID),
        Inst::CompareEqual => compare(ctx, o, C::f_ord_equal, C::i_equal),
        Inst::CompareGreater => compare(ctx, o, C::f_ord_greater_than, C::s_greater_than),
        Inst::CompareGreaterOrEqual => {
            compare(ctx, o, C::f_ord_greater_than_equal, C::s_greater_than_equal)
        }
        Inst::CompareGreaterOrEqualU32 => compare_u32(ctx, o, C::u_greater_than_equal),
        Inst::CompareGreaterU32 => compare_u32(ctx, o, C::u_greater_than),
        Inst::CompareLess => compare(ctx, o, C::f_ord_less_than, C::s_less_than),
        Inst::CompareLessOrEqual => compare(ctx, o, C::f_ord_less_than_equal, C::s_less_than_equal),
        Inst::CompareLessOrEqualU32 => compare_u32(ctx, o, C::u_less_than_equal),
        Inst::CompareLessU32 => compare_u32(ctx, o, C::u_less_than),
        Inst::CompareNotEqual => compare(ctx, o, C::f_ord_not_equal, C::i_not_equal),
        Inst::ConditionalSelect => {
            let (s1, s2, s3) = (src(o, 0)?, src(o, 1)?, src(o, 2)?);
            let cond = ctx.get(AggType::BOOL, s1)?;
            if o.inst.0 & Inst::FP64 != 0 {
                let t = ctx.type_fp64();
                let a = ctx.get_fp64(s2)?;
                let b = ctx.get_fp64(s3)?;
                Ok(OpResult::new(AggType::FP64, ctx.select(t, cond, a, b)))
            } else if o.inst.0 & Inst::FP32 != 0 {
                let t = ctx.type_fp32();
                let a = ctx.get_fp32(s2)?;
                let b = ctx.get_fp32(s3)?;
                Ok(OpResult::new(AggType::FP32, ctx.select(t, cond, a, b)))
            } else {
                let t = ctx.type_s32();
                let a = ctx.get_s32(s2)?;
                let b = ctx.get_s32(s3)?;
                Ok(OpResult::new(AggType::S32, ctx.select(t, cond, a, b)))
            }
        }
        Inst::ConvertFP32ToFP64 => convert(ctx, o, AggType::FP32, AggType::FP64, C::f_convert),
        Inst::ConvertFP32ToS32 => convert(ctx, o, AggType::FP32, AggType::S32, C::convert_f_to_s),
        Inst::ConvertFP32ToU32 => convert(ctx, o, AggType::FP32, AggType::U32, C::convert_f_to_u),
        Inst::ConvertFP64ToFP32 => convert(ctx, o, AggType::FP64, AggType::FP32, C::f_convert),
        Inst::ConvertFP64ToS32 => convert(ctx, o, AggType::FP64, AggType::S32, C::convert_f_to_s),
        Inst::ConvertFP64ToU32 => convert(ctx, o, AggType::FP64, AggType::U32, C::convert_f_to_u),
        Inst::ConvertS32ToFP32 => convert(ctx, o, AggType::S32, AggType::FP32, C::convert_s_to_f),
        Inst::ConvertS32ToFP64 => convert(ctx, o, AggType::S32, AggType::FP64, C::convert_s_to_f),
        Inst::ConvertU32ToFP32 => convert(ctx, o, AggType::U32, AggType::FP32, C::convert_u_to_f),
        Inst::ConvertU32ToFP64 => convert(ctx, o, AggType::U32, AggType::FP64, C::convert_u_to_f),
        Inst::Cosine => unary(ctx, o, C::glsl_cos, None),
        Inst::Ddx => unary_fp32(ctx, o, C::d_pdx),
        Inst::Ddy => unary_fp32(ctx, o, C::d_pdy),
        Inst::Discard => {
            ctx.kill();
            Ok(OpResult::INVALID)
        }
        Inst::Divide => binary(ctx, o, C::f_div, Some(C::s_div)),
        Inst::EmitVertex => {
            ctx.emit_vertex();
            Ok(OpResult::INVALID)
        }
        Inst::EndPrimitive => {
            ctx.end_primitive();
            Ok(OpResult::INVALID)
        }
        Inst::ExponentB2 => unary(ctx, o, C::glsl_exp2, None),
        Inst::FSIBegin => {
            if ctx.host.supports_fragment_shader_interlock {
                ctx.begin_invocation_interlock_ext();
            }
            Ok(OpResult::INVALID)
        }
        Inst::FSIEnd => {
            if ctx.host.supports_fragment_shader_interlock {
                ctx.end_invocation_interlock_ext();
            }
            Ok(OpResult::INVALID)
        }
        Inst::FindLSB => {
            let s = src(o, 0)?;
            let source = ctx.get_u32(s)?;
            let t = ctx.type_u32();
            Ok(OpResult::new(AggType::U32, ctx.glsl_find_i_lsb(t, source)))
        }
        Inst::FindMSBS32 => {
            let s = src(o, 0)?;
            let source = ctx.get_s32(s)?;
            let t = ctx.type_u32();
            Ok(OpResult::new(AggType::U32, ctx.glsl_find_s_msb(t, source)))
        }
        Inst::FindMSBU32 => {
            let s = src(o, 0)?;
            let source = ctx.get_u32(s)?;
            let t = ctx.type_u32();
            Ok(OpResult::new(AggType::U32, ctx.glsl_find_u_msb(t, source)))
        }
        Inst::Floor => unary(ctx, o, C::glsl_floor, None),
        Inst::FusedMultiplyAdd => ternary(ctx, o, C::glsl_fma, None),
        Inst::GroupMemoryBarrier => {
            let tu = ctx.type_u32();
            let s = ctx.constant(tu, Lit::I32(scope::Workgroup as i32));
            let tu = ctx.type_u32();
            let m = ctx.constant(tu, Lit::I32(DEFAULT_MEMORY_SEMANTICS as i32));
            ctx.memory_barrier(s, m);
            Ok(OpResult::INVALID)
        }
        Inst::ImageAtomic => super::instr_image_atomic::image_atomic(ctx, o),
        Inst::ImageLoad => super::instr_texture::image_load(ctx, o),
        Inst::ImageStore => super::instr_texture::image_store(ctx, o),
        Inst::IsNan => {
            let s = src(o, 0)?;
            let r = if o.inst.0 & Inst::FP64 != 0 {
                let t = ctx.type_bool(false);
                let v = ctx.get_fp64(s)?;
                ctx.is_nan(t, v)
            } else {
                let t = ctx.type_bool(false);
                let v = ctx.get_fp32(s)?;
                ctx.is_nan(t, v)
            };
            Ok(OpResult::new(AggType::BOOL, r))
        }
        Inst::Load => load_or_store(ctx, o, false),
        Inst::Lod => super::instr_texture::lod(ctx, o),
        Inst::LogarithmB2 => unary(ctx, o, C::glsl_log2, None),
        Inst::LogicalAnd => binary_bool(ctx, o, C::logical_and),
        Inst::LogicalExclusiveOr => binary_bool(ctx, o, C::logical_not_equal),
        Inst::LogicalNot => {
            let s = src(o, 0)?;
            let t = ctx.type_bool(false);
            let v = ctx.get(AggType::BOOL, s)?;
            Ok(OpResult::new(AggType::BOOL, ctx.logical_not(t, v)))
        }
        Inst::LogicalOr => binary_bool(ctx, o, C::logical_or),
        Inst::LoopBreak => {
            let mut lb = ctx.current_block.ok_or("no current block")?;
            while ctx.ast.block_type(lb) != Some(BlockType::DoWhile) {
                lb = ctx.ast.parent(lb).ok_or("break outside a loop")?;
            }
            let p = ctx.ast.parent(lb).ok_or("loop without parent")?;
            let l = ctx.next_label(p);
            ctx.branch(l);
            Ok(OpResult::INVALID)
        }
        Inst::LoopContinue => {
            let mut lb = ctx.current_block.ok_or("no current block")?;
            while ctx.ast.block_type(lb) != Some(BlockType::DoWhile) {
                lb = ctx.ast.parent(lb).ok_or("continue outside a loop")?;
            }
            let (_, ct) = *ctx.loop_targets.get(&lb).ok_or("loop without targets")?;
            ctx.branch(ct);
            Ok(OpResult::INVALID)
        }
        Inst::Maximum => binary(ctx, o, C::glsl_f_max, Some(C::glsl_s_max)),
        Inst::MaximumU32 => binary_u32(ctx, o, C::glsl_u_max),
        Inst::MemoryBarrier => {
            let tu = ctx.type_u32();
            let s = ctx.constant(tu, Lit::I32(scope::Device as i32));
            let tu = ctx.type_u32();
            let m = ctx.constant(tu, Lit::I32(DEFAULT_MEMORY_SEMANTICS as i32));
            ctx.memory_barrier(s, m);
            Ok(OpResult::INVALID)
        }
        Inst::Minimum => binary(ctx, o, C::glsl_f_min, Some(C::glsl_s_min)),
        Inst::MinimumU32 => binary_u32(ctx, o, C::glsl_u_min),
        Inst::Modulo => binary(ctx, o, C::f_mod, None),
        Inst::Multiply => binary(ctx, o, C::f_mul, Some(C::i_mul)),
        Inst::MultiplyHighS32 => {
            let (s1, s2) = (src(o, 0)?, src(o, 1)?);
            let a = ctx.type_s32();
            let b = ctx.type_s32();
            let rt = ctx.type_struct(false, &[a, b]);
            let x = ctx.get_s32(s1)?;
            let y = ctx.get_s32(s2)?;
            let r = ctx.s_mul_extended(rt, x, y);
            let t = ctx.type_s32();
            let r = ctx.composite_extract(t, r, &[Lit::I32(1)]);
            Ok(OpResult::new(AggType::S32, r))
        }
        Inst::MultiplyHighU32 => {
            let (s1, s2) = (src(o, 0)?, src(o, 1)?);
            let a = ctx.type_u32();
            let b = ctx.type_u32();
            let rt = ctx.type_struct(false, &[a, b]);
            let x = ctx.get_u32(s1)?;
            let y = ctx.get_u32(s2)?;
            let r = ctx.u_mul_extended(rt, x, y);
            let t = ctx.type_u32();
            let r = ctx.composite_extract(t, r, &[Lit::I32(1)]);
            Ok(OpResult::new(AggType::U32, r))
        }
        Inst::Negate => unary(ctx, o, C::f_negate, Some(C::s_negate)),
        Inst::PackDouble2x32 => {
            let v0 = ctx.get_u32(src(o, 0)?)?;
            let v1 = ctx.get_u32(src(o, 1)?)?;
            let u = ctx.type_u32();
            let vt = ctx.type_vector(u, Lit::I32(2), false);
            let vector = ctx.composite_construct(vt, &[v0, v1]);
            let t = ctx.type_fp64();
            Ok(OpResult::new(
                AggType::FP64,
                ctx.glsl_pack_double2x32(t, vector),
            ))
        }
        Inst::PackHalf2x16 => {
            let v0 = ctx.get_fp32(src(o, 0)?)?;
            let v1 = ctx.get_fp32(src(o, 1)?)?;
            let f = ctx.type_fp32();
            let vt = ctx.type_vector(f, Lit::I32(2), false);
            let vector = ctx.composite_construct(vt, &[v0, v1]);
            let t = ctx.type_u32();
            Ok(OpResult::new(
                AggType::U32,
                ctx.glsl_pack_half2x16(t, vector),
            ))
        }
        Inst::ReciprocalSquareRoot => unary(ctx, o, C::glsl_inverse_sqrt, None),
        Inst::Return => {
            ctx.may_have_returned = true;
            if !o.sources.is_empty() {
                let rt = ctx.functions[ctx.current_function].return_type;
                let v = ctx.get(rt, src(o, 0)?)?;
                ctx.return_value(v);
            } else {
                ctx.return_();
            }
            Ok(OpResult::INVALID)
        }
        Inst::Round => unary(ctx, o, C::glsl_round_even, None),
        Inst::ShiftLeft => binary_s32(ctx, o, C::shift_left_logical),
        Inst::ShiftRightS32 => binary_s32(ctx, o, C::shift_right_arithmetic),
        Inst::ShiftRightU32 => binary_s32(ctx, o, C::shift_right_logical),
        Inst::Shuffle => shuffle(ctx, o, C::group_non_uniform_shuffle),
        Inst::ShuffleDown => shuffle(ctx, o, C::group_non_uniform_shuffle_down),
        Inst::ShuffleUp => shuffle(ctx, o, C::group_non_uniform_shuffle_up),
        Inst::ShuffleXor => shuffle(ctx, o, C::group_non_uniform_shuffle_xor),
        Inst::Sine => unary(ctx, o, C::glsl_sin, None),
        Inst::SquareRoot => unary(ctx, o, C::glsl_sqrt, None),
        Inst::Store => load_or_store(ctx, o, true),
        Inst::Subtract => binary(ctx, o, C::f_sub, Some(C::i_sub)),
        Inst::SwizzleAdd => swizzle_add(ctx, o),
        Inst::TextureSample => super::instr_texture::texture_sample(ctx, o),
        Inst::TextureQuerySamples => super::instr_texture::query_samples(ctx, o),
        Inst::TextureQuerySize => super::instr_texture::query_size(ctx, o),
        Inst::Truncate => unary(ctx, o, C::glsl_trunc, None),
        Inst::UnpackDouble2x32 => {
            let v = ctx.get_fp64(src(o, 0)?)?;
            let u = ctx.type_u32();
            let vt = ctx.type_vector(u, Lit::I32(2), false);
            let vector = ctx.glsl_unpack_double2x32(vt, v);
            let t = ctx.type_u32();
            Ok(OpResult::new(
                AggType::U32,
                ctx.composite_extract(t, vector, &[Lit::I32(o.index)]),
            ))
        }
        Inst::UnpackHalf2x16 => {
            let v = ctx.get_u32(src(o, 0)?)?;
            let f = ctx.type_fp32();
            let vt = ctx.type_vector(f, Lit::I32(2), false);
            let vector = ctx.glsl_unpack_half2x16(vt, v);
            let t = ctx.type_fp32();
            Ok(OpResult::new(
                AggType::FP32,
                ctx.composite_extract(t, vector, &[Lit::I32(o.index)]),
            ))
        }
        Inst::VectorExtract => {
            let (vector, vector_type) = ctx.get_with_type(src(o, 0)?)?;
            let scalar_type = AggType(vector_type.0 & !AggType::ELEMENT_COUNT_MASK);
            let rt = ctx.ty(scalar_type)?;
            let s1 = src(o, 1)?;
            let result = if let Kind::Operand {
                ty: OperandType::Constant,
                value,
                ..
            } = ctx.ast.kind(s1).clone()
            {
                ctx.composite_extract(rt, vector, &[Lit::I32(value)])
            } else {
                let index = ctx.get(AggType::S32, s1)?;
                ctx.vector_extract_dynamic(rt, vector, index)
            };
            Ok(OpResult::new(scalar_type, result))
        }
        Inst::VoteAll | Inst::VoteAllEqual | Inst::VoteAny => {
            let tu = ctx.type_u32();
            let execution = ctx.constant(tu, Lit::I32(scope::Subgroup as i32));
            let t = ctx.type_bool(false);
            let p = ctx.get(AggType::BOOL, src(o, 0)?)?;
            let r = match inst {
                Inst::VoteAll => ctx.group_non_uniform_all(t, execution, p),
                Inst::VoteAllEqual => ctx.group_non_uniform_all_equal(t, execution, p),
                _ => ctx.group_non_uniform_any(t, execution, p),
            };
            Ok(OpResult::new(AggType::BOOL, r))
        }
        _ => Err(format!("unported: instruction {:x}", o.inst.0)),
    }
}

fn control_barrier(ctx: &mut CodeGenContext, sc: u32, semantics: u32) {
    let tu = ctx.type_u32();
    let a = ctx.constant(tu, Lit::I32(sc as i32));
    let tu = ctx.type_u32();
    let b = ctx.constant(tu, Lit::I32(sc as i32));
    let tu = ctx.type_u32();
    let c = ctx.constant(tu, Lit::I32(semantics as i32));
    ctx.control_barrier(a, b, c);
}

fn generate_call(ctx: &mut CodeGenContext, o: &OpData) -> R<OpResult> {
    let Kind::Operand { value, .. } = ctx.ast.kind(src(o, 0)?).clone() else {
        return Err("call without function id".into());
    };
    let function = ctx
        .functions
        .get(value as usize)
        .ok_or("call to an unknown function")?
        .clone();
    let spv_func = ctx.spv_functions[value as usize].1;
    let mut args = Vec::new();
    for i in 0..o.sources.len() - 1 {
        args.push(ctx.local_pointer(src(o, i + 1)?)?);
    }
    let ret_type = function.return_type;
    let rt = ctx.ty(ret_type)?;
    let result = ctx.function_call(rt, spv_func, &args);
    Ok(OpResult::new(ret_type, result))
}

fn convert(
    ctx: &mut CodeGenContext,
    o: &OpData,
    from: AggType,
    to: AggType,
    f: fn(&mut CodeGenContext, SInst, SInst) -> SInst,
) -> R<OpResult> {
    let s = src(o, 0)?;
    let t = ctx.ty(to)?;
    let v = ctx.get(from, s)?;
    Ok(OpResult::new(to, f(ctx, t, v)))
}

fn shuffle(
    ctx: &mut CodeGenContext,
    o: &OpData,
    f: fn(&mut CodeGenContext, SInst, SInst, SInst, SInst) -> SInst,
) -> R<OpResult> {
    let value = ctx.get_fp32(src(o, 0)?)?;
    let index = ctx.get_u32(src(o, 1)?)?;
    let t = ctx.type_fp32();
    let tu = ctx.type_u32();
    let sc = ctx.constant(tu, Lit::I32(scope::Subgroup as i32));
    Ok(OpResult::new(AggType::FP32, f(ctx, t, sc, value, index)))
}

fn swizzle_add(ctx: &mut CodeGenContext, o: &OpData) -> R<OpResult> {
    let x = ctx.get(AggType::FP32, src(o, 0)?)?;
    let y = ctx.get(AggType::FP32, src(o, 1)?)?;
    let mask = ctx.get(AggType::U32, src(o, 2)?)?;
    let f = ctx.type_fp32();
    let v4 = ctx.type_vector(f, Lit::I32(4), false);
    let tf = ctx.type_fp32();
    let one = ctx.constant(tf, Lit::f32(1.0));
    let tf = ctx.type_fp32();
    let minus_one = ctx.constant(tf, Lit::f32(-1.0));
    let tf = ctx.type_fp32();
    let zero = ctx.constant(tf, Lit::f32(0.0));
    let x_lut = ctx.constant_composite(v4, &[one, minus_one, one, zero]);
    let y_lut = ctx.constant_composite(v4, &[one, one, minus_one, one]);
    let tu = ctx.type_u32();
    let three = ctx.constant(tu, Lit::I32(3));
    let thread_id = get_scalar_input(ctx, crate::ir::IoVariable::SubgroupLaneId)?;
    let tu = ctx.type_u32();
    let mut shift = ctx.bitwise_and(tu, thread_id, three);
    let tu = ctx.type_u32();
    let one_u = ctx.const_u32_i(1);
    shift = ctx.shift_left_logical(tu, shift, one_u);
    let tu = ctx.type_u32();
    let mut lut_idx = ctx.shift_right_logical(tu, mask, shift);
    let tu = ctx.type_u32();
    lut_idx = ctx.bitwise_and(tu, lut_idx, three);
    let tf = ctx.type_fp32();
    let xv = ctx.vector_extract_dynamic(tf, x_lut, lut_idx);
    let tf = ctx.type_fp32();
    let yv = ctx.vector_extract_dynamic(tf, y_lut, lut_idx);
    let tf = ctx.type_fp32();
    let xr = ctx.f_mul(tf, x, xv);
    let tf = ctx.type_fp32();
    let yr = ctx.f_mul(tf, y, yv);
    let tf = ctx.type_fp32();
    let r = ctx.f_add(tf, xr, yr);
    Ok(OpResult::new(AggType::FP32, r))
}
