//! `TensorPass.Uniformity`: which values are the same in every lane of a subgroup (where all 32 run together, as at an
//! HMMA), and of the others, which are U + f(lane) with f known here: the lane-dependent part of an LDSM's row
//! addresses. Divergent: the lane, the thread index components that change within a subgroup, local memory, atomics,
//! calls, textures, any operation outside `pure`, phis where paths meet (all but the phis of a loop header whose exits
//! all branch on uniform conditions, and, when the loop has several back edges, whose other branches do too), and what
//! is computed from them.

use crate::ir::{Block, Inst, IoVariable, Ir, Node, NodeId, Opd, OperandType, StorageKind};
use std::collections::{HashMap, HashSet};

type Lanes = [i32; 32];

/// `x = U + lanes[lane]`, U the same in every lane (zero when `pure`).
type Form = Option<(Lanes, bool)>;

/// The blocks of a loop whose branches its header's phis depend on: those that leave it, and with several back edges
/// the others.
struct Loop {
    exits: Vec<usize>,
    inner: Vec<usize>,
}

pub(super) struct Uniformity {
    divergent: HashSet<Opd>,
    forms: HashMap<Opd, Form>,
    size_x: i32,
    size_y: i32,
}

impl Uniformity {
    pub(super) fn new(
        ir: &Ir,
        blocks: &[Block],
        idom: &[Option<usize>],
        size: [i32; 3],
    ) -> Uniformity {
        let mut u = Uniformity {
            divergent: HashSet::new(),
            forms: HashMap::new(),
            size_x: size[0],
            size_y: size[1],
        };
        let loops: HashMap<usize, Loop> = blocks
            .iter()
            .filter_map(|b| loop_of(blocks, idom, b.index).map(|l| (b.index, l)))
            .collect();
        loop {
            let mut changed = false;
            for block in blocks {
                for &n in &block.ops {
                    let dests: Vec<Opd> = ir
                        .node_dests(n)
                        .into_iter()
                        .flatten()
                        .filter(|d| ir.ty(*d) == OperandType::LocalVariable)
                        .collect();
                    if dests.iter().all(|d| u.divergent.contains(d)) {
                        continue;
                    }
                    if u.diverges(ir, blocks, n, block.index, &loops) {
                        u.divergent.extend(dests);
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        u
    }

    fn is_divergent(&self, ir: &Ir, x: Option<Opd>) -> bool {
        x.is_some_and(|x| ir.ty(x) == OperandType::LocalVariable && self.divergent.contains(&x))
    }

    fn any_source_divergent(&self, ir: &Ir, n: NodeId) -> bool {
        ir.node_sources(n)
            .into_iter()
            .any(|s| self.is_divergent(ir, s))
    }

    /// The first of the blocks that ends in a conditional branch on a divergent condition.
    fn divergent_branch(&self, ir: &Ir, blocks: &[Block], list: &[usize]) -> bool {
        list.iter().any(|&b| {
            blocks[b].ops.last().is_some_and(|&n| {
                ir.is_op(n)
                    && matches!(ir.op(n).inst, Inst::BranchIfTrue | Inst::BranchIfFalse)
                    && self.is_divergent(ir, ir.op(n).sources.first().copied().flatten())
            })
        })
    }

    fn diverges(
        &self,
        ir: &Ir,
        blocks: &[Block],
        n: NodeId,
        block: usize,
        loops: &HashMap<usize, Loop>,
    ) -> bool {
        match &ir.nodes[n.0 as usize] {
            Node::Phi(_) => {
                let Some(l) = loops.get(&block) else {
                    return true;
                };
                if self.divergent_branch(ir, blocks, &l.exits)
                    || self.divergent_branch(ir, blocks, &l.inner)
                {
                    return true;
                }
                self.any_source_divergent(ir, n)
            }
            Node::Op(op) => {
                if op.inst == Inst::Load {
                    return match op.storage_kind {
                        StorageKind::Input => !self.uniform_input(ir, n),
                        StorageKind::ConstantBuffer
                        | StorageKind::StorageBuffer
                        | StorageKind::SharedMemory => self.any_source_divergent(ir, n),
                        _ => true,
                    };
                }
                !pure(op.inst) || self.any_source_divergent(ir, n)
            }
            Node::Comment(_) => false,
        }
    }

    fn uniform_input(&self, ir: &Ir, n: NodeId) -> bool {
        let o = ir.op(n);
        let src: Vec<Opd> = o.sources.iter().flatten().copied().collect();
        let Some(&v) = src.first() else {
            return false;
        };
        let v = IoVariable(ir.value(v) as u32);
        if v == IoVariable::CtaId {
            return true;
        }
        if v == IoVariable::ThreadId && src.len() == 2 && ir.ty(src[1]) == OperandType::Constant {
            return match ir.value(src[1]) {
                1 => self.size_x % 32 == 0,
                2 => (self.size_x * self.size_y) % 32 == 0,
                _ => false,
            };
        }
        false
    }

    /// `RowStarts`: lane L's row of an LDSM at `x` is U + starts[m] + r * stride for lane 8m + r of each of its `count`
    /// matrices, U the same in every lane. The starts are relative to lane 0's row.
    pub(super) fn row_starts(&mut self, ir: &Ir, x: Opd, count: usize) -> Option<(Vec<i32>, i32)> {
        let (lanes, _) = self.form(ir, x, 0)?;
        let stride = lanes[1].wrapping_sub(lanes[0]);
        // An LDSM's rows are 16-byte aligned.
        if stride <= 0 || stride % 16 != 0 {
            return None;
        }
        let mut starts = vec![0; count];
        for (m, start) in starts.iter_mut().enumerate() {
            *start = lanes[8 * m].wrapping_sub(lanes[0]);
            if *start % 16 != 0 {
                return None;
            }
            for r in 1..8 {
                if lanes[8 * m + r] != lanes[8 * m].wrapping_add(r as i32 * stride) {
                    return None;
                }
            }
        }
        Some((starts, stride))
    }

    fn form(&mut self, ir: &Ir, x: Opd, depth: u32) -> Form {
        if ir.ty(x) == OperandType::Constant {
            return Some(([ir.value(x); 32], true));
        }
        if ir.ty(x) != OperandType::LocalVariable || depth > 32 {
            return None;
        }
        if !self.divergent.contains(&x) {
            return Some(([0; 32], false));
        }
        if let Some(f) = self.forms.get(&x) {
            return *f;
        }
        let f = self.compute(ir, x, depth);
        self.forms.insert(x, f);
        f
    }

    fn compute(&mut self, ir: &Ir, x: Opd, depth: u32) -> Form {
        let n = ir.asg_operation(x)?;
        let op = ir.op(n);
        if op.inst.0 & !Inst::MASK != 0 {
            return None;
        }
        let src: Vec<Opd> = op.sources.iter().flatten().copied().collect();
        if op.inst == Inst::Load {
            return if op.storage_kind == StorageKind::Input {
                self.input_form(ir, &src)
            } else {
                None
            };
        }
        let inst = op.inst;
        if inst == Inst::Copy {
            return self.form(ir, *src.first()?, depth + 1);
        }
        if src.is_empty() || src.len() > 2 {
            return None;
        }
        let (a, pa) = self.form(ir, src[0], depth + 1)?;
        let (b, pb) = if src.len() == 2 {
            self.form(ir, src[1], depth + 1)?
        } else {
            ([0; 32], true)
        };
        let const_a = ir.ty(src[0]) == OperandType::Constant;
        let const_b = src.len() == 2 && ir.ty(src[1]) == OperandType::Constant;
        // Linear in U: sums, and products and left shifts by a constant.
        match inst {
            Inst::Add => return Some((map(&a, &b, i32::wrapping_add), pa && pb)),
            Inst::Subtract => return Some((map(&a, &b, i32::wrapping_sub), pa && pb)),
            Inst::Multiply if const_a || const_b => {
                return Some((
                    map(&a, &b, i32::wrapping_mul),
                    if const_a { pb } else { pa },
                ));
            }
            Inst::ShiftLeft if const_b => return Some((map(&a, &b, shl), pa)),
            _ => {}
        }
        if !pa || !pb {
            return None;
        }
        let f: fn(i32, i32) -> i32 = match inst {
            Inst::Multiply => i32::wrapping_mul,
            Inst::ShiftLeft => shl,
            Inst::ShiftRightU32 => |u, v| ((u as u32) >> (v & 31)) as i32,
            Inst::ShiftRightS32 => |u, v| u >> (v & 31),
            Inst::BitwiseAnd => |u, v| u & v,
            Inst::BitwiseOr => |u, v| u | v,
            Inst::BitwiseExclusiveOr => |u, v| u ^ v,
            Inst::BitwiseNot => |u, _| !u,
            Inst::Minimum => |u, v| u.min(v),
            Inst::Maximum => |u, v| u.max(v),
            Inst::MinimumU32 => |u, v| (u as u32).min(v as u32) as i32,
            Inst::MaximumU32 => |u, v| (u as u32).max(v as u32) as i32,
            _ => return None,
        };
        Some((map(&a, &b, f), true))
    }

    /// The lane and the thread index: a subgroup is 32 consecutive threads of the workgroup in x, then y order.
    fn input_form(&self, ir: &Ir, src: &[Opd]) -> Form {
        let v = IoVariable(ir.value(*src.first()?) as u32);
        let lane: Lanes = std::array::from_fn(|l| l as i32);
        if v == IoVariable::SubgroupLaneId && src.len() == 1 {
            return Some((lane, true));
        }
        if v != IoVariable::ThreadId || src.len() != 2 || ir.ty(src[1]) != OperandType::Constant {
            return None;
        }
        let component = ir.value(src[1]);
        let (sx, sy) = (self.size_x, self.size_y);
        if component == 0 && sx % 32 == 0 {
            return Some((lane, false));
        }
        if sx > 0 && sx < 32 && 32 % sx == 0 && (sx * sy) % 32 == 0 && component < 2 {
            let lanes = std::array::from_fn(|l| {
                if component == 0 {
                    l as i32 % sx
                } else {
                    l as i32 / sx
                }
            });
            return Some((lanes, component == 0));
        }
        None
    }
}

fn shl(u: i32, v: i32) -> i32 {
    u.wrapping_shl((v & 31) as u32)
}

fn map(a: &Lanes, b: &Lanes, f: fn(i32, i32) -> i32) -> Lanes {
    std::array::from_fn(|l| f(a[l], b[l]))
}

/// The loop headed by block `header`, or `None` when it heads none. Latches are its predecessors it dominates.
fn loop_of(blocks: &[Block], idom: &[Option<usize>], header: usize) -> Option<Loop> {
    let mut body: HashSet<usize> = HashSet::from([header]);
    let mut work = Vec::new();
    let mut latches = 0;
    for &p in &blocks[header].preds {
        if dominates(idom, header, p) {
            latches += 1;
            if body.insert(p) {
                work.push(p);
            }
        }
    }
    if latches == 0 {
        return None;
    }
    while let Some(b) = work.pop() {
        for &p in &blocks[b].preds {
            if body.insert(p) {
                work.push(p);
            }
        }
    }
    let (mut exits, mut inner) = (Vec::new(), Vec::new());
    let mut list: Vec<usize> = body.iter().copied().collect();
    list.sort_unstable();
    for b in list {
        let out = |t: Option<usize>| t.is_some_and(|t| !body.contains(&t));
        if out(blocks[b].next) || out(blocks[b].branch) {
            exits.push(b);
        } else if latches > 1 {
            inner.push(b);
        }
    }
    Some(Loop { exits, inner })
}

fn dominates(idom: &[Option<usize>], a: usize, b: usize) -> bool {
    let mut x = Some(b);
    while let Some(v) = x {
        if v == a {
            return true;
        }
        x = idom.get(v).copied().flatten().filter(|&d| d != v);
    }
    false
}

/// `Pure`: an operation whose result follows from its sources alone.
fn pure(inst: Inst) -> bool {
    matches!(
        Inst(inst.0 & Inst::MASK),
        Inst::Add
            | Inst::Subtract
            | Inst::Multiply
            | Inst::MultiplyHighS32
            | Inst::MultiplyHighU32
            | Inst::Divide
            | Inst::Modulo
            | Inst::Negate
            | Inst::Minimum
            | Inst::MinimumU32
            | Inst::Maximum
            | Inst::MaximumU32
            | Inst::Clamp
            | Inst::ClampU32
            | Inst::ShiftLeft
            | Inst::ShiftRightS32
            | Inst::ShiftRightU32
            | Inst::BitwiseAnd
            | Inst::BitwiseOr
            | Inst::BitwiseExclusiveOr
            | Inst::BitwiseNot
            | Inst::BitfieldExtractS32
            | Inst::BitfieldExtractU32
            | Inst::BitfieldInsert
            | Inst::BitCount
            | Inst::FindLSB
            | Inst::FindMSBS32
            | Inst::FindMSBU32
            | Inst::ConditionalSelect
            | Inst::CompareEqual
            | Inst::CompareNotEqual
            | Inst::CompareLess
            | Inst::CompareLessU32
            | Inst::CompareLessOrEqual
            | Inst::CompareLessOrEqualU32
            | Inst::CompareGreater
            | Inst::CompareGreaterU32
            | Inst::CompareGreaterOrEqual
            | Inst::CompareGreaterOrEqualU32
            | Inst::LogicalAnd
            | Inst::LogicalOr
            | Inst::LogicalNot
            | Inst::LogicalExclusiveOr
            | Inst::ConvertFP32ToS32
            | Inst::ConvertFP32ToU32
            | Inst::ConvertS32ToFP32
            | Inst::ConvertU32ToFP32
            | Inst::ConvertFP32ToFP64
            | Inst::ConvertFP64ToFP32
            | Inst::ConvertFP64ToS32
            | Inst::ConvertFP64ToU32
            | Inst::ConvertS32ToFP64
            | Inst::ConvertU32ToFP64
            | Inst::Absolute
            | Inst::Ceiling
            | Inst::Floor
            | Inst::Round
            | Inst::Truncate
            | Inst::FusedMultiplyAdd
            | Inst::SquareRoot
            | Inst::ReciprocalSquareRoot
            | Inst::ExponentB2
            | Inst::LogarithmB2
            | Inst::Sine
            | Inst::Cosine
            | Inst::IsNan
            | Inst::PackHalf2x16
            | Inst::UnpackHalf2x16
            | Inst::PackDouble2x32
            | Inst::UnpackDouble2x32
            | Inst::Copy
    )
}
