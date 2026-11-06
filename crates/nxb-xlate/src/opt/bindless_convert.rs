//! `BindlessElimination.TryConvertBindless` and its helpers.
use super::tex::{self, ATOMIC_METAL, BINDLESS, INT_COORDS, LOD_LEVEL};
use super::{Env, utils};
use crate::ir::{Block, Inst, Ir, NodeId, Opd, OperandType};
use crate::translation::props::{ShaderProperties, TextureFlags};
use crate::types::SamplerType;
use std::collections::HashSet;

type R<T> = Result<T, String>;

fn s(ir: &Ir, n: NodeId, i: usize) -> R<Opd> {
    ir.op(n)
        .sources
        .get(i)
        .copied()
        .flatten()
        .ok_or_else(|| "missing source".to_string())
}

fn asg_phi(ir: &Ir, o: Opd) -> Option<NodeId> {
    ir.data(o).asg_op.filter(|n| ir.is_phi(*n))
}

fn op_of(ir: &Ir, o: Opd, inst: Inst) -> Option<NodeId> {
    ir.asg_operation(o).filter(|n| ir.op(*n).inst == inst)
}

pub(super) fn try_convert_bindless(
    ir: &mut Ir,
    blocks: &[Block],
    bi: usize,
    env: &mut Env,
    node: NodeId,
) -> R<bool> {
    let inst = ir.op(node).inst;
    let t = tex::tex(ir, node).ok_or("tex")?.clone();
    if inst == Inst::TextureSample || tex::is_texture_query(inst) {
        let mut handle = s(ir, node, 0)?;
        if let Some(sh) = op_of(ir, handle, Inst::Shuffle) {
            handle = s(ir, sh, 0)?;
        }
        handle = utils::find_last_operation(ir, blocks, handle, bi, true)?;
        let rewrite = t.ty == SamplerType::TEXTURE_BUFFER
            || inst == Inst::TextureQuerySamples
            || inst == Inst::TextureQuerySize;
        if ir.ty(handle) == OperandType::ConstantBuffer {
            set_handle(
                ir,
                env,
                node,
                ir.cbuf_offset(handle),
                ir.cbuf_slot(handle),
                rewrite,
                false,
            )?;
            return Ok(true);
        }
        let Some(combine) = try_get_operation(ir, ir.data(handle).asg_op) else {
            return Ok(false);
        };
        if ir.op(combine).inst != Inst::BitwiseOr {
            return Ok(false);
        }
        let mut src0 = utils::find_last_operation(ir, blocks, s(ir, combine, 0)?, bi, true)?;
        let mut src1 = utils::find_last_operation(ir, blocks, s(ir, combine, 1)?, bi, true)?;
        if ir.ty(src0) == OperandType::Constant && ir.ty(src1) != OperandType::Constant {
            std::mem::swap(&mut src0, &mut src1);
        }
        // The compiler ORs the two words in either order. A word in c[2]'s separate sampler table is the sampler.
        if super::bindless_elim::is_sampler_word(ir, src0)
            && !super::bindless_elim::is_sampler_word(ir, src1)
        {
            std::mem::swap(&mut src0, &mut src1);
        }
        let mut handle_type = tex::SEPARATE_SAMPLER_HANDLE;
        if let Some(a0) = ir.asg_operation(src0) {
            if let Some(a1) = ir.asg_operation(src1)
                && ir.op(a0).inst == Inst::BitwiseAnd
                && ir.op(a1).inst == Inst::BitwiseAnd
            {
                let mut n0 = masked_handle(ir, a0, 0xFFFFF)?;
                let mut n1 = masked_handle(ir, a1, 0xFFF00000)?;
                if n0.is_none() || n1.is_none() {
                    n0 = masked_handle(ir, a1, 0xFFFFF)?;
                    n1 = masked_handle(ir, a0, 0xFFF00000)?;
                }
                match (n0, n1) {
                    (Some(x), Some(y)) => {
                        src0 = x;
                        src1 = y;
                    }
                    _ => return Ok(false),
                }
            } else if ir.op(a0).inst == Inst::ShiftLeft {
                let shift = s(ir, a0, 1)?;
                if ir.ty(shift) == OperandType::Constant && ir.value(shift) == 20 {
                    let new1 = s(ir, a0, 0)?;
                    src0 = src1;
                    src1 = new1;
                    handle_type = tex::SEPARATE_SAMPLER_ID;
                }
            }
        } else if let Some(a1) = op_of(ir, src1, Inst::ShiftLeft) {
            let shift = s(ir, a1, 1)?;
            if ir.ty(shift) == OperandType::Constant && ir.value(shift) == 20 {
                src1 = s(ir, a1, 0)?;
                handle_type = tex::SEPARATE_SAMPLER_ID;
            }
        } else if ir.ty(src1) == OperandType::Constant && (ir.value(src1) & 0xfffff) == 0 {
            handle_type = tex::SEPARATE_CONSTANT_SAMPLER_HANDLE;
        }
        if ir.ty(src0) != OperandType::ConstantBuffer {
            return Ok(false);
        }
        if handle_type == tex::SEPARATE_CONSTANT_SAMPLER_HANDLE {
            let off = tex::pack_offsets(
                ir.cbuf_offset(src0),
                (ir.value(src1) >> 20) & 0xfff,
                handle_type,
            );
            let slots = tex::pack_slots(ir.cbuf_slot(src0), 0);
            set_handle(ir, env, node, off, slots, rewrite, false)?;
            return Ok(true);
        } else if ir.ty(src1) == OperandType::ConstantBuffer {
            let off = tex::pack_offsets(ir.cbuf_offset(src0), ir.cbuf_offset(src1), handle_type);
            let slots = tex::pack_slots(ir.cbuf_slot(src0), ir.cbuf_slot(src1));
            set_handle(ir, env, node, off, slots, rewrite, false)?;
            return Ok(true);
        }
    } else if tex::is_image(inst) {
        let src0 = utils::find_last_operation(ir, blocks, s(ir, node, 0)?, bi, true)?;
        if ir.ty(src0) == OperandType::ConstantBuffer {
            let (cbuf_offset, cbuf_slot) = (ir.cbuf_offset(src0), ir.cbuf_slot(src0));
            if t.format == 0 {
                let f = if inst == Inst::ImageAtomic {
                    ShaderProperties::get_texture_format_atomic(
                        env.acc,
                        cbuf_offset,
                        cbuf_slot,
                        t.flags & ATOMIC_METAL != 0,
                    )?
                } else {
                    ShaderProperties::get_texture_format(env.acc, cbuf_offset, cbuf_slot)
                };
                tex::tex_mut(ir, node)?.format = f;
            }
            let rewrite = t.ty == SamplerType::TEXTURE_BUFFER;
            set_handle(ir, env, node, cbuf_offset, cbuf_slot, rewrite, true)?;
            return Ok(true);
        }
    }
    Ok(false)
}

