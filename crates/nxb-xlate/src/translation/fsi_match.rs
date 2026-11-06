//! `FunctionMatch.RunPass`: finds the fragment shader interlock functions of a Maxwell program. The functions that
//! compute the interlock address disappear and the begin and end functions become the `FSIBegin` and `FSIEnd`
//! instructions at their call sites.

use super::fsi_patterns::{PUse, Patterns};
use crate::maxwell::decoder::{Block, DecodedFunction, DecodedProgram};
use crate::maxwell::names::InstName;
use crate::maxwell::table::{InstOp, props};
use std::collections::HashSet;

const PRED_COUNT: usize = 8;
const GPR_COUNT: usize = 256;
const REGISTER_ZERO: u8 = 255;
const PREDICATE_TRUE: u8 = 7;

const RB2: u16 = 16;
const PD_MASK: u16 = 7 << 7;
const PS: u16 = 1 << 11;

/// `FunctionType` of a function after the pass (`User` is the plain function).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FunctionType {
    User,
    Unused,
    FsiBegin,
    FsiEnd,
}

#[derive(Clone, Copy)]
struct TUse {
    index: i32,
    inverted: bool,
    node: Option<usize>,
}

struct TNode {
    op: InstOp,
    uses: Vec<TUse>,
    label: bool,
    order: u8,
}

fn is_order_dependant(n: InstName) -> bool {
    matches!(
        n,
        InstName::Atom
            | InstName::AtomCas
            | InstName::Atoms
            | InstName::AtomsCas
            | InstName::Ld
            | InstName::Ldg
            | InstName::Ldl
            | InstName::Lds
            | InstName::Suatom
            | InstName::SuatomB
            | InstName::SuatomB2
            | InstName::SuatomCas
            | InstName::SuatomCasB
            | InstName::Suld
            | InstName::SuldB
            | InstName::SuldD
            | InstName::SuldDB
    )
}

/// `BuildTree`: the operations of a function as trees of their uses. The roots are the nodes nobody uses.
pub(super) struct Tree {
    nodes: Vec<TNode>,
    roots: Vec<usize>,
}

