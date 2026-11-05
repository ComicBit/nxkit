//! Control flow of `InstEmitAmpere.cs`: BRA, BRX, EXIT, KILL, CALL, RET.

use super::operand::*;
use crate::maxwell::ampere::{AmpOp, opc};
use crate::maxwell::ampere_decode::{INST_SIZE, is_jump_call, target};
use crate::maxwell::ampere_table::has_uniform_guard;

pub(super) fn nop(_ctx: &mut Ctx) -> R {
    Ok(())
}

/// The guard (12..15) and the condition predicate (87..90) of a branch-like instruction, ANDed.
fn guard_and_condition(
    ctx: &mut Ctx,
    op: &AmpOp,
    uniform_condition: bool,
) -> Option<crate::ir::Opd> {
    let mut result = None;
    if op.pred() != PT || op.pred_inv() {
        let g = pred_reg(ctx, op.pred(), has_uniform_guard(op.opcode()));
        result = Some(if op.pred_inv() { ctx.bitwise_not(g) } else { g });
    }
    if op.bits(87, 90) != PT || op.bit(90) {
        let c = pred(ctx, op, 87, 90, uniform_condition);
        result = Some(match result {
            None => c,
            Some(r) => ctx.bitwise_and(r, c),
        });
    }
    result
}

pub(super) fn bra(ctx: &mut Ctx) -> R {
    let cur = ctx.curr_op;
    let op = AmpOp::of(&cur);
    let t = target(&cur);
    if t == cur.address + INST_SIZE {
        return Ok(());
    }
    let label = ctx.get_label(t);
    match guard_and_condition(ctx, &op, op.bit(91)) {
        None => ctx.branch(label),
        Some(c) => ctx.branch_if_true(label, c),
    }
    Ok(())
}

pub(super) fn brx(ctx: &mut Ctx) -> R {
    let cur = ctx.curr_op;
    let op = AmpOp::of(&cur);
    let blk = ctx.curr_block.ok_or("no current block")?;
    let func = ctx.curr_function.ok_or("no current function")?;
    let start = usize::from(blk.has_next());
    if blk.succs.len() <= start {
        return Err(fail(ctx, "BRX with no target found in the c1 jump table"));
    }
    let next = cur.address as i64 + INST_SIZE as i64 + op.branch_offset();
    let ty = if op.opcode() == opc::BRXU {
        crate::ir::ops::regtype::UNIFORM
    } else {
        crate::ir::ops::regtype::GPR
    };
    let r = ctx.ir.register(op.ra(), ty);
    let n = ctx.ir.const_(next as i32);
    let address = ctx.i_add(r, n);
    let mut targets: Vec<u64> = blk.succs[start..]
        .iter()
        .map(|&i| func.blocks[i].address)
        .collect();
    targets.sort();
    for (i, t) in targets.iter().enumerate() {
        let label = ctx.get_label(*t);
        if i == targets.len() - 1 {
            ctx.branch(label);
        } else {
            let c = ctx.ir.const_(*t as i32);
            let eq = ctx.i_compare_equal(address, c);
            ctx.branch_if_true(label, eq);
        }
    }
    Ok(())
}

pub(super) fn exit(ctx: &mut Ctx) -> R {
    let op = x(ctx);
    if ctx.is_non_main {
        return Err(fail(ctx, "EXIT inside a called function"));
    }
    let mut skip = None;
    if op.bits(87, 90) != PT || op.bit(90) {
        let l = ctx.ir.label();
        skip = Some(l);
        let p = pred(ctx, &op, 87, 90, false);
        ctx.branch_if_false(l, p);
    }
    if ctx.prepare_for_return()? {
        ctx.return_();
    }
    if let Some(l) = skip {
        ctx.mark_label(l);
    }
    Ok(())
}

pub(super) fn kill(ctx: &mut Ctx) -> R {
    let op = x(ctx);
    if op.bits(87, 90) != PT || op.bit(90) {
        let l = ctx.ir.label();
        let p = pred(ctx, &op, 87, 90, false);
        ctx.branch_if_false(l, p);
        ctx.discard();
        ctx.mark_label(l);
    } else {
        ctx.discard();
    }
    Ok(())
}

pub(super) fn call(ctx: &mut Ctx) -> R {
    let cur = ctx.curr_op;
    if is_jump_call(&ctx.tc.program, &cur) {
        let l = ctx.get_label(target(&cur));
        ctx.branch(l);
        return Ok(());
    }
    let t = target(&cur);
    let id = ctx
        .function_addresses
        .iter()
        .position(|a| *a == t)
        .ok_or("CALL to an unknown function")?;
    ctx.call(id as i32, false, &[]);
    Ok(())
}

pub(super) fn ret(ctx: &mut Ctx) -> R {
    if !ctx.is_non_main {
        return Err(fail(ctx, "RET in the main function"));
    }
    ctx.return_();
    Ok(())
}
