//! `BindlessToArray.RunPass` (the Vulkan path).
use super::tex::{self, BINDLESS};
use super::{Env, utils};
use crate::ir::{Block, Inst, Ir, NodeId, Opd, OperandType, StorageKind};
use crate::translation::props::TextureFlags;

type R<T> = Result<T, String>;

pub const MINIMUM_ARRAY_LENGTH: i32 = 2;

fn s(ir: &Ir, n: NodeId, i: usize) -> R<Opd> {
    ir.op(n)
        .sources
        .get(i)
        .copied()
        .flatten()
        .ok_or_else(|| "missing source".to_string())
}

fn cnst(ir: &Ir, o: Opd, v: i32) -> bool {
    ir.ty(o) == OperandType::Constant && ir.value(o) == v
}

fn op_of(ir: &Ir, o: Opd, inst: Inst) -> Option<NodeId> {
    ir.asg_operation(o).filter(|n| ir.op(*n).inst == inst)
}

pub fn run_block(ir: &mut Ir, blocks: &mut [Block], bi: usize, env: &mut Env) -> R<()> {
    let mut i = 0;
    while i < blocks[bi].ops.len() {
        let node = blocks[bi].ops[i];
        i += 1;
        let Some(t) = tex::tex(ir, node) else {
            continue;
        };
        if t.flags & BINDLESS == 0 {
            continue;
        }
        let added = process(ir, blocks, bi, i - 1, env)?;
        i += added;
    }
    Ok(())
}

/// Returns the number of nodes inserted before the texture operation.
fn process(ir: &mut Ir, blocks: &mut [Block], bi: usize, pos: usize, env: &mut Env) -> R<usize> {
    let node = blocks[bi].ops[pos];
    let src0 = s(ir, node, 0)?;
    let handle = utils::find_last_operation(ir, blocks, src0, bi, true)?;
    let Some(mut handle_op) = ir.asg_operation(handle) else {
        return Ok(0);
    };
    let (mut sec_slot, mut sec_off, mut has_secondary) = (0, 0, false);
    if ir.op(handle_op).inst == Inst::BitwiseOr {
        let a = utils::find_last_operation(ir, blocks, s(ir, handle_op, 0)?, bi, true)?;
        let b = utils::find_last_operation(ir, blocks, s(ir, handle_op, 1)?, bi, true)?;
        if ir.ty(a) == OperandType::ConstantBuffer && ir.asg_operation(b).is_some() {
            handle_op = ir.asg_operation(b).ok_or("op")?;
            sec_slot = ir.cbuf_slot(a);
            sec_off = ir.cbuf_offset(a);
            has_secondary = true;
        } else if ir.asg_operation(a).is_some() && ir.ty(b) == OperandType::ConstantBuffer {
            handle_op = ir.asg_operation(a).ok_or("op")?;
            sec_slot = ir.cbuf_slot(b);
            sec_off = ir.cbuf_offset(b);
            has_secondary = true;
        }
    }
    let h = ir.op(handle_op);
    if h.inst != Inst::Load || h.storage_kind != StorageKind::ConstantBuffer || h.sources.len() != 4
    {
        return Ok(0);
    }
    let ldc0 = s(ir, handle_op, 0)?;
    if ir.ty(ldc0) != OperandType::Constant {
        return Ok(0);
    }
    let Some(src0_slot) = env.rm.try_get_constant_buffer_slot(ir.value(ldc0)) else {
        return Ok(0);
    };
    let ldc1 = s(ir, handle_op, 1)?;
    if !cnst(ir, ldc1, 0) {
        return Ok(0);
    }
    let vec_index = s(ir, handle_op, 2)?;
    let elem_index = s(ir, handle_op, 3)?;
    if ir.ty(vec_index) != OperandType::LocalVariable
        || ir.ty(elem_index) != OperandType::LocalVariable
    {
        return Ok(0);
    }
    let (cbuf_slot, handle_index) = if has_secondary {
        (
            tex::pack_slots(src0_slot, sec_slot),
            tex::pack_offsets(0, sec_off, tex::SEPARATE_SAMPLER_HANDLE),
        )
    } else {
        (src0_slot, 0)
    };
    let length =
        MINIMUM_ARRAY_LENGTH.max(env.acc.query_texture_array_length_from_buffer(src0_slot));

    if !has_secondary
        && let Some((table_index, base_offset)) = try_array_base(ir, vec_index)?
        && base_offset / 8 >= length
    {
        turn_into_array(ir, env, node, cbuf_slot, base_offset / 4, length)?;
        let element = ir.local();
        let element_min = ir.local();
        let c3 = ir.const_(3);
        let n1 = ir.new_op(
            Inst::ShiftRightU32,
            StorageKind::None,
            Some(element),
            &[table_index, c3],
        );
        let cl = ir.const_(length - 1);
        let n2 = ir.new_op(
            Inst::MinimumU32,
            StorageKind::None,
            Some(element_min),
            &[element, cl],
        );
        blocks[bi].ops.insert(pos, n1);
        blocks[bi].ops.insert(pos + 1, n2);
        ir.set_source(node, 0, Some(element_min));
        return Ok(2);
    }

    turn_into_array(ir, env, node, cbuf_slot, handle_index, length)?;
    let (vi, ei, idx, idx_min) = (ir.local(), ir.local(), ir.local(), ir.local());
    let c1a = ir.const_(1);
    let c1b = ir.const_(1);
    let cl = ir.const_(length - 1);
    let n1 = ir.new_op(
        Inst::ShiftLeft,
        StorageKind::None,
        Some(vi),
        &[vec_index, c1a],
    );
    let n2 = ir.new_op(
        Inst::ShiftRightU32,
        StorageKind::None,
        Some(ei),
        &[elem_index, c1b],
    );
    let n3 = ir.new_op(Inst::Add, StorageKind::None, Some(idx), &[vi, ei]);
    let n4 = ir.new_op(
        Inst::MinimumU32,
        StorageKind::None,
        Some(idx_min),
        &[idx, cl],
    );
    for (k, n) in [n1, n2, n3, n4].into_iter().enumerate() {
        blocks[bi].ops.insert(pos + k, n);
    }
    ir.set_source(node, 0, Some(idx_min));
    Ok(4)
}