pub(super) fn build_tree(blocks: &[Block]) -> Tree {
    let mut nodes: Vec<TNode> = Vec::new();
    let mut roots: Vec<usize> = Vec::new();
    let mut labels: Vec<(u64, usize)> = Vec::new();
    let mut pred_defs = [TUse {
        index: 0,
        inverted: false,
        node: None,
    }; PRED_COUNT];
    let mut gpr_defs = [TUse {
        index: 0,
        inverted: false,
        node: None,
    }; GPR_COUNT];
    let mut order = 0u8;
    for block in blocks {
        if block.preds.len() > 1 {
            nodes.push(TNode {
                op: InstOp::none(),
                uses: Vec::new(),
                label: true,
                order,
            });
            order += 1;
            roots.push(nodes.len() - 1);
            labels.push((block.address, nodes.len() - 1));
        }
        for &op in &block.ops {
            let this = nodes.len();
            let mut uses = Vec::new();
            let use_pred = |roots: &mut Vec<usize>, index: u8, inv: bool| -> TUse {
                if index == PREDICATE_TRUE {
                    return TUse {
                        index: -1,
                        inverted: false,
                        node: None,
                    };
                }
                let mut u = pred_defs[index as usize];
                if let Some(n) = u.node {
                    if let Some(p) = roots.iter().position(|&r| r == n) {
                        roots.remove(p);
                    }
                } else {
                    u = TUse {
                        index: -(index as i32 + 2),
                        inverted: false,
                        node: None,
                    };
                }
                if inv {
                    u.inverted = !u.inverted;
                }
                u
            };
            if !op.has(props::NO_PRED) {
                uses.push(use_pred(
                    &mut roots,
                    ((op.raw >> 16) & 7) as u8,
                    op.raw & 0x80000 != 0,
                ));
            }
            if op.has(PS) {
                uses.push(use_pred(
                    &mut roots,
                    ((op.raw >> 39) & 7) as u8,
                    op.raw & 0x400_0000_0000 != 0,
                ));
            }
            let use_gpr = |roots: &mut Vec<usize>, index: u8| -> TUse {
                if index == REGISTER_ZERO {
                    return TUse {
                        index: -1,
                        inverted: false,
                        node: None,
                    };
                }
                let u = gpr_defs[index as usize];
                if let Some(n) = u.node {
                    if let Some(p) = roots.iter().position(|&r| r == n) {
                        roots.remove(p);
                    }
                    u
                } else {
                    TUse {
                        index: -(index as i32 + 2),
                        inverted: false,
                        node: None,
                    }
                }
            };
            if op.has(props::RA) {
                uses.push(use_gpr(&mut roots, (op.raw >> 8) as u8));
            }
            if op.props & (props::RB | RB2) != 0 {
                let rb = if op.has(RB2) {
                    op.raw as u8
                } else {
                    (op.raw >> 20) as u8
                };
                uses.push(use_gpr(&mut roots, rb));
            }
            if op.has(props::RC) {
                uses.push(use_gpr(&mut roots, (op.raw >> 39) as u8));
            }
            if op.name == InstName::Bra
                && let Some(&(_, l)) = labels.iter().find(|(a, _)| *a == op.absolute_address())
            {
                uses.push(TUse {
                    index: 0,
                    inverted: false,
                    node: Some(l),
                });
            }
            let mut def_index = 0;
            let pd_bit = match op.props & PD_MASK {
                0 => None,
                x if x == 1 << 7 => Some(3),
                x if x == 2 << 7 => Some(48),
                x if x == 3 << 7 => Some(30),
                x if x == 4 << 7 => Some(51),
                _ => Some(45),
            };
            if let Some(bit) = pd_bit {
                let pred = ((op.raw >> bit) & 7) as u8;
                if pred != PREDICATE_TRUE {
                    pred_defs[pred as usize] = TUse {
                        index: def_index,
                        inverted: false,
                        node: Some(this),
                    };
                }
                def_index += 1;
            }
            if op.has(props::RD) {
                let rd = op.raw as u8;
                if rd != REGISTER_ZERO {
                    gpr_defs[rd as usize] = TUse {
                        index: def_index,
                        inverted: false,
                        node: Some(this),
                    };
                }
            }
            let ord = if is_order_dependant(op.name) {
                order
            } else {
                0
            };
            nodes.push(TNode {
                op,
                uses,
                label: false,
                order: ord,
            });
            roots.push(this);
        }
    }
    Tree { nodes, roots }
}

pub(super) fn matches_tree(p: &Patterns, pat: &[usize], t: &Tree, ext: Option<&[u8; 4]>) -> bool {
    pat.len() == t.roots.len()
        && pat
            .iter()
            .zip(&t.roots)
            .all(|(&pn, &tn)| matches_node(p, pn, t, tn, ext))
}

fn matches_node(p: &Patterns, pn: usize, t: &Tree, tn: usize, ext: Option<&[u8; 4]>) -> bool {
    let (pat, node) = (&p.nodes[pn], &t.nodes[tn]);
    let name_ok = pat.name == node.op.name && (pat.matches)(node.op.raw);
    if !name_ok
        || pat.label != node.label
        || pat.order != node.order
        || pat.is_imm != node.op.has(props::IB)
    {
        return false;
    }
    if pat.label {
        return true;
    }
    if pat.uses.len() != node.uses.len() {
        return false;
    }
    pat.uses
        .iter()
        .zip(&node.uses)
        .all(|(pu, tu): (&PUse, &TUse)| {
            let index_ok = if pu.index <= -2 {
                ext.is_some_and(|e| e[(-pu.index - 2) as usize] as i32 == -tu.index - 2)
            } else {
                pu.index == tu.index
            };
            index_ok
                && pu.inverted == tu.inverted
                && pu.node.is_some() == tu.node.is_some()
                && match (pu.node, tu.node) {
                    (Some(a), Some(b)) => matches_node(p, a, t, b, ext),
                    _ => true,
                }
        })
}

