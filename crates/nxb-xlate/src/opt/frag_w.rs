//! `Optimizer.EliminateMultiplyByFragmentCoordW`.
use super::fold::fp;
use super::utils::{is_input_load, is_input_load_of};
use crate::ir::{Block, Inst, IoVariable, Ir, NodeId, OperandType};

fn src(ir: &Ir, n: NodeId, i: usize) -> Result<crate::ir::Opd, String> {
    ir.op(n)
        .sources
        .get(i)
        .copied()
        .flatten()
        .ok_or_else(|| "missing source".to_string())
}

pub fn run_block(ir: &mut Ir, block: &Block) -> Result<(), String> {
    for &n in &block.ops {
        if ir.is_op(n) {
            run_op(ir, n)?;
        }
    }
    Ok(())
}

fn run_op(ir: &mut Ir, op: NodeId) -> Result<(), String> {
    if ir.op(op).inst != fp(Inst::Multiply) {
        return Ok(());
    }
    let (lhs, rhs) = (src(ir, op, 0)?, src(ir, op, 1)?);
    let Some(attr_mul) = ir.asg_operation(lhs) else {
        return Ok(());
    };
    if ir.op(attr_mul).inst != fp(Inst::Multiply) {
        return Ok(());
    }
    let (attr_lhs, attr_rhs) = (src(ir, attr_mul, 0)?, src(ir, attr_mul, 1)?);
    let asg = |o| ir.data(o).asg_op;
    if !is_input_load(ir, asg(attr_lhs))
        || !is_input_load_of(ir, asg(attr_rhs), IoVariable::FragmentCoord, 3)
    {
        return Ok(());
    }
    let Some(recip) = ir.asg_operation(rhs) else {
        return Ok(());
    };
    if ir.op(recip).inst != fp(Inst::Divide) {
        return Ok(());
    }
    let (rl, rr) = (src(ir, recip, 0)?, src(ir, recip, 1)?);
    if ir.ty(rl) != OperandType::Constant || ir.as_float(rl) != 1.0 {
        return Ok(());
    }
    if !is_input_load_of(ir, asg(rr), IoVariable::FragmentCoord, 3) {
        return Ok(());
    }
    ir.turn_into_copy(op, attr_lhs);
    Ok(())
}
