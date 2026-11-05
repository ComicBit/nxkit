//! `GotoElimination`: a modified version of the algorithm of "Taming Control Flow: A Structured Approach to Eliminating
//! Goto Statements".

use super::GotoStatement;
use super::ast::*;
use crate::ir::Inst;
use std::collections::HashSet;

type R<T> = Result<T, String>;

fn parent_block(ast: &Ast, node: A) -> R<A> {
    if ast.is_block(node) {
        return ast
            .parent(node)
            .ok_or_else(|| "block without parent".to_string());
    }
    let mut n = node;
    while !ast.is_block(n) {
        n = ast.parent(n).ok_or("node without enclosing block")?;
    }
    Ok(n)
}

fn level(ast: &Ast, node: A) -> i32 {
    let mut l = 0;
    let mut n = Some(node);
    while let Some(x) = n {
        l += 1;
        n = ast.parent(x);
    }
    l
}

fn parent_of(ast: &Ast, b: A) -> R<A> {
    ast.parent(b)
        .ok_or_else(|| "block without parent".to_string())
}

fn directly_related(ast: &Ast, l_block: A, r_block: A, l_level: i32, r_level: i32) -> bool {
    if l_level == r_level {
        return false;
    }
    let (mut block, mut block_lvl, other, other_lvl) = if l_level > r_level {
        (Some(l_block), l_level, r_block, r_level)
    } else {
        (Some(r_block), r_level, l_block, l_level)
    };
    while block_lvl >= other_lvl {
        if block == Some(other) {
            return true;
        }
        block = block.and_then(|b| ast.parent(b));
        block_lvl -= 1;
    }
    false
}

fn indirectly_related(ast: &Ast, l: A, r: A, ll: i32, rl: i32) -> bool {
    !(l == r || directly_related(ast, l, r, ll, rl))
}

fn is_same_cond(ast: &Ast, l: Option<A>, r: Option<A>) -> bool {
    let (mut l, mut r) = (l, r);
    if let Some(lo) = l.and_then(|x| ast.op(x))
        && lo.inst == Inst::LogicalNot
    {
        let Some(ro) = r.and_then(|x| ast.op(x)) else {
            return false;
        };
        if ro.inst != lo.inst {
            return false;
        }
        l = lo.sources.first().copied();
        r = ro.sources.first().copied();
    }
    l == r
}

fn contains_cond_comb(ast: &Ast, node: Option<A>, inst: Inst, new_cond: A) -> bool {
    let mut node = node;
    while let Some(op) = node.and_then(|n| ast.op(n)) {
        if op.sources.len() != 2 {
            break;
        }
        if op.inst == inst && is_same_cond(ast, Some(op.sources[1]), Some(new_cond)) {
            return true;
        }
        node = Some(op.sources[0]);
    }
    false
}

fn inverse_cond(ast: &mut Ast, cond: A) -> A {
    ast.new_simple_op(Inst::LogicalNot, vec![cond])
}

fn block_matches(ast: &Ast, node: A, ty: BlockType, cond: Option<A>) -> bool {
    match ast.block(node) {
        Some(b) => b.ty == ty && is_same_cond(ast, b.cond, cond),
        None => false,
    }
}

/// `Enclose`: wraps the nodes from `first` up to (not including) `last` in a new block.`None` when empty.
fn enclose(
    ast: &mut Ast,
    block: A,
    ty: BlockType,
    cond: Option<A>,
    first: Option<A>,
    last: Option<A>,
) -> R<Option<A>> {
    if first == last {
        return Ok(None);
    }
    let mut first = first.ok_or("enclose without first node")?;
    let mut cond = cond;
    if ty == BlockType::If {
        let c = cond.ok_or("if without condition")?;
        cond = Some(inverse_cond(ast, c));
    }
    let has_single_node = ast.next(first) == last;
    if has_single_node && block_matches(ast, first, ty, cond) {
        return Ok(if ast.is_block(first) {
            Some(first)
        } else {
            None
        });
    }
    let new_block = ast.new_block(ty, cond);
    ast.add_before(block, first, new_block)?;
    while Some(first) != last {
        let next = ast.next(first);
        ast.remove(block, first);
        ast.add(new_block, first)?;
        match next {
            Some(n) => first = n,
            None => break,
        }
    }
    Ok(Some(new_block))
}

fn condition_of(ast: &Ast, stmt: &GotoStatement) -> R<A> {
    match ast.kind(stmt.label) {
        Kind::Assign { dest, .. } => Ok(*dest),
        _ => Err("label is not an assignment".into()),
    }
}

fn is_unconditional(ast: &Ast, stmt: &GotoStatement) -> bool {
    ast.op(stmt.goto).is_some_and(|o| o.inst == Inst::Branch)
}

