//! `Translator.EmitOpsAmpere` and `DeferAmpereWrites`: guards and the scoreboard model with deferred register writes.

use crate::emit;
use crate::ir::emitter::EmitterContext;
use crate::ir::inst::{Inst, StorageKind};
use crate::ir::ops::{OperandType, regtype};
use crate::ir::{NodeId, Opd};
use crate::maxwell::ampere::{AmpOp, opc};
use crate::maxwell::ampere_table::has_uniform_guard;
use crate::maxwell::decoder::Block;
use crate::maxwell::table::props;

/// (index, type) as `Ir::reg_of`
type Reg = (i32, i32);

const SHADOWS_COUNT: i32 = 256;

#[derive(Default)]
struct Scoreboard {
    pending: [Vec<(Reg, Opd)>; 7],
    used: [bool; 7],
    shadows: i32,
    free: Vec<i32>,
}

impl Scoreboard {
    fn commit(&mut self, ctx: &mut EmitterContext<'_, '_>, barrier: usize) {
        if !self.used[barrier] {
            return;
        }
        for (reg, value) in std::mem::take(&mut self.pending[barrier]) {
            let r = ctx.ir.register(reg.0, reg.1);
            ctx.copy_to(r, value);
            self.free.push(ctx.ir.reg_of(value).0);
        }
        self.used[barrier] = false;
    }

    fn commit_all(&mut self, ctx: &mut EmitterContext<'_, '_>) {
        for b in 0..7 {
            self.commit(ctx, b);
        }
    }
}

/// `EmitOpsAmpere`.
pub fn emit_ops_ampere(ctx: &mut EmitterContext<'_, '_>, block: &Block) -> Result<(), String> {
    let mut sb = Scoreboard::default();
    for (op_index, op) in block.ops.iter().enumerate() {
        let a = AmpOp::of(op);
        let wait = a.bits(116, 122);
        let write_barrier = a.bits(110, 113);
        for b in 0..6 {
            if wait & (1 << b) != 0 {
                sb.commit(ctx, b);
            }
        }
        if a.opcode() == 0x91a || op.has(props::BRA) {
            sb.commit_all(ctx);
        }
        if ctx.tc.options.debug_mode {
            ctx.comment(&format!(
                "0x{:06X}: 0x{:016X}{:016X} {:?}",
                op.address, op.raw_hi, op.raw, op.name
            ));
        }
        if a.pred() == 7 && a.pred_inv() {
            continue;
        }
        let op_start = ctx.operations_count();
        let mut pred_skip = None;
        if a.pred() != 7 && a.opcode() != opc::BRA {
            let label = if op_index == block.ops.len() - 1 && block.has_next() {
                let succ = *block.succs.first().ok_or("block without successor")?;
                ctx.get_label(ctx.block_address(succ))
            } else {
                let l = ctx.ir.label();
                pred_skip = Some(l);
                l
            };
            let ty = if has_uniform_guard(a.opcode()) {
                regtype::UNIFORM_PREDICATE
            } else {
                regtype::PREDICATE
            };
            let pred = ctx.ir.register(a.pred(), ty);
            if a.pred_inv() {
                ctx.branch_if_true(label, pred);
            } else {
                ctx.branch_if_false(label, pred);
            }
        }
        ctx.curr_op = *op;
        let Some(e) = op.aemit else {
            return Err("Ampere op without emitter".into());
        };
        if let Err(m) = emit::ampere::dispatch(ctx, e) {
            if !ctx.lenient {
                return Err(m);
            }
            ctx.errors.push((format!("{:?}", op.name), m));
        }
        if let Some(l) = pred_skip {
            ctx.mark_label(l);
        }
        if write_barrier < 6 && !op.has(props::BRA) {
            defer_writes(ctx, op_start, &mut sb, write_barrier as usize)?;
        }
    }
    sb.commit_all(ctx);
    Ok(())
}

fn reg_operand(ctx: &EmitterContext<'_, '_>, o: Option<Opd>) -> Option<(Opd, Reg)> {
    let o = o?;
    if ctx.ir.ty(o) == OperandType::Register {
        Some((o, ctx.ir.reg_of(o)))
    } else {
        None
    }
}

/// `DeferAmpereWrites`: the registers written by the operations from `op_start` on become shadow registers.
fn defer_writes(
    ctx: &mut EmitterContext<'_, '_>,
    op_start: usize,
    sb: &mut Scoreboard,
    barrier: usize,
) -> Result<(), String> {
    sb.used[barrier] = true;
    let end = ctx.operations_count();
    // register -> shadow operand, in first-write order
    let mut locals: Vec<(Reg, Opd)> = Vec::new();
    for i in op_start..end {
        let n: NodeId = ctx.operation(i);
        for d in ctx.ir.node_dests(n) {
            if let Some((_, reg)) = reg_operand(ctx, d)
                && !locals.iter().any(|(r, _)| *r == reg)
            {
                if sb.free.is_empty() && sb.shadows == SHADOWS_COUNT {
                    return Err(
                        "Ampere: more deferred register writes pending at once than shadow registers"
                            .into(),
                    );
                }
                let idx = sb.free.pop().unwrap_or_else(|| {
                    sb.shadows += 1;
                    sb.shadows - 1
                });
                let s = ctx.ir.register(idx, regtype::SHADOW);
                locals.push((reg, s));
            }
        }
    }
    if locals.is_empty() {
        return Ok(());
    }
    for i in op_start..end {
        let n = ctx.operation(i);
        for (d, dest) in ctx.ir.node_dests(n).into_iter().enumerate() {
            if let Some((_, reg)) = reg_operand(ctx, dest)
                && let Some((_, l)) = locals.iter().find(|(r, _)| *r == reg)
            {
                ctx.ir.set_dest(n, d, Some(*l));
            }
        }
        for (s, src) in ctx.ir.node_sources(n).into_iter().enumerate() {
            if let Some((_, reg)) = reg_operand(ctx, src)
                && let Some((_, l)) = locals.iter().find(|(r, _)| *r == reg)
            {
                ctx.ir.set_node_source(n, s, Some(*l));
            }
        }
    }
    for ((reg, local), at) in locals.into_iter().zip(op_start..) {
        let r = ctx.ir.register(reg.0, reg.1);
        let copy = ctx
            .ir
            .new_op(Inst::Copy, StorageKind::None, Some(local), &[r]);
        ctx.insert_operation(at, copy);
        // a register written again before its earlier deferred write landed: the later write wins, the earlier shadow is free
        for list in &mut sb.pending {
            let mut k = 0;
            while k < list.len() {
                if list[k].0 == reg {
                    let v = list.remove(k).1;
                    let idx = ctx.ir.reg_of(v).0;
                    sb.free.push(idx);
                } else {
                    k += 1;
                }
            }
        }
        sb.pending[barrier].push((reg, local));
    }
    Ok(())
}
