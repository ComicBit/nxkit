//! Helpers of `Instructions`: the generic unary, binary and ternary forms, compares, atomics, loads and stores and the
//! storage pointer computation.

use super::context::*;
use super::spirv_enums::{decoration, memory_access_mask, storage_class};
use crate::ir::{Inst, IoVariable, OperandType, StorageKind};
use crate::spv::{Inst as SInst, Lit, Operand};
use crate::structured::ast::*;
use crate::structured::{IoDef, types::AggType};

type R<T> = Result<T, String>;
pub type F1 = fn(&mut CodeGenContext, SInst, SInst) -> SInst;
pub type F2 = fn(&mut CodeGenContext, SInst, SInst, SInst) -> SInst;
pub type F3 = fn(&mut CodeGenContext, SInst, SInst, SInst, SInst) -> SInst;

fn src(o: &OpData, i: usize) -> R<A> {
    o.sources
        .get(i)
        .copied()
        .ok_or_else(|| "missing source".to_string())
}

fn is64(o: &OpData) -> bool {
    o.inst.0 & Inst::FP64 != 0
}

fn is32(o: &OpData) -> bool {
    o.inst.0 & Inst::FP32 != 0
}

fn need<T>(f: Option<T>) -> R<T> {
    f.ok_or_else(|| "no integer form of this instruction".to_string())
}

fn no_contraction(ctx: &mut CodeGenContext, o: &OpData, result: SInst) {
    if !ctx.host.reduced_precision || o.force_precise {
        ctx.decorate(result, decoration::NoContraction, &[]);
    }
}

pub fn unary(ctx: &mut CodeGenContext, o: &OpData, f: F1, i: Option<F1>) -> R<OpResult> {
    let s = src(o, 0)?;
    if is64(o) {
        let t = ctx.type_fp64();
        let v = ctx.get_fp64(s)?;
        Ok(OpResult::new(AggType::FP64, f(ctx, t, v)))
    } else if is32(o) {
        let t = ctx.type_fp32();
        let v = ctx.get_fp32(s)?;
        Ok(OpResult::new(AggType::FP32, f(ctx, t, v)))
    } else {
        let i = need(i)?;
        let t = ctx.type_s32();
        let v = ctx.get_s32(s)?;
        Ok(OpResult::new(AggType::S32, i(ctx, t, v)))
    }
}

pub fn unary_fp32(ctx: &mut CodeGenContext, o: &OpData, f: F1) -> R<OpResult> {
    let s = src(o, 0)?;
    let t = ctx.type_fp32();
    let v = ctx.get_fp32(s)?;
    Ok(OpResult::new(AggType::FP32, f(ctx, t, v)))
}

pub fn unary_s32(ctx: &mut CodeGenContext, o: &OpData, f: F1) -> R<OpResult> {
    let s = src(o, 0)?;
    let t = ctx.type_s32();
    let v = ctx.get_s32(s)?;
    Ok(OpResult::new(AggType::S32, f(ctx, t, v)))
}

pub fn binary(ctx: &mut CodeGenContext, o: &OpData, f: F2, i: Option<F2>) -> R<OpResult> {
    let (s1, s2) = (src(o, 0)?, src(o, 1)?);
    if is64(o) {
        let t = ctx.type_fp64();
        let a = ctx.get_fp64(s1)?;
        let b = ctx.get_fp64(s2)?;
        let r = f(ctx, t, a, b);
        no_contraction(ctx, o, r);
        Ok(OpResult::new(AggType::FP64, r))
    } else if is32(o) {
        let t = ctx.type_fp32();
        let a = ctx.get_fp32(s1)?;
        let b = ctx.get_fp32(s2)?;
        let r = f(ctx, t, a, b);
        no_contraction(ctx, o, r);
        Ok(OpResult::new(AggType::FP32, r))
    } else {
        let i = need(i)?;
        let t = ctx.type_s32();
        let a = ctx.get_s32(s1)?;
        let b = ctx.get_s32(s2)?;
        Ok(OpResult::new(AggType::S32, i(ctx, t, a, b)))
    }
}

pub fn binary_bool(ctx: &mut CodeGenContext, o: &OpData, f: F2) -> R<OpResult> {
    let (s1, s2) = (src(o, 0)?, src(o, 1)?);
    let t = ctx.type_bool(false);
    let a = ctx.get(AggType::BOOL, s1)?;
    let b = ctx.get(AggType::BOOL, s2)?;
    Ok(OpResult::new(AggType::BOOL, f(ctx, t, a, b)))
}