fn try_array_base(ir: &Ir, vec_index: Opd) -> R<Option<(Opd, i32)>> {
    let shift_by_2 = |o: Opd| -> R<Option<Opd>> {
        if let Some(op) = op_of(ir, o, Inst::ShiftRightU32) {
            let c = s(ir, op, 1)?;
            if cnst(ir, c, 2) {
                return Ok(Some(s(ir, op, 0)?));
            }
        }
        Ok(None)
    };
    let Some(mut word) = shift_by_2(vec_index)? else {
        return Ok(None);
    };
    if let Some(plus0) = op_of(ir, word, Inst::Add) {
        let c = s(ir, plus0, 1)?;
        if cnst(ir, c, 0) {
            word = s(ir, plus0, 0)?;
        }
    }
    let Some(address) = shift_by_2(word)? else {
        return Ok(None);
    };
    let Some(add) = op_of(ir, address, Inst::Add) else {
        return Ok(None);
    };
    let c = s(ir, add, 1)?;
    if ir.ty(c) != OperandType::Constant {
        return Ok(None);
    }
    let index = s(ir, add, 0)?;
    let byte_offset = ir.value(c);
    if byte_offset > 0 && (byte_offset & 7) == 0 {
        Ok(Some((index, byte_offset)))
    } else {
        Ok(None)
    }
}

fn turn_into_array(
    ir: &mut Ir,
    env: &mut Env,
    node: NodeId,
    cbuf_slot: i32,
    handle_index: i32,
    length: i32,
) -> R<()> {
    let t = tex::tex(ir, node).ok_or("texture operation")?.clone();
    let inst = ir.op(node).inst;
    let sb = env.rm.get_texture_or_image_binding(
        env.acc,
        inst,
        t.ty,
        t.format,
        TextureFlags(t.flags & !BINDLESS),
        cbuf_slot,
        handle_index,
        length,
        false,
    )?;
    tex::turn_into_array(ir, node, sb)
}