fn enclose_single_inst(ast: &mut Ast, stmt: &GotoStatement, inst: Inst) -> R<()> {
    let block = parent_block(ast, stmt.goto)?;
    let cond = condition_of(ast, stmt)?;
    let new_block = ast.new_block(BlockType::If, Some(cond));
    ast.add_after(block, stmt.goto, new_block)?;
    let op = ast.new_simple_op(inst, vec![]);
    ast.add_first(new_block, op)
}

fn enclose_do_while(ast: &mut Ast, stmt: &GotoStatement, block: A, first: A) -> R<Option<A>> {
    let cond = condition_of(ast, stmt)?;
    if ast.block_type(block) == Some(BlockType::DoWhile) && Some(first) == ast.first(block) {
        if ast.next(stmt.goto).is_some() || ast.cond(block) != Some(cond) {
            enclose_single_inst(ast, stmt, Inst::LoopContinue)?;
        }
        if !contains_cond_comb(ast, ast.cond(block), Inst::LogicalOr, cond) {
            or_condition(ast, block, cond);
        }
        return Ok(Some(block));
    }
    enclose(
        ast,
        block,
        BlockType::DoWhile,
        Some(cond),
        Some(first),
        Some(stmt.goto),
    )
}

fn or_condition(ast: &mut Ast, block: A, cond: A) {
    let c = ast.cond(block);
    let new = ast.new_simple_op(Inst::LogicalOr, c.into_iter().chain([cond]).collect());
    ast.set_condition(block, Some(new));
}

fn and_condition(ast: &mut Ast, block: A, cond: A) {
    let c = ast.cond(block);
    let new = ast.new_simple_op(Inst::LogicalAnd, c.into_iter().chain([cond]).collect());
    ast.set_condition(block, Some(new));
}

fn backwards_path(ast: &Ast, top: A, bottom: A) -> R<Vec<A>> {
    let mut block = bottom;
    let mut path = Vec::new();
    while block != top {
        path.push(block);
        block = parent_of(ast, block)?;
    }
    Ok(path)
}

fn lift(ast: &mut Ast, stmt: &mut GotoStatement) -> R<()> {
    let block = parent_block(ast, stmt.goto)?;
    let label_block = parent_block(ast, stmt.label)?;
    let path = backwards_path(ast, block, label_block)?;
    let mut loop_first_stmt = *path.last().ok_or("empty path")?;
    if ast.block_type(loop_first_stmt) == Some(BlockType::Else) {
        match ast.prev(loop_first_stmt) {
            Some(p) if ast.block_type(p) == Some(BlockType::If) => loop_first_stmt = p,
            _ => return Err("Found an else without a matching if.".into()),
        }
    }
    let new_block =
        enclose_do_while(ast, stmt, block, loop_first_stmt)?.ok_or("lift without block")?;
    ast.remove(block, stmt.goto);
    ast.add_first(new_block, stmt.goto)?;
    stmt.is_loop = false;
    Ok(())
}

fn is_inside(ast: &Ast, node: A, block: A) -> bool {
    let mut n = Some(node);
    while let Some(x) = n {
        if x == block {
            return true;
        }
        n = ast.parent(x);
    }
    false
}

fn has_goto_into(
    ast: &Ast,
    gotos: &[GotoStatement],
    index: usize,
    source: A,
    block: A,
    first: Option<A>,
    last: Option<A>,
) -> bool {
    let mut range: HashSet<A> = HashSet::new();
    let mut node = first;
    while node != last && node.is_some() {
        if let Some(n) = node {
            range.insert(n);
            node = ast.next(n);
        }
    }
    let mut other = index as i64;
    while other == index as i64
        || (other >= 0 && is_inside(ast, gotos[other as usize].goto, source))
    {
        let mut label = Some(gotos[other as usize].label);
        while let Some(l) = label {
            if ast.parent(l) == Some(block) {
                break;
            }
            label = ast.parent(l);
        }
        if let Some(l) = label
            && range.contains(&l)
        {
            return true;
        }
        other -= 1;
    }
    false
}