pub fn binary_s32(ctx: &mut CodeGenContext, o: &OpData, f: F2) -> R<OpResult> {
    let (s1, s2) = (src(o, 0)?, src(o, 1)?);
    let t = ctx.type_s32();
    let a = ctx.get_s32(s1)?;
    let b = ctx.get_s32(s2)?;
    Ok(OpResult::new(AggType::S32, f(ctx, t, a, b)))
}

pub fn binary_u32(ctx: &mut CodeGenContext, o: &OpData, f: F2) -> R<OpResult> {
    let (s1, s2) = (src(o, 0)?, src(o, 1)?);
    let t = ctx.type_u32();
    let a = ctx.get_u32(s1)?;
    let b = ctx.get_u32(s2)?;
    Ok(OpResult::new(AggType::U32, f(ctx, t, a, b)))
}

pub fn ternary(ctx: &mut CodeGenContext, o: &OpData, f: F3, i: Option<F3>) -> R<OpResult> {
    let (s1, s2, s3) = (src(o, 0)?, src(o, 1)?, src(o, 2)?);
    if is64(o) {
        let t = ctx.type_fp64();
        let a = ctx.get_fp64(s1)?;
        let b = ctx.get_fp64(s2)?;
        let c = ctx.get_fp64(s3)?;
        let r = f(ctx, t, a, b, c);
        no_contraction(ctx, o, r);
        Ok(OpResult::new(AggType::FP64, r))
    } else if is32(o) {
        let t = ctx.type_fp32();
        let a = ctx.get_fp32(s1)?;
        let b = ctx.get_fp32(s2)?;
        let c = ctx.get_fp32(s3)?;
        let r = f(ctx, t, a, b, c);
        no_contraction(ctx, o, r);
        Ok(OpResult::new(AggType::FP32, r))
    } else {
        let i = need(i)?;
        let t = ctx.type_s32();
        let a = ctx.get_s32(s1)?;
        let b = ctx.get_s32(s2)?;
        let c = ctx.get_s32(s3)?;
        Ok(OpResult::new(AggType::S32, i(ctx, t, a, b, c)))
    }
}

pub fn ternary_u32(ctx: &mut CodeGenContext, o: &OpData, f: F3) -> R<OpResult> {
    let (s1, s2, s3) = (src(o, 0)?, src(o, 1)?, src(o, 2)?);
    let t = ctx.type_u32();
    let a = ctx.get_u32(s1)?;
    let b = ctx.get_u32(s2)?;
    let c = ctx.get_u32(s3)?;
    Ok(OpResult::new(AggType::U32, f(ctx, t, a, b, c)))
}

pub fn compare(ctx: &mut CodeGenContext, o: &OpData, f: F2, i: F2) -> R<OpResult> {
    let (s1, s2) = (src(o, 0)?, src(o, 1)?);
    let r = if is64(o) {
        let t = ctx.type_bool(false);
        let a = ctx.get_fp64(s1)?;
        let b = ctx.get_fp64(s2)?;
        f(ctx, t, a, b)
    } else if is32(o) {
        let t = ctx.type_bool(false);
        let a = ctx.get_fp32(s1)?;
        let b = ctx.get_fp32(s2)?;
        f(ctx, t, a, b)
    } else {
        let t = ctx.type_bool(false);
        let a = ctx.get_s32(s1)?;
        let b = ctx.get_s32(s2)?;
        i(ctx, t, a, b)
    };
    Ok(OpResult::new(AggType::BOOL, r))
}

pub fn compare_u32(ctx: &mut CodeGenContext, o: &OpData, f: F2) -> R<OpResult> {
    let (s1, s2) = (src(o, 0)?, src(o, 1)?);
    let t = ctx.type_bool(false);
    let a = ctx.get_u32(s1)?;
    let b = ctx.get_u32(s2)?;
    Ok(OpResult::new(AggType::BOOL, f(ctx, t, a, b)))
}