fn try_get_operation(ir: &Ir, asg: Option<NodeId>) -> Option<NodeId> {
    let mut asg = asg;
    if let Some(phi) = asg.filter(|n| ir.is_phi(*n)) {
        let mut first: Option<Opd> = None;
        let mut visited: HashSet<NodeId> = HashSet::new();
        let mut pending = vec![phi];
        while let Some(cur) = pending.pop() {
            if !visited.insert(cur) {
                continue;
            }
            for src in ir.node_sources(cur).into_iter().flatten() {
                if let Some(sp) = asg_phi(ir, src) {
                    pending.push(sp);
                } else if let Some(f) = first {
                    if !is_same_operand(ir, f, src) {
                        return None;
                    }
                } else {
                    first = Some(src);
                }
            }
        }
        let f = first?;
        asg = ir.data(f).asg_op;
    }
    asg.filter(|n| ir.is_op(*n))
}

fn both_cbuf(ir: &Ir, x: Opd, y: Opd) -> bool {
    ir.ty(x) == ir.ty(y) && ir.value(x) == ir.value(y) && ir.ty(x) == OperandType::ConstantBuffer
}

fn both_const(ir: &Ir, x: Opd, y: Opd) -> bool {
    ir.ty(x) == ir.ty(y) && ir.value(x) == ir.value(y) && ir.ty(x) == OperandType::Constant
}

