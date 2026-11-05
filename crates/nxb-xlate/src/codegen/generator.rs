//! `SpirvGenerator`: capabilities, entry point and execution modes, and the walk over the structured program.

use super::cap_names::*;
use super::context::*;
use super::declarations;
use super::instructions;
use super::props::Props;
use super::spirv_enums::{
    addressing_model, execution_mode, execution_model, function_control_mask, loop_control_mask,
    memory_model, selection_control_mask, storage_class,
};
use crate::attr::Stage;
use crate::ir::{Inst as IrInst, IoVariable, OperandType, StorageKind};
use crate::spv::{Lit, Operand};
use crate::structured::ast::*;
use crate::structured::types::AggType;
use crate::structured::{StructInfo, helper};

type R<T> = Result<T, String>;

/// `SpirvGenerator.Generate`: the SPIR-V module bytes.
pub fn generate(info: StructInfo, props: Props) -> R<Vec<u8>> {
    if props.definitions.transform_feedback_enabled {
        return Err("unported: transform feedback".into());
    }
    let mut ctx = CodeGenContext::new(info, props);
    let stage = ctx.definitions.stage()?;
    ctx.add_capability(SHADER);
    if ctx.properties.uses_physical_pointers {
        ctx.add_capability(PHYSICAL_STORAGE_BUFFER_ADDRESSES);
        ctx.add_capability(INT64);
        ctx.add_extension("SPV_KHR_physical_storage_buffer");
        ctx.set_memory_model(
            addressing_model::PhysicalStorageBuffer64,
            memory_model::GLSL450,
        );
    } else {
        ctx.set_memory_model(addressing_model::Logical, memory_model::GLSL450);
    }
    ctx.add_capability(GROUP_NON_UNIFORM_BALLOT);
    ctx.add_capability(GROUP_NON_UNIFORM_SHUFFLE);
    ctx.add_capability(GROUP_NON_UNIFORM_VOTE);
    ctx.add_capability(IMAGE_BUFFER);
    ctx.add_capability(IMAGE_GATHER_EXTENDED);
    ctx.add_capability(IMAGE_QUERY);
    ctx.add_capability(SAMPLED_BUFFER);
    if ctx.host.supports_shader_float64 {
        ctx.add_capability(FLOAT64);
    }
    let has = |ctx: &CodeGenContext, k: StorageKind, v: IoVariable| {
        ctx.io_definitions.iter().any(|d| {
            d.storage_kind == k && d.io_variable == v && d.location == 0 && d.component == 0
        })
    };
    if stage == Stage::Fragment {
        if has(&ctx, StorageKind::Input, IoVariable::Layer)
            || has(&ctx, StorageKind::Input, IoVariable::PrimitiveId)
        {
            ctx.add_capability(GEOMETRY);
        }
        if has(&ctx, StorageKind::Input, IoVariable::SampleId) {
            ctx.add_capability(SAMPLE_RATE_SHADING);
        }
        if ctx.host.supports_fragment_shader_interlock {
            ctx.add_capability(FRAGMENT_SHADER_PIXEL_INTERLOCK_EXT);
            ctx.add_extension("SPV_EXT_fragment_shader_interlock");
        }
    } else if matches!(
        stage,
        Stage::Geometry | Stage::TessellationControl | Stage::TessellationEvaluation
    ) {
        return Err("unported: geometry and tessellation stages".into());
    } else if stage == Stage::Vertex {
        ctx.add_capability(DRAW_PARAMETERS);
    }
    if stage != Stage::Fragment
        && stage != Stage::Geometry
        && stage != Stage::Compute
        && (has(&ctx, StorageKind::Output, IoVariable::Layer)
            || has(&ctx, StorageKind::Output, IoVariable::ViewportIndex))
    {
        ctx.add_extension("SPV_EXT_shader_viewport_index_layer");
        ctx.add_capability(SHADER_VIEWPORT_INDEX_LAYER_EXT);
    }
    if has(&ctx, StorageKind::Output, IoVariable::ViewportMask) {
        ctx.add_extension("SPV_NV_viewport_array2");
        ctx.add_capability(SHADER_VIEWPORT_MASK_NV);
    }
    if ctx.helper_functions_mask & helper::SWIZZLE_ADD != 0 {
        let d = crate::structured::IoDef {
            storage_kind: StorageKind::Input,
            io_variable: IoVariable::SubgroupLaneId,
            location: 0,
            component: 0,
        };
        if !ctx.io_definitions.contains(&d) {
            ctx.io_definitions.push(d);
        }
    }
    declarations::declare_all(&mut ctx)?;

    for func_index in 0..ctx.functions.len() {
        let f = ctx.functions[func_index].clone();
        let ret = ctx.ty(f.return_type)?;
        let mut args = Vec::new();
        for arg_index in 0..f.in_arguments.len() + f.out_arguments.len() {
            let at = ctx.ty(f.argument_type(arg_index))?;
            args.push(ctx.type_pointer(storage_class::Function, at, false));
        }
        let ft = ctx.type_function(ret, false, &args);
        let spv_func = ctx.function(ret, function_control_mask::MaskNone, ft);
        if f.name.starts_with("dmsh_") {
            ctx.name(spv_func, &f.name);
        }
        ctx.spv_functions.push((func_index, spv_func));
    }
    for func_index in 0..ctx.functions.len() {
        generate_function(&mut ctx, func_index)?;
    }
    ctx.m.generate()
}