pub fn atomic_memory(ctx: &mut CodeGenContext, o: &OpData, inst: Inst) -> R<OpResult> {
    let (elem_pointer, var_type) = get_storage_pointer(ctx, o)?;
    if inst == Inst::AtomicCompareAndSwap {
        let v0 = ctx.get(var_type, src(o, o.sources.len() - 2)?)?;
        let v1 = ctx.get(var_type, src(o, o.sources.len() - 1)?)?;
        let one = ctx.const_u32_i(1);
        let zero = ctx.const_u32_i(0);
        let t = ctx.ty(var_type)?;
        let r = ctx.atomic_compare_exchange(t, elem_pointer, one, zero, zero, v1, v0);
        return Ok(OpResult::new(var_type, r));
    }
    let value = ctx.get(var_type, src(o, o.sources.len() - 1)?)?;
    let one = ctx.const_u32_i(1);
    let zero = ctx.const_u32_i(0);
    let t = ctx.ty(var_type)?;
    let r = match inst {
        Inst::AtomicAdd => ctx.atomic_i_add(t, elem_pointer, one, zero, value),
        Inst::AtomicAnd => ctx.atomic_and(t, elem_pointer, one, zero, value),
        Inst::AtomicMinS32 => ctx.atomic_s_min(t, elem_pointer, one, zero, value),
        Inst::AtomicMinU32 => ctx.atomic_u_min(t, elem_pointer, one, zero, value),
        Inst::AtomicMaxS32 => ctx.atomic_s_max(t, elem_pointer, one, zero, value),
        Inst::AtomicMaxU32 => ctx.atomic_u_max(t, elem_pointer, one, zero, value),
        Inst::AtomicOr => ctx.atomic_or(t, elem_pointer, one, zero, value),
        Inst::AtomicSwap => ctx.atomic_exchange(t, elem_pointer, one, zero, value),
        _ => ctx.atomic_xor(t, elem_pointer, one, zero, value),
    };
    Ok(OpResult::new(var_type, r))
}

pub fn load_or_store(ctx: &mut CodeGenContext, o: &OpData, is_store: bool) -> R<OpResult> {
    let (pointer, var_type) = get_storage_pointer(ctx, o)?;
    let physical = o.storage_kind == StorageKind::GlobalMemory;
    let access = if physical {
        Some(memory_access_mask::Aligned)
    } else {
        None
    };
    if is_store {
        let v = ctx.get(var_type, src(o, o.sources.len() - 1)?)?;
        let store = ctx.store(pointer, v, access);
        if physical {
            ctx.m.add(store, Operand::Lit(Lit::I32(4)));
        }
        Ok(OpResult::INVALID)
    } else {
        let t = ctx.ty(var_type)?;
        let result = ctx.load(t, pointer, access);
        if physical {
            ctx.m.add(result, Operand::Lit(Lit::I32(4)));
        }
        Ok(OpResult::new(var_type, result))
    }
}

fn const_operand(ctx: &CodeGenContext, a: A) -> Option<i32> {
    match ctx.ast.kind(a) {
        Kind::Operand {
            ty: OperandType::Constant,
            value,
            ..
        } => Some(*value),
        _ => None,
    }
}

fn is_atomic(inst: Inst) -> bool {
    matches!(
        Inst(inst.0 & Inst::MASK),
        Inst::AtomicAdd
            | Inst::AtomicAnd
            | Inst::AtomicCompareAndSwap
            | Inst::AtomicMaxS32
            | Inst::AtomicMaxU32
            | Inst::AtomicMinS32
            | Inst::AtomicMinU32
            | Inst::AtomicOr
            | Inst::AtomicSwap
            | Inst::AtomicXor
    )
}

