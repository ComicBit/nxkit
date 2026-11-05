//! `XmadOptimizer`.
use crate::ir::{Block, Inst, Ir, NodeId, Opd, OperandType, StorageKind};

type R<T> = Result<T, String>;

fn s(ir: &Ir, n: NodeId, i: usize) -> R<Opd> {
    ir.op(n)
        .sources
        .get(i)
        .copied()
        .flatten()
        .ok_or_else(|| "missing source".to_string())
}

fn is_const(ir: &Ir, o: Opd, v: i32) -> bool {
    ir.ty(o) == OperandType::Constant && ir.value(o) == v
}

fn is_copy(ir: &Ir, n: NodeId) -> R<bool> {
    let inst = ir.op(n).inst;
    Ok(inst == Inst::Copy || (inst == Inst::Add && is_const(ir, s(ir, n, 1)?, 0)))
}

fn copy_source(ir: &Ir, v: Option<Opd>) -> R<Opd> {
    let mut v = v.ok_or("null reference in XmadOptimizer")?;
    while let Some(op) = ir.asg_operation(v) {
        if !is_copy(ir, op)? {
            break;
        }
        v = s(ir, op, 0)?;
    }
    Ok(v)
}

fn masked16(ir: &Ir, v: Opd) -> R<Option<Opd>> {
    let Some(m) = ir.asg_operation(v) else {
        return Ok(None);
    };
    if ir.op(m).inst != Inst::BitwiseAnd || !is_const(ir, s(ir, m, 1)?, 0xffff) {
        return Ok(None);
    }
    Ok(Some(s(ir, m, 0)?))
}

fn shifted16(ir: &Ir, v: Opd, inst: Inst) -> R<Option<Opd>> {
    let Some(m) = ir.asg_operation(v) else {
        return Ok(None);
    };
    if ir.op(m).inst != inst || !is_const(ir, s(ir, m, 1)?, 16) {
        return Ok(None);
    }
    Ok(Some(s(ir, m, 0)?))
}

struct Match {
    x: Opd,
    y: Opd,
    addend: Option<Opd>,
}

fn low_times_low(ir: &Ir, mut op: NodeId) -> R<Option<Match>> {
    let mut addend = None;
    if ir.op(op).inst == Inst::Add {
        let Some(mul) = ir.asg_operation(s(ir, op, 0)?) else {
            return Ok(None);
        };
        addend = Some(s(ir, op, 1)?);
        op = mul;
    }
    if ir.op(op).inst != Inst::Multiply {
        return Ok(None);
    }
    let a = masked16(ir, s(ir, op, 0)?)?;
    let b = masked16(ir, s(ir, op, 1)?)?;
    match (a, b) {
        (Some(x), Some(y)) => Ok(Some(Match { x, y, addend })),
        _ => Ok(None),
    }
}

fn low_times_high(ir: &Ir, op: NodeId, x: Opd, y: Opd) -> R<bool> {
    if ir.op(op).inst != Inst::BitwiseOr {
        return Ok(false);
    }
    let Some(mul_result) = masked16(ir, s(ir, op, 0)?)? else {
        return Ok(false);
    };
    let mul_result = copy_source(ir, Some(mul_result))?;
    let Some(mul) = ir.asg_operation(mul_result) else {
        return Ok(false);
    };
    if ir.op(mul).inst != Inst::Multiply {
        return Ok(false);
    }
    if masked16(ir, s(ir, mul, 0)?)? != Some(x) {
        return Ok(false);
    }
    if shifted16(ir, s(ir, mul, 1)?, Inst::ShiftRightU32)? != Some(y) {
        return Ok(false);
    }
    if shifted16(ir, s(ir, op, 1)?, Inst::ShiftLeft)? != Some(y) {
        return Ok(false);
    }
    Ok(true)
}

fn high_times_low(ir: &Ir, op: NodeId, x: Opd, y: Opd) -> R<bool> {
    if ir.op(op).inst != Inst::ShiftLeft || !is_const(ir, s(ir, op, 1)?, 16) {
        return Ok(false);
    }
    let Some(mul) = ir.asg_operation(s(ir, op, 0)?) else {
        return Ok(false);
    };
    if ir.op(mul).inst != Inst::Multiply {
        return Ok(false);
    }
    if shifted16(ir, s(ir, mul, 0)?, Inst::ShiftRightU32)? != Some(x) {
        return Ok(false);
    }
    let src2 = masked16(ir, s(ir, mul, 1)?)?.ok_or("null reference in XmadOptimizer")?;
    Ok(ir.ty(src2) == ir.ty(y) && ir.value(src2) == ir.value(y))
}