fn generate_function(ctx: &mut CodeGenContext, func_index: usize) -> R<()> {
    let function = ctx.functions[func_index].clone();
    let spv_func = ctx.spv_functions[func_index].1;
    ctx.current_function = func_index;
    ctx.add_function(spv_func);
    ctx.start_function(func_index == 0);
    declarations::declare_parameters(ctx, &function)?;
    ctx.enter_block(function.main_block);
    declarations::declare_locals(ctx, &function)?;
    generate_block(ctx, function.main_block)?;
    let ends_with_exit = ctx
        .ast
        .last(function.main_block)
        .and_then(|l| ctx.ast.op(l))
        .is_some_and(|o| o.inst == IrInst::Return || o.inst == IrInst::Discard);
    if !ends_with_exit {
        ctx.return_();
    }
    ctx.function_end();
    if func_index == 0 {
        let stage = ctx.definitions.stage()?;
        let model = match stage {
            Stage::Compute => execution_model::GLCompute,
            Stage::Vertex => execution_model::Vertex,
            Stage::Fragment => execution_model::Fragment,
            _ => return Err("unported: execution model".into()),
        };
        let iface = ctx.main_interface();
        ctx.m.add_entry_point(model, spv_func, "main", &iface);
        match stage {
            Stage::Fragment => {
                let mode = if ctx.definitions.origin_upper_left {
                    execution_mode::OriginUpperLeft
                } else {
                    execution_mode::OriginLowerLeft
                };
                ctx.m.add_execution_mode(spv_func, mode, &[]);
                if ctx.io_definitions.iter().any(|d| {
                    d.storage_kind == StorageKind::Output
                        && d.io_variable == IoVariable::FragmentOutputDepth
                        && d.location == 0
                        && d.component == 0
                }) {
                    ctx.m
                        .add_execution_mode(spv_func, execution_mode::DepthReplacing, &[]);
                }
                if ctx.definitions.early_z_force {
                    ctx.m
                        .add_execution_mode(spv_func, execution_mode::EarlyFragmentTests, &[]);
                }
                if ctx.helper_functions_mask & helper::FSI != 0
                    && ctx.host.supports_fragment_shader_interlock
                {
                    ctx.m.add_execution_mode(
                        spv_func,
                        execution_mode::PixelInterlockOrderedEXT,
                        &[],
                    );
                }
            }
            Stage::Compute => {
                let [x, y, z] = ctx.definitions.compute_local_size;
                ctx.m.add_execution_mode(
                    spv_func,
                    execution_mode::LocalSize,
                    &[
                        Operand::Lit(Lit::I32(x)),
                        Operand::Lit(Lit::I32(y)),
                        Operand::Lit(Lit::I32(z)),
                    ],
                );
            }
            _ => {}
        }
    }
    Ok(())
}