/// `GetStoragePointer`: the pointer and the element type.
pub fn get_storage_pointer(ctx: &mut CodeGenContext, o: &OpData) -> R<(SInst, AggType)> {
    let storage_kind = o.storage_kind;
    let (storage_class_v, base_obj, mut var_type);
    let mut src_index = 0usize;
    let mut per_vertex_built_in: Option<IoVariable> = None;
    match storage_kind {
        StorageKind::GlobalMemory => {
            let tu = ctx.type_u32();
            let vt = ctx.type_vector(tu, Lit::I32(2), false);
            let a = ctx.get(AggType::U32, src(o, 0)?)?;
            let b = ctx.get(AggType::U32, src(o, 1)?)?;
            let words = ctx.composite_construct(vt, &[a, b]);
            let tu = ctx.type_u32();
            let pt = ctx.type_pointer(storage_class::PhysicalStorageBuffer, tu, false);
            let t64 = ctx.type_int(Lit::I32(64), Lit::I32(0), false);
            let bc = ctx.bitcast(t64, words);
            return Ok((ctx.convert_u_to_ptr(pt, bc), AggType::U32));
        }
        StorageKind::ConstantBuffer | StorageKind::StorageBuffer => {
            let binding = const_operand(ctx, src(o, src_index)?)
                .ok_or("first input of a buffer access must be a constant operand")?;
            src_index += 1;
            let field = const_operand(ctx, src(o, src_index)?)
                .ok_or("second input of a buffer access must be a constant operand")?;
            let is_cb = storage_kind == StorageKind::ConstantBuffer;
            let list = if is_cb {
                &ctx.properties.constant_buffers
            } else {
                &ctx.properties.storage_buffers
            };
            let buffer = list
                .iter()
                .find(|b| b.binding == binding)
                .ok_or("buffer not in the properties")?;
            let f = buffer
                .fields
                .get(field as usize)
                .ok_or("field out of range")?;
            storage_class_v = storage_class::Uniform;
            var_type = AggType(f.ty.0 & AggType::ELEMENT_TYPE_MASK);
            base_obj = if is_cb {
                ctx.constant_buffers.get(&binding).copied()
            } else {
                ctx.storage_buffers.get(&binding).copied()
            }
            .ok_or("buffer not declared")?;
        }
        StorageKind::LocalMemory | StorageKind::SharedMemory => {
            let id = const_operand(ctx, src(o, src_index)?)
                .ok_or("first input of a memory access must be a constant operand")?;
            src_index += 1;
            if storage_kind == StorageKind::LocalMemory {
                storage_class_v = storage_class::Private;
                let m = ctx
                    .properties
                    .local_memories
                    .iter()
                    .find(|m| m.id == id)
                    .ok_or("local memory not declared")?;
                var_type = AggType(m.ty.0 & AggType::ELEMENT_TYPE_MASK);
                base_obj = ctx.local_memories.get(&id).copied().ok_or("local memory")?;
            } else {
                storage_class_v = storage_class::Workgroup;
                let m = ctx
                    .properties
                    .shared_memories
                    .iter()
                    .find(|m| m.id == id)
                    .ok_or("shared memory not declared")?;
                var_type = AggType(m.ty.0 & AggType::ELEMENT_TYPE_MASK);
                base_obj = ctx
                    .shared_memories
                    .get(&id)
                    .copied()
                    .ok_or("shared memory")?;
            }
        }
        StorageKind::Input
        | StorageKind::InputPerPatch
        | StorageKind::Output
        | StorageKind::OutputPerPatch => {
            let var_id = const_operand(ctx, src(o, src_index)?)
                .ok_or("first input of an io access must be a constant operand")?;
            src_index += 1;
            let mut io_variable = IoVariable(var_id as u32);
            let is_out = is_output(storage_kind);
            let per_patch = is_per_patch(storage_kind);
            let mut location = 0;
            let mut component = 0;
            if ctx
                .definitions
                .has_per_location_input_or_output(io_variable, is_out)
            {
                location = const_operand(ctx, src(o, src_index)?)
                    .ok_or("second input of an io access must be a constant operand")?;
                src_index += 1;
                if o.sources.len() > src_index
                    && let Some(elem) = const_operand(ctx, o.sources[src_index])
                    && ctx.definitions.has_per_location_input_or_output_component(
                        io_variable,
                        location,
                        elem,
                        is_out,
                    )
                {
                    component = elem;
                    src_index += 1;
                }
            }
            if io_variable == IoVariable::UserDefined {
                var_type = ctx.definitions.user_defined_type(location, is_out)?;
            } else if io_variable == IoVariable::FragmentOutputColor {
                var_type = ctx.definitions.fragment_output_color_type(location)?;
            } else {
                var_type = spirv_built_in(io_variable).1;
                if matches!(
                    io_variable,
                    IoVariable::Position | IoVariable::PointSize | IoVariable::ClipDistance
                ) {
                    per_vertex_built_in = Some(io_variable);
                    io_variable = IoVariable::Position;
                }
            }
            var_type = AggType(var_type.0 & AggType::ELEMENT_TYPE_MASK);
            storage_class_v = if is_out {
                storage_class::Output
            } else {
                storage_class::Input
            };
            let def = IoDef {
                storage_kind,
                io_variable,
                location,
                component,
            };
            let dict = if per_patch {
                if is_out {
                    &ctx.outputs_per_patch
                } else {
                    &ctx.inputs_per_patch
                }
            } else if is_out {
                &ctx.outputs
            } else {
                &ctx.inputs
            };
            base_obj = dict.get(def).ok_or("io variable not declared")?;
        }
        _ => return Err(format!("Invalid storage kind {}.", storage_kind.0)),
    }
    let is_store_or_atomic = o.inst == Inst::Store || is_atomic(o.inst);
    let mut inputs_count = (if is_store_or_atomic {
        o.sources.len() - 1
    } else {
        o.sources.len()
    }) as i64
        - src_index as i64;
    if let Some(v) = per_vertex_built_in {
        let field_index = match v {
            IoVariable::Position => 0,
            IoVariable::PointSize => 1,
            _ => 2,
        };
        let stage = ctx.definitions.stage()?;
        if stage == crate::attr::Stage::TessellationControl
            || (storage_kind != StorageKind::Output
                && matches!(
                    stage,
                    crate::attr::Stage::TessellationEvaluation | crate::attr::Stage::Geometry
                ))
        {
            return Err("unported: per-vertex array built-in access".into());
        }
        let mut indexes: Vec<SInst> = Vec::new();
        let ts = ctx.type_s32();
        indexes.push(ctx.constant(ts, Lit::I32(field_index)));
        let mut index = 1;
        while index < inputs_count + 1 {
            indexes.push(ctx.get(AggType::S32, src(o, src_index)?)?);
            src_index += 1;
            index += 1;
        }
        let t = ctx.ty(var_type)?;
        let pt = ctx.type_pointer(storage_class_v, t, false);
        return Ok((ctx.access_chain(pt, base_obj, &indexes), var_type));
    }
    if o.inst == Inst::AtomicCompareAndSwap {
        inputs_count -= 1;
    }
    if inputs_count == 0 {
        return Ok((base_obj, var_type));
    }
    let mut indexes = Vec::new();
    for _ in 0..inputs_count {
        indexes.push(ctx.get(AggType::S32, src(o, src_index)?)?);
        src_index += 1;
    }
    let t = ctx.ty(var_type)?;
    let pt = ctx.type_pointer(storage_class_v, t, false);
    Ok((ctx.access_chain(pt, base_obj, &indexes), var_type))
}

