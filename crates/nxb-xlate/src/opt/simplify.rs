//! `Simplification`.
use crate::ir::{Inst, Ir, NodeId, Opd, OperandType};

const ALL_ONES: i32 = !0;

fn s(ir: &Ir, n: NodeId, i: usize) -> Result<Opd, String> {
    ir.op(n)
        .sources
        .get(i)
        .copied()
        .flatten()
        .ok_or_else(|| "missing source".to_string())
}

fn is_const_equal(ir: &Ir, o: Opd, v: i32) -> bool {
    ir.ty(o) == OperandType::Constant && ir.value(o) == v
}

pub fn is_comparison(inst: Inst) -> bool {
    matches!(
        Inst(inst.0 & Inst::MASK),
        Inst::CompareEqual
            | Inst::CompareGreater
            | Inst::CompareGreaterOrEqual
            | Inst::CompareGreaterOrEqualU32
            | Inst::CompareGreaterU32
            | Inst::CompareLess
            | Inst::CompareLessOrEqual
            | Inst::CompareLessOrEqualU32
            | Inst::CompareLessU32
            | Inst::CompareNotEqual
    )
}

pub fn run_pass(ir: &mut Ir, n: NodeId) -> Result<(), String> {
    if !ir.is_op(n) {
        return Ok(());
    }
    match ir.op(n).inst {
        Inst::Add => commutative(ir, n, 0),
        Inst::BitwiseAnd => bitwise_and(ir, n),
        Inst::BitwiseExclusiveOr => {
            if !xor_swap(ir, n)? {
                commutative(ir, n, 0)
            } else {
                Ok(())
            }
        }
        Inst::BitwiseOr => bitwise_or(ir, n),
        Inst::CompareNotEqual => compare_not_equal(ir, n),
        Inst::ConditionalSelect => conditional_select(ir, n),
        Inst::Divide => binary_op_y(ir, n, 1),
        Inst::Multiply => commutative(ir, n, 1),
        Inst::ShiftLeft | Inst::ShiftRightS32 | Inst::ShiftRightU32 | Inst::Subtract => {
            binary_op_y(ir, n, 0)
        }
        _ => Ok(()),
    }
}

fn bitwise_and(ir: &mut Ir, n: NodeId) -> Result<(), String> {
    let (x, y) = (s(ir, n, 0)?, s(ir, n, 1)?);
    if is_const_equal(ir, x, ALL_ONES) {
        ir.turn_into_copy(n, y);
    } else if is_const_equal(ir, y, ALL_ONES) {
        ir.turn_into_copy(n, x);
    } else if is_const_equal(ir, x, 0) || is_const_equal(ir, y, 0) {
        let c = ir.const_(0);
        ir.turn_into_copy(n, c);
    }
    Ok(())
}

fn xor_swap(ir: &mut Ir, x_copy: NodeId) -> Result<bool, String> {
    let (x, y) = (s(ir, x_copy, 0)?, s(ir, x_copy, 1)?);
    let (Some(t_copy), Some(y_copy)) = (ir.asg_operation(x), ir.asg_operation(y)) else {
        return Ok(false);
    };
    if ir.op(t_copy).inst != Inst::BitwiseExclusiveOr
        || ir.op(y_copy).inst != Inst::BitwiseExclusiveOr
    {
        return Ok(false);
    }
    if t_copy == y_copy {
        return Ok(false);
    }
    if s(ir, y_copy, 0)? != x
        || s(ir, y_copy, 1)? != s(ir, t_copy, 1)?
        || ir.data(x).use_ops.len() != 2
    {
        return Ok(false);
    }
    let x = s(ir, t_copy, 0)?;
    let y = s(ir, t_copy, 1)?;
    ir.turn_into_copy(t_copy, y);
    ir.turn_into_copy(y_copy, x);
    let td = ir
        .op(t_copy)
        .dests
        .first()
        .copied()
        .flatten()
        .ok_or("dest")?;
    ir.turn_into_copy(x_copy, td);
    Ok(true)
}

fn bitwise_or(ir: &mut Ir, n: NodeId) -> Result<(), String> {
    let (x, y) = (s(ir, n, 0)?, s(ir, n, 1)?);
    if is_const_equal(ir, x, 0) {
        ir.turn_into_copy(n, y);
    } else if is_const_equal(ir, y, 0) {
        ir.turn_into_copy(n, x);
    } else if is_const_equal(ir, x, ALL_ONES) || is_const_equal(ir, y, ALL_ONES) {
        let c = ir.const_(ALL_ONES);
        ir.turn_into_copy(n, c);
    }
    Ok(())
}

fn binary_op_y(ir: &mut Ir, n: NodeId, comparand: i32) -> Result<(), String> {
    let (x, y) = (s(ir, n, 0)?, s(ir, n, 1)?);
    if is_const_equal(ir, y, comparand) {
        ir.turn_into_copy(n, x);
    }
    Ok(())
}

fn commutative(ir: &mut Ir, n: NodeId, comparand: i32) -> Result<(), String> {
    let (x, y) = (s(ir, n, 0)?, s(ir, n, 1)?);
    if is_const_equal(ir, x, comparand) {
        ir.turn_into_copy(n, y);
    } else if is_const_equal(ir, y, comparand) {
        ir.turn_into_copy(n, x);
    }
    Ok(())
}

fn compare_not_equal(ir: &mut Ir, n: NodeId) -> Result<(), String> {
    let (mut lhs, mut rhs) = (s(ir, n, 0)?, s(ir, n, 1)?);
    if ir.ty(lhs) == OperandType::Constant {
        std::mem::swap(&mut lhs, &mut rhs);
    }
    if ir.ty(rhs) != OperandType::Constant || ir.value(rhs) != 0 {
        return Ok(());
    }
    let Some(cmp) = ir.asg_operation(lhs) else {
        return Ok(());
    };
    if !is_comparison(ir.op(cmp).inst) {
        return Ok(());
    }
    ir.turn_into_copy(n, lhs);
    Ok(())
}

fn conditional_select(ir: &mut Ir, n: NodeId) -> Result<(), String> {
    let cond = s(ir, n, 0)?;
    if ir.ty(cond) != OperandType::Constant {
        return Ok(());
    }
    let idx = if ir.value(cond) != 0 { 1 } else { 2 };
    let source = s(ir, n, idx)?;
    ir.turn_into_copy(n, source);
    Ok(())
}