fn block_entered(ctx: &mut CodeGenContext, block: A) -> R<()> {
    let merge_block = ctx.ast.parent(block).ok_or("block without parent")?;
    match ctx.ast.block_type(block) {
        Some(BlockType::If) => {
            let if_false = match ctx.ast.next(block) {
                Some(n) if ctx.ast.block_type(n) == Some(BlockType::Else) => n,
                _ => merge_block,
            };
            let cond = ctx.ast.cond(block).ok_or("if without condition")?;
            let condition = ctx.get(AggType::BOOL, cond)?;
            let mb = ctx.next_label(merge_block);
            ctx.selection_merge(mb, selection_control_mask::MaskNone);
            let t = ctx.next_label(block);
            let f = ctx.next_label(if_false);
            ctx.branch_conditional(condition, t, f, &[]);
        }
        Some(BlockType::DoWhile) => {
            let continue_target = ctx.label();
            let nb = ctx.new_block();
            ctx.loop_targets.insert(block, (nb, continue_target));
            let mb = ctx.next_label(merge_block);
            ctx.loop_merge(mb, continue_target, loop_control_mask::MaskNone);
            let first = ctx.first_label(block);
            ctx.branch(first);
        }
        _ => {}
    }
    ctx.enter_block(block);
    Ok(())
}

fn block_left(ctx: &mut CodeGenContext, block: A) -> R<()> {
    let Some(parent) = ctx.ast.parent(block) else {
        return Ok(());
    };
    if ctx.ast.block_type(block) == Some(BlockType::DoWhile) {
        let (loop_target, continue_target) =
            *ctx.loop_targets.get(&block).ok_or("loop without targets")?;
        ctx.branch(continue_target);
        ctx.add_label(continue_target);
        let cond = ctx.ast.cond(block).ok_or("loop without condition")?;
        let condition = ctx.get(AggType::BOOL, cond)?;
        let merge = ctx.next_label(parent);
        ctx.branch_conditional(condition, loop_target, merge, &[]);
    } else {
        let last_is_cf = ctx
            .ast
            .last(block)
            .and_then(|l| ctx.ast.op(l))
            .is_some_and(|o| {
                matches!(
                    o.inst,
                    IrInst::Discard | IrInst::LoopBreak | IrInst::LoopContinue | IrInst::Return
                )
            });
        if !last_is_cf {
            let l = ctx.next_label(parent);
            ctx.branch(l);
        }
    }
    let has_else = ctx.ast.next(block).is_some_and(|n| {
        matches!(
            ctx.ast.block_type(n),
            Some(BlockType::Else) | Some(BlockType::ElseIf)
        )
    });
    if !has_else {
        ctx.enter_block(parent);
    }
    Ok(())
}

fn generate_block(ctx: &mut CodeGenContext, main: A) -> R<()> {
    ctx.loop_targets.clear();
    let mut block = main;
    let mut node = ctx.ast.first(block);
    while node.is_some() {
        while let Some(n) = node {
            if !ctx.ast.is_block(n) {
                break;
            }
            block = n;
            node = ctx.ast.first(n);
            block_entered(ctx, n)?;
        }
        if let Some(n) = node {
            let next = ctx.ast.next(n);
            process_node(ctx, n)?;
            node = next;
        }
        while node.is_none() && ctx.ast.block_type(block) != Some(BlockType::Main) {
            block_left(ctx, block)?;
            node = ctx.ast.next(block);
            block = ctx.ast.parent(block).ok_or("block without parent")?;
        }
    }
    Ok(())
}

fn process_node(ctx: &mut CodeGenContext, n: A) -> R<()> {
    match ctx.ast.kind(n).clone() {
        Kind::Assign { dest, src } => {
            let Kind::Operand {
                ty,
                var_type,
                value,
            } = ctx.ast.kind(dest).clone()
            else {
                return Err("assignment destination is not an operand".into());
            };
            match ty {
                OperandType::LocalVariable => {
                    let source = ctx.get(var_type, src)?;
                    let p = ctx.local_pointer(dest)?;
                    ctx.store(p, source, None);
                }
                OperandType::Argument => {
                    let source = ctx.get(var_type, src)?;
                    let p = ctx.argument_pointer(value)?;
                    ctx.store(p, source, None);
                }
                other => return Err(format!("{other:?}")),
            }
        }
        Kind::Operation(o) => {
            instructions::generate(ctx, &o)?;
        }
        _ => {}
    }
    Ok(())
}
