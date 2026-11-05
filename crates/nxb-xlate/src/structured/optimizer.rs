//! `AstOptimizer`: with debug mode on (the translator always runs with it) only the empty block removal runs.

use super::ast::*;
use super::context::Context;
use crate::ir::Inst;
use std::collections::VecDeque;

/// `AstOptimizer.Optimize`.
pub fn optimize(ctx: &mut Context) -> Result<(), String> {
    if !ctx.debug_mode {
        return Err("unported: AST expression propagation (debug mode off)".into());
    }
    let main = ctx.cur.main_block;
    remove_empty_blocks(&mut ctx.ast, main)
}

fn remove_empty_blocks(ast: &mut Ast, main: A) -> Result<(), String> {
    let mut pending: VecDeque<A> = VecDeque::new();
    pending.push_back(main);
    while let Some(block) = pending.pop_front() {
        for node in ast.children(block) {
            if ast.is_block(node) {
                pending.push_back(node);
            }
        }
        let Some(parent) = ast.parent(block) else {
            continue;
        };
        let next_block = ast.next(block).filter(|n| ast.is_block(*n));
        let has_else = next_block.is_some_and(|n| ast.block_type(n) == Some(BlockType::Else));
        let is_if = ast.block_type(block) == Some(BlockType::If);
        if ast.count(block) == 0 {
            if is_if {
                if has_else && let Some(nb) = next_block {
                    let c = ast.cond(block).ok_or("if without condition")?;
                    let inv = ast.new_simple_op(Inst::LogicalNot, vec![c]);
                    ast.set_condition(nb, Some(inv));
                    ast.set_type(nb, BlockType::If);
                }
                ast.remove(parent, block);
            } else if ast.block_type(block) == Some(BlockType::Else) {
                ast.remove(parent, block);
            }
        } else if is_if
            && ast.block_type(parent) == Some(BlockType::Else)
            && ast.count(parent) == if has_else { 2 } else { 1 }
        {
            let pop = ast.parent(parent).ok_or("else without parent")?;
            ast.remove(parent, block);
            ast.add_after(pop, parent, block)?;
            if has_else && let Some(nb) = next_block {
                ast.remove(parent, nb);
                ast.add_after(pop, block, nb)?;
            }
            ast.remove(pop, parent);
            ast.set_type(block, BlockType::ElseIf);
        }
    }
    Ok(())
}
