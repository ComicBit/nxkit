//! `Optimizations/Utils`.
use crate::ir::{Block, Inst, IoVariable, Ir, Node, NodeId, Opd, OperandType, StorageKind};

type R<T> = Result<T, String>;

/// `Utils.IsInputLoad(node)`.
pub fn is_input_load(ir: &Ir, n: Option<NodeId>) -> bool {
    let Some(n) = n else { return false };
    matches!(&ir.nodes[n.0 as usize], Node::Op(o) if o.inst == Inst::Load && o.storage_kind == StorageKind::Input)
}

/// `Utils.IsInputLoad(node, ioVariable, elemIndex)`.
pub fn is_input_load_of(ir: &Ir, n: Option<NodeId>, var: IoVariable, elem: i32) -> bool {
    let Some(n) = n else { return false };
    let Node::Op(o) = &ir.nodes[n.0 as usize] else {
        return false;
    };
    if o.inst != Inst::Load || o.storage_kind != StorageKind::Input || o.sources.len() != 2 {
        return false;
    }
    let (Some(a), Some(b)) = (o.sources[0], o.sources[1]) else {
        return false;
    };
    ir.ty(a) == OperandType::Constant
        && ir.value(a) == var.0 as i32
        && ir.ty(b) == OperandType::Constant
        && ir.value(b) == elem
}

fn is_same_operand(ir: &Ir, x: Opd, y: Opd) -> bool {
    if ir.ty(x) != ir.ty(y) || ir.value(x) != ir.value(y) {
        return false;
    }
    x == y || ir.ty(x) == OperandType::Constant || ir.ty(x) == OperandType::ConstantBuffer
}

fn all_sources_equal(ir: &Ir, a: NodeId, b: NodeId) -> bool {
    let (sa, sb) = (ir.node_sources(a), ir.node_sources(b));
    if sa.len() != sb.len() {
        return false;
    }
    sa.iter().zip(sb.iter()).all(|(x, y)| match (x, y) {
        (Some(x), Some(y)) => is_same_operand(ir, *x, *y),
        _ => false,
    })
}

/// `Utils.AreAllSourcesTheSameOperand`.
pub fn are_all_sources_the_same_operand(ir: &Ir, n: NodeId) -> R<bool> {
    let srcs = ir.node_sources(n);
    let first = srcs.first().copied().flatten().ok_or("no sources")?;
    for s in srcs.iter().skip(1) {
        let s = s.ok_or("null source")?;
        if !is_same_operand(ir, first, s) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn find_branch_source(ir: &Ir, blocks: &[Block], block: usize) -> Option<NodeId> {
    for &sb in &blocks[block].preds {
        if let Some(&last) = blocks[sb].ops.last()
            && let Node::Op(o) = &ir.nodes[last.0 as usize]
            && matches!(o.inst, Inst::BranchIfFalse | Inst::BranchIfTrue)
            && blocks[sb].next == Some(block)
        {
            return Some(last);
        }
    }
    None
}

fn is_same_condition(ir: &Ir, cur: Opd, query: Opd) -> bool {
    if cur == query {
        return true;
    }
    match (ir.asg_operation(cur), ir.asg_operation(query)) {
        (Some(a), Some(b)) => ir.op(a).inst == ir.op(b).inst && all_sources_equal(ir, a, b),
        _ => false,
    }
}

fn block_conditions_match(ir: &Ir, blocks: &[Block], cur: usize, query: usize) -> bool {
    let cb = find_branch_source(ir, blocks, cur);
    let qb = find_branch_source(ir, blocks, query);
    let (Some(cb), Some(qb)) = (cb, qb) else {
        return false;
    };
    if ir.op(cb).inst != ir.op(qb).inst {
        return false;
    }
    match (
        ir.op(cb).sources.first().copied().flatten(),
        ir.op(qb).sources.first().copied().flatten(),
    ) {
        (Some(a), Some(b)) => is_same_condition(ir, a, b),
        _ => false,
    }
}

/// `Utils.FindLastOperation`.
pub fn find_last_operation(
    ir: &Ir,
    blocks: &[Block],
    source: Opd,
    block: usize,
    recurse: bool,
) -> R<Opd> {
    let Some(asg) = ir.data(source).asg_op else {
        return Ok(source);
    };
    let Node::Phi(phi) = &ir.nodes[asg.0 as usize] else {
        return Ok(source);
    };
    let sources = phi.sources.clone();
    let mut undef_count = 0;
    for i in (0..sources.len()).rev() {
        let (phi_block, phi_source) = (sources[i].0 as usize, sources[i].1);
        if block_conditions_match(ir, blocks, block, phi_block) {
            return Ok(phi_source);
        } else if recurse && ir.data(phi_source).asg_op.is_some_and(|a| ir.is_phi(a)) {
            let m = find_last_operation(ir, blocks, phi_source, block, false)?;
            if m != phi_source {
                return Ok(m);
            }
        } else if ir.ty(phi_source) == OperandType::Undefined {
            undef_count += 1;
        }
    }
    if undef_count + 1 == sources.len() {
        for i in (0..sources.len()).rev() {
            if ir.ty(sources[i].1) != OperandType::Undefined {
                return Ok(sources[i].1);
            }
        }
    }
    Ok(source)
}

/// `Utils.DeleteNode`: take the node out of the block, drop its sources and its destination.
pub fn delete_node(ir: &mut Ir, block: &mut Block, pos: usize) {
    let node = block.ops.remove(pos);
    let n = ir.op(node).sources.len();
    for i in 0..n {
        ir.set_source(node, i, None);
    }
    ir.clear_dests(node);
}
