//! `BranchElimination`.
use crate::ir::{Block, Inst, Ir, Node};

/// Remove the branch edge of `blocks[bi]` when the next non-empty block only jumps to the same label.
pub fn run_pass(ir: &Ir, blocks: &mut [Block], bi: usize) -> Result<bool, String> {
    let Some(old_branch) = blocks[bi].branch else {
        return Ok(false);
    };
    let Some(&last) = blocks[bi].ops.last() else {
        return Err("block with a branch and no operations".into());
    };
    let Node::Op(current) = &ir.nodes[last.0 as usize] else {
        return Err("last node is not an operation".into());
    };
    // Next(block)
    let mut next = blocks[bi].next;
    while let Some(nb) = next {
        if !blocks[nb].ops.is_empty() {
            break;
        }
        if blocks[nb].branch.is_some() {
            return Err("Found a bogus empty block that \"ends with a branch\".".into());
        }
        next = blocks[nb].next;
    }
    let Some(nb) = next else {
        return Ok(false);
    };
    let Node::Op(first) = &ir.nodes[blocks[nb].ops[0].0 as usize] else {
        return Ok(false);
    };
    if first.inst != Inst::Branch {
        return Ok(false);
    }
    if current.dests.first() != first.dests.first() {
        return Ok(false);
    }
    // block.Branch = null
    if let Some(p) = blocks[old_branch].preds.iter().position(|x| *x == bi) {
        blocks[old_branch].preds.remove(p);
    }
    blocks[bi].branch = None;
    Ok(true)
}