fn high_times_high(ir: &Ir, op: NodeId, x: Opd, ltl: Opd) -> R<bool> {
    if ir.op(op).inst != Inst::ShiftLeft || !is_const(ir, s(ir, op, 1)?, 16) {
        return Ok(false);
    }
    let Some(mul) = ir.asg_operation(s(ir, op, 0)?) else {
        return Ok(false);
    };
    if ir.op(mul).inst != Inst::Multiply {
        return Ok(false);
    }
    if shifted16(ir, s(ir, mul, 0)?, Inst::ShiftRightU32)? != Some(x) {
        return Ok(false);
    }
    let sh = shifted16(ir, s(ir, mul, 1)?, Inst::ShiftRightU32)?;
    Ok(copy_source(ir, sh)? == ltl)
}

fn match32x32(ir: &Ir, op: NodeId) -> R<Option<Match>> {
    if ir.op(op).inst != Inst::Add {
        return Ok(None);
    }
    let (src1, src2) = (s(ir, op, 0)?, s(ir, op, 1)?);
    let Some(add_op) = ir.asg_operation(src2) else {
        return Ok(None);
    };
    if ir.op(add_op).inst != Inst::Add {
        return Ok(None);
    }
    let ltl_result = copy_source(ir, Some(s(ir, add_op, 0)?))?;
    let Some(ltl_op) = ir.asg_operation(ltl_result) else {
        return Ok(None);
    };
    let Some(m) = low_times_low(ir, ltl_op)? else {
        return Ok(None);
    };
    let sh = shifted16(ir, s(ir, add_op, 1)?, Inst::ShiftLeft)?;
    let lth_result = copy_source(ir, sh)?;
    let Some(lth_op) = ir.asg_operation(lth_result) else {
        return Ok(None);
    };
    if !low_times_high(ir, lth_op, m.x, m.y)? {
        return Ok(None);
    }
    let Some(hth_op) = ir.asg_operation(src1) else {
        return Ok(None);
    };
    if !high_times_high(ir, hth_op, m.x, lth_result)? {
        return Ok(None);
    }
    Ok(Some(m))
}

fn match32x16(ir: &Ir, op: NodeId) -> R<Option<Match>> {
    if ir.op(op).inst != Inst::Add {
        return Ok(None);
    }
    let (src1, src2) = (s(ir, op, 0)?, s(ir, op, 1)?);
    let ltl_result = copy_source(ir, Some(src2))?;
    let Some(ltl_op) = ir.asg_operation(ltl_result) else {
        return Ok(None);
    };
    let Some(m) = low_times_low(ir, ltl_op)? else {
        return Ok(None);
    };
    let Some(htl_op) = ir.asg_operation(src1) else {
        return Ok(None);
    };
    if !high_times_low(ir, htl_op, m.x, m.y)? {
        return Ok(None);
    }
    let ok = ir.ty(m.y) == OperandType::Constant && (ir.value(m.y) as u16 as i32) == ir.value(m.y);
    Ok(if ok { Some(m) } else { None })
}

pub fn run_pass(ir: &mut Ir, block: &mut Block) -> R<()> {
    let mut i = 0;
    while i < block.ops.len() {
        let node = block.ops[i];
        if !ir.is_op(node) {
            i += 1;
            continue;
        }
        let m = match match32x32(ir, node)? {
            Some(m) => Some(m),
            None => match32x16(ir, node)?,
        };
        let Some(m) = m else {
            i += 1;
            continue;
        };
        let dest = ir.op(node).dests.first().copied().flatten();
        let mut added = 0;
        if let Some(addend) = m.addend {
            let temp = ir.local();
            let mul = ir.new_op(Inst::Multiply, StorageKind::None, Some(temp), &[m.x, m.y]);
            let add = ir.new_op(Inst::Add, StorageKind::None, dest, &[temp, addend]);
            block.ops.insert(i + 1, mul);
            block.ops.insert(i + 2, add);
            added = 2;
        } else {
            let mul = ir.new_op(Inst::Multiply, StorageKind::None, dest, &[m.x, m.y]);
            block.ops.insert(i + 1, mul);
            added += 1;
        }
        super::remove_node(ir, block, i);
        i += added;
    }
    Ok(())
}
