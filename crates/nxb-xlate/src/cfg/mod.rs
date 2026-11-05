//! `ControlFlowGraph`, `Dominance`, `RegisterUsage` (calls only) and `Ssa`.
#![allow(missing_docs)]

pub mod dominance;
pub mod register_usage;
pub mod ssa;

use crate::ir::{Block, Inst, Ir, Node, NodeId, Opd};
use std::collections::HashMap;

/// `ControlFlowGraph` plus the dominator data that C# keeps on `BasicBlock`.
#[derive(Debug, Default)]
pub struct Cfg {
    pub blocks: Vec<Block>,
    pub post_order: Vec<usize>,
    pub post_order_map: Vec<usize>,
    pub idom: Vec<Option<usize>>,
    pub frontiers: Vec<crate::ir::DotnetSet<usize>>,
}

struct Tmp {
    ops: Vec<NodeId>,
    next: Option<usize>,
    branch: Option<usize>,
    preds: Vec<usize>,
}

fn set_succ(blocks: &mut [Tmp], this: usize, branch: bool, new: Option<usize>) {
    let old = if branch {
        blocks[this].branch
    } else {
        blocks[this].next
    };
    if let Some(o) = old
        && let Some(p) = blocks[o].preds.iter().position(|x| *x == this)
    {
        blocks[o].preds.remove(p);
    }
    if let Some(n) = new {
        blocks[n].preds.push(this);
    }
    if branch {
        blocks[this].branch = new;
    } else {
        blocks[this].next = new;
    }
}

fn inst_of(ir: &Ir, n: NodeId) -> Inst {
    match &ir.nodes[n.0 as usize] {
        Node::Op(o) => o.inst,
        Node::Comment(_) => Inst::Comment,
        Node::Phi(_) => Inst(u32::MAX),
    }
}

fn ends_unconditional(ir: &Ir, n: Option<NodeId>) -> bool {
    n.is_some_and(|n| matches!(inst_of(ir, n), Inst::Branch | Inst::Discard | Inst::Return))
}

impl Cfg {
    /// `ControlFlowGraph(blocks)`: post order of the reachable blocks.
    pub fn from_blocks(blocks: Vec<Block>) -> Cfg {
        let n = blocks.len();
        let mut visited = vec![false; n];
        let mut stack = vec![];
        let mut post_order = Vec::with_capacity(n);
        let mut post_order_map = vec![0; n];
        if n > 0 {
            visited[0] = true;
            stack.push(0usize);
        }
        while let Some(b) = stack.pop() {
            let nx = blocks[b]
                .next
                .filter(|x| !std::mem::replace(&mut visited[*x], true));
            if let Some(x) = nx {
                stack.push(b);
                stack.push(x);
                continue;
            }
            let br = blocks[b]
                .branch
                .filter(|x| !std::mem::replace(&mut visited[*x], true));
            if let Some(x) = br {
                stack.push(b);
                stack.push(x);
            } else {
                post_order_map[b] = post_order.len();
                post_order.push(b);
            }
        }
        Cfg {
            blocks,
            post_order,
            post_order_map,
            idom: vec![None; n],
            frontiers: vec![Default::default(); n],
        }
    }

    /// `ControlFlowGraph.Create` from the operations of a function.
    pub fn create(ir: &Ir, operations: &[NodeId]) -> Result<Cfg, String> {
        let mut blocks: Vec<Tmp> = vec![];
        let mut order: Vec<usize> = vec![];
        let mut labels: HashMap<Opd, usize> = HashMap::new();
        let mut current: Option<usize> = None;
        let mut needs_new = true;

        // NextBlock
        fn next_block(ir: &Ir, blocks: &mut [Tmp], current: &mut Option<usize>, nb: usize) {
            if let Some(c) = *current {
                let last = blocks[c].ops.last().copied();
                if !ends_unconditional(ir, last) {
                    set_succ(blocks, c, false, Some(nb));
                }
            }
            *current = Some(nb);
        }
        fn new_next(
            ir: &Ir,
            blocks: &mut Vec<Tmp>,
            order: &mut Vec<usize>,
            current: &mut Option<usize>,
        ) {
            blocks.push(Tmp {
                ops: vec![],
                next: None,
                branch: None,
                preds: vec![],
            });
            let id = blocks.len() - 1;
            order.push(id);
            next_block(ir, blocks, current, id);
        }

        for &n in operations {
            let inst = inst_of(ir, n);
            let dest = match &ir.nodes[n.0 as usize] {
                Node::Op(o) => o.dests.first().copied().flatten(),
                _ => None,
            };
            if inst == Inst::MarkLabel {
                let label = dest.ok_or("label without dest")?;
                if let Some(&nb) = labels.get(&label) {
                    order.push(nb);
                    next_block(ir, &mut blocks, &mut current, nb);
                } else {
                    new_next(ir, &mut blocks, &mut order, &mut current);
                    labels.insert(label, current.ok_or("no block")?);
                }
            } else {
                if needs_new {
                    new_next(ir, &mut blocks, &mut order, &mut current);
                }
                blocks[current.ok_or("no block")?].ops.push(n);
            }
            needs_new = matches!(
                inst,
                Inst::Branch | Inst::BranchIfTrue | Inst::BranchIfFalse
            );
            if needs_new {
                let label = dest.ok_or("branch without label")?;
                let bb = match labels.get(&label) {
                    Some(b) => *b,
                    None => {
                        blocks.push(Tmp {
                            ops: vec![],
                            next: None,
                            branch: None,
                            preds: vec![],
                        });
                        let id = blocks.len() - 1;
                        labels.insert(label, id);
                        id
                    }
                };
                set_succ(&mut blocks, current.ok_or("no block")?, true, Some(bb));
            }
        }

        // Remove unreachable blocks.
        loop {
            let mut has_unreachable = false;
            let mut i = 1;
            while i < order.len() {
                let b = order[i];
                if blocks[b].preds.is_empty() {
                    set_succ(&mut blocks, b, false, None);
                    set_succ(&mut blocks, b, true, None);
                    order.remove(i);
                    has_unreachable = true;
                } else {
                    i += 1;
                }
            }
            if !has_unreachable {
                break;
            }
        }

        let mut index_of = vec![usize::MAX; blocks.len()];
        for (i, b) in order.iter().enumerate() {
            index_of[*b] = i;
        }
        let map = |x: usize| -> Result<usize, String> {
            match index_of[x] {
                usize::MAX => Err("reference to a block that is not in the list".to_string()),
                v => Ok(v),
            }
        };
        let mut out = Vec::with_capacity(order.len());
        for (i, &b) in order.iter().enumerate() {
            let t = &blocks[b];
            out.push(Block {
                index: i,
                ops: t.ops.clone(),
                next: t.next.map(map).transpose()?,
                branch: t.branch.map(map).transpose()?,
                preds: t.preds.iter().map(|p| map(*p)).collect::<Result<_, _>>()?,
            });
        }
        Ok(Cfg::from_blocks(out))
    }
}

#[cfg(test)]
mod tests;