fn is_same_operand(ir: &Ir, x: Opd, y: Opd) -> bool {
    if x == y {
        return true;
    }
    if ir.ty(x) == ir.ty(y) && ir.ty(x) == OperandType::LocalVariable {
        let (Some(xo), Some(yo)) = (ir.asg_operation(x), ir.asg_operation(y)) else {
            return false;
        };
        if ir.op(xo).inst != Inst::BitwiseOr || ir.op(yo).inst != Inst::BitwiseOr {
            return false;
        }
        let g = |n: NodeId, i: usize| ir.op(n).sources.get(i).copied().flatten();
        let (Some(x0), Some(y0), Some(x1), Some(y1)) = (g(xo, 0), g(yo, 0), g(xo, 1), g(yo, 1))
        else {
            return false;
        };
        return both_cbuf(ir, x0, y0) && (both_cbuf(ir, x1, y1) || both_const(ir, x1, y1));
    }
    false
}

fn masked_handle(ir: &Ir, asg: NodeId, mask: u32) -> R<Option<Opd>> {
    let (a, b) = (s(ir, asg, 0)?, s(ir, asg, 1)?);
    let (ta, tb) = (ir.ty(a), ir.ty(b));
    if ta == OperandType::ConstantBuffer && tb == OperandType::ConstantBuffer {
        return Ok(Some(if ir.cbuf_slot(a) == 1 { b } else { a }));
    } else if ta == OperandType::ConstantBuffer && tb == OperandType::Constant {
        if ir.value(b) as u32 == mask {
            return Ok(Some(a));
        }
    } else if ta == OperandType::Constant
        && tb == OperandType::ConstantBuffer
        && ir.value(a) as u32 == mask
    {
        return Ok(Some(b));
    }
    Ok(None)
}

fn set_handle(
    ir: &mut Ir,
    env: &mut Env,
    node: NodeId,
    cbuf_offset: i32,
    cbuf_slot: i32,
    rewrite_sampler_type: bool,
    is_image: bool,
) -> R<()> {
    let inst = ir.op(node).inst;
    if rewrite_sampler_type {
        let mut new_type = env.acc.query_sampler_type(cbuf_offset, cbuf_slot);
        let t = tex::tex(ir, node).ok_or("tex")?.clone();
        if tex::is_texture_query(inst) {
            tex::tex_mut(ir, node)?.ty = new_type;
        } else if t.ty == SamplerType::TEXTURE_BUFFER && new_type == SamplerType::TEXTURE_1D {
            let mut coords_count = 2usize;
            // InstEmit.Sample1DAs2D is true
            new_type = SamplerType::TEXTURE_2D;
            let c = ir.const_(0);
            ir.insert_source(node, coords_count, c);
            coords_count += 1;
            if !is_image && (t.flags & INT_COORDS) != 0 && (t.flags & LOD_LEVEL) == 0 {
                tex::tex_mut(ir, node)?.flags |= LOD_LEVEL;
                let c = ir.const_(0);
                ir.insert_source(node, coords_count, c);
            }
            tex::tex_mut(ir, node)?.ty = new_type;
        }
    }
    let t = tex::tex(ir, node).ok_or("tex")?.clone();
    let sb = env.rm.get_texture_or_image_binding(
        env.acc,
        inst,
        t.ty,
        t.format,
        TextureFlags(t.flags & !BINDLESS),
        cbuf_slot,
        cbuf_offset,
        1,
        false,
    )?;
    tex::set_binding(ir, node, sb)
}