/// `GetScalarInput`.
pub fn get_scalar_input(ctx: &mut CodeGenContext, v: IoVariable) -> R<SInst> {
    let (_, vt) = spirv_built_in(v);
    let vt = AggType(vt.0 & AggType::ELEMENT_TYPE_MASK);
    let def = IoDef {
        storage_kind: StorageKind::Input,
        io_variable: v,
        location: 0,
        component: 0,
    };
    let t = ctx.ty(vt)?;
    let p = ctx.inputs.get(def).ok_or("input not declared")?;
    Ok(ctx.load(t, p, None))
}

/// `GetSwizzledResult`.
pub fn get_swizzled_result(
    ctx: &mut CodeGenContext,
    vector: SInst,
    swizzled: AggType,
    mask: i32,
) -> R<SInst> {
    if swizzled.0 & AggType::ELEMENT_COUNT_MASK != 0 {
        let mut components = Vec::new();
        for i in 0..4 {
            if mask & (1 << i) != 0 {
                components.push(Lit::I32(i));
            }
        }
        let t = ctx.ty(swizzled)?;
        Ok(ctx.vector_shuffle(t, vector, vector, &components))
    } else {
        let idx = (mask as u32).trailing_zeros() as i32;
        let t = ctx.ty(swizzled)?;
        Ok(ctx.composite_extract(t, vector, &[Lit::I32(idx)]))
    }
}

/// `MarkNonUniform`.
pub fn mark_non_uniform(ctx: &mut CodeGenContext, inst: SInst) {
    use super::cap_names::*;
    if ctx.host.supports_shader_non_uniform_indexing {
        if !ctx.was_non_uniform_access_declared {
            ctx.add_extension("SPV_EXT_descriptor_indexing");
            ctx.add_capability(SHADER_NON_UNIFORM);
            ctx.add_capability(SAMPLED_IMAGE_ARRAY_NON_UNIFORM_INDEXING);
            ctx.add_capability(STORAGE_IMAGE_ARRAY_NON_UNIFORM_INDEXING);
        }
        ctx.decorate(inst, decoration::NonUniform, &[]);
        ctx.was_non_uniform_access_declared = true;
    }
}