fn last_op(f: &DecodedFunction) -> Option<InstOp> {
    f.blocks.first().and_then(|b| b.ops.last().copied())
}

/// `FunctionMatch.RunPass` for a Maxwell program: the type of every function (in the order of the program).
pub fn run_pass(program: &DecodedProgram) -> Vec<FunctionType> {
    let patterns = Patterns::build();
    let funcs = &program.functions;
    let mut types = vec![FunctionType::User; funcs.len()];
    let by_address = |a: u64| funcs.iter().position(|f| f.address == a);
    // callers[i]: the functions whose blocks end in a CAL to function i
    let mut callers: Vec<HashSet<usize>> = vec![HashSet::new(); funcs.len()];
    for (fi, f) in funcs.iter().enumerate() {
        for b in &f.blocks {
            let last = b.last_op();
            if last.name == InstName::Cal
                && let Some(t) = by_address(last.absolute_address())
            {
                callers[t].insert(fi);
            }
        }
    }
    let mut ext = [0u8; 4];
    let mut has_get_address = false;
    for (fi, f) in funcs.iter().enumerate().skip(1) {
        let tree = build_tree(&f.blocks);
        let rd = |i: usize| tree.nodes[tree.roots[i]].op.raw as u8;
        let mut reg4 = 0u8;
        if matches_tree(&patterns, &patterns.get_address, &tree, None) {
            ext[1] = rd(0);
            ext[2] = rd(2);
            ext[3] = rd(1);
            reg4 = rd(3);
        } else if matches_tree(&patterns, &patterns.get_address_v2, &tree, None) {
            ext[1] = rd(2);
            ext[2] = rd(1);
            ext[3] = rd(0);
            reg4 = rd(3);
        }
        let (r1, r2, r3) = (ext[1] as i32, ext[2] as i32, ext[3] as i32);
        if r1 != r2
            && r2 != r3
            && r1 != r3
            && r1 + 1 != r2
            && r1 + 1 != r3
            && r1 + 1 == reg4 as i32
            && ext[2] != REGISTER_ZERO
            && ext[3] != REGISTER_ZERO
            && reg4 != REGISTER_ZERO
        {
            has_get_address = true;
            types[fi] = FunctionType::Unused;
            break;
        }
    }
    if !has_get_address {
        return types;
    }
    for (fi, f) in funcs.iter().enumerate().skip(1) {
        if types[fi] != FunctionType::User {
            continue;
        }
        let tree = build_tree(&f.blocks);
        for (pat, ty) in [
            (&patterns.begin, FunctionType::FsiBegin),
            (&patterns.end, FunctionType::FsiEnd),
        ] {
            if matches_fsi(
                &patterns,
                pat,
                funcs,
                &mut callers,
                fi,
                &tree,
                &mut ext,
                &mut types,
            ) {
                types[fi] = ty;
                break;
            }
        }
    }
    types
}

#[allow(clippy::too_many_arguments)]
fn matches_fsi(
    p: &Patterns,
    pat: &[usize],
    funcs: &[DecodedFunction],
    callers: &mut [HashSet<usize>],
    fi: usize,
    tree: &Tree,
    ext: &mut [u8; 4],
    types: &mut [FunctionType],
) -> bool {
    let Some(call) = last_op(&funcs[fi]).filter(|o| o.name == InstName::Cal) else {
        return false;
    };
    let Some(target) = funcs
        .iter()
        .position(|f| f.address == call.absolute_address())
    else {
        return false;
    };
    let target_tree = build_tree(&funcs[target].blocks);
    if !matches_tree(p, &p.is_last_warp_thread, &target_tree, None) {
        return false;
    }
    let Some(&first) = target_tree.roots.first() else {
        return false;
    };
    ext[0] = ((target_tree.nodes[first].op.raw >> 3) & 7) as u8;
    if matches_tree(p, pat, tree, Some(ext)) {
        if callers[target].remove(&fi) && callers[target].is_empty() {
            types[target] = FunctionType::Unused;
        }
        return true;
    }
    false
}