fn move_outward(
    ast: &mut Ast,
    gotos: &[GotoStatement],
    index: usize,
    stmt: &GotoStatement,
    g_level: i32,
    l_level: i32,
) -> R<()> {
    let mut g_level = g_level;
    let origin = parent_block(ast, stmt.goto)?;
    let mut block = origin;
    let mut loop_block = origin;
    let mut loop_level = g_level;
    let cond = condition_of(ast, stmt)?;
    while loop_level > l_level {
        let child = loop_block;
        loop_block = parent_of(ast, loop_block)?;
        loop_level -= 1;
        if ast.block_type(child) == Some(BlockType::DoWhile) {
            enclose_single_inst(ast, stmt, Inst::LoopBreak)?;
            ast.remove(block, stmt.goto);
            ast.add_after(loop_block, child, stmt.goto)?;
            block = loop_block;
            g_level = loop_level;
        }
    }
    let try_insert_else =
        is_unconditional(ast, stmt) && ast.block_type(origin) == Some(BlockType::If);
    while g_level > l_level {
        let next = ast.next(stmt.goto);
        enclose(ast, block, BlockType::If, Some(cond), next, None)?;
        ast.remove(block, stmt.goto);
        let mut child = block;
        if let Some(n) = ast.next(child)
            && ast.block_type(n) == Some(BlockType::Else)
        {
            child = n;
        }
        block = parent_of(ast, block)?;
        ast.add_after(block, child, stmt.goto)?;
        g_level -= 1;
        if try_insert_else && child == origin {
            let l_block = parent_block(ast, stmt.label)?;
            let last = if block == l_block && !stmt.is_loop {
                Some(stmt.label)
            } else {
                None
            };
            let next = ast.next(stmt.goto);
            let new_block = if has_goto_into(ast, gotos, index, child, block, next, last) {
                None
            } else {
                enclose(ast, block, BlockType::Else, None, next, last)?
            };
            if let Some(nb) = new_block {
                ast.remove(block, stmt.goto);
                ast.add_after(block, nb, stmt.goto)?;
            }
        }
    }
    Ok(())
}

fn move_inward(ast: &mut Ast, stmt: &GotoStatement) -> R<()> {
    let mut block = parent_block(ast, stmt.goto)?;
    let label_block = parent_block(ast, stmt.label)?;
    let path = backwards_path(ast, block, label_block)?;
    let cond = condition_of(ast, stmt)?;
    for &child in path.iter().rev() {
        let mut last = Some(child);
        match ast.block_type(child) {
            Some(BlockType::If) => {
                if !contains_cond_comb(ast, ast.cond(child), Inst::LogicalOr, cond) {
                    or_condition(ast, child, cond);
                }
            }
            Some(BlockType::Else) => {
                let if_block = match ast.prev(child) {
                    Some(p) if ast.block_type(p) == Some(BlockType::If) => p,
                    _ => return Err("Found an else without a matching if.".into()),
                };
                let inv = inverse_cond(ast, cond);
                if !contains_cond_comb(ast, ast.cond(if_block), Inst::LogicalAnd, inv) {
                    and_condition(ast, if_block, inv);
                }
                last = Some(if_block);
            }
            _ => {}
        }
        let next = ast.next(stmt.goto);
        enclose(ast, block, BlockType::If, Some(cond), next, last)?;
        ast.remove(block, stmt.goto);
        ast.add_first(child, stmt.goto)?;
        block = child;
    }
    Ok(())
}

/// `GotoElimination.Eliminate`.
pub fn eliminate(ast: &mut Ast, gotos_in: &[GotoStatement]) -> R<()> {
    let mut gotos: Vec<GotoStatement> = gotos_in.to_vec();
    for index in (0..gotos.len()).rev() {
        let mut stmt = gotos[index];
        let mut g_block = parent_block(ast, stmt.goto)?;
        let mut l_block = parent_block(ast, stmt.label)?;
        let mut g_level = level(ast, g_block);
        let mut l_level = level(ast, l_block);
        if indirectly_related(ast, g_block, l_block, g_level, l_level) {
            let mut dr_block = g_block;
            let mut dr_level = g_level;
            loop {
                dr_block = parent_of(ast, dr_block)?;
                dr_level -= 1;
                if directly_related(ast, dr_block, l_block, dr_level, l_level) {
                    break;
                }
            }
            move_outward(ast, &gotos, index, &stmt, g_level, dr_level)?;
            g_block = dr_block;
            g_level = dr_level;
            if let Some(p) = ast.prev(stmt.goto)
                && ast.block_type(p) == Some(BlockType::Else)
            {
                l_block = parent_block(ast, stmt.label)?;
                l_level = level(ast, l_block);
                if !indirectly_related(ast, p, l_block, g_level + 1, l_level) {
                    stmt.is_loop = true;
                    gotos[index].is_loop = true;
                }
            }
        }
        if directly_related(ast, g_block, l_block, g_level, l_level) {
            if g_level > l_level {
                move_outward(ast, &gotos, index, &stmt, g_level, l_level)?;
            } else {
                if stmt.is_loop {
                    lift(ast, &mut stmt)?;
                    gotos[index].is_loop = stmt.is_loop;
                }
                move_inward(ast, &stmt)?;
            }
        }
        g_block = parent_block(ast, stmt.goto)?;
        if stmt.is_loop {
            enclose_do_while(ast, &stmt, g_block, stmt.label)?;
        } else {
            let cond = condition_of(ast, &stmt)?;
            let next = ast.next(stmt.goto);
            enclose(
                ast,
                g_block,
                BlockType::If,
                Some(cond),
                next,
                Some(stmt.label),
            )?;
        }
        ast.remove(g_block, stmt.goto);
    }
    Ok(())
}
