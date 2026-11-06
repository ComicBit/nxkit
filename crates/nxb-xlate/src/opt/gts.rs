//! `GlobalToStorage`: the inline storage path (global memory loads, stores and atomics with a known storage buffer).
//! The helper-function paths (small integers, single and multi target functions, call generation) are in `gts_fn`.
//! the raw path (address traced to no storage buffer) fails closed.
use super::gts_fn::{self, Entry};
use super::{Env, utils};
use crate::ir::{Block, Inst, Ir, NodeId, Opd, OperandType, StorageKind};
use crate::xform::helper::Hfm;
use std::collections::HashMap;

type R<T> = Result<T, String>;

const DRIVER_RESERVED_CB: i32 = 0;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum LsType {
    Local,
    Shared,
}

#[derive(Clone, Copy)]
struct Search {
    sb_cb_slot: i32,
    sb_cb_offset: i32,
    offset: Option<Opd>,
    const_offset: i32,
}

impl Search {
    fn found(&self) -> bool {
        self.sb_cb_slot != -1
    }
    fn not_found() -> Search {
        Search {
            sb_cb_slot: -1,
            sb_cb_offset: 0,
            offset: None,
            const_offset: 0,
        }
    }
}

type LsKey = (Option<Opd>, i32, LsType);

#[derive(Default)]
struct Gts {
    shared: HashMap<LsKey, Vec<(u32, Search)>>,
    entries: Vec<Entry>,
}

impl Gts {
    fn add_memory_target_cb(
        &mut self,
        ty: LsType,
        base: Option<Opd>,
        co: i32,
        target: u32,
        r: Search,
    ) {
        let entry = self.shared.entry((base, co, ty)).or_default();
        if let Some(e) = entry.iter_mut().find(|(t, _)| *t == target) {
            let ex = e.1;
            if ex.offset.is_some() && (ex.offset != r.offset || ex.const_offset != r.const_offset) {
                e.1 = Search {
                    sb_cb_slot: r.sb_cb_slot,
                    sb_cb_offset: r.sb_cb_offset,
                    offset: None,
                    const_offset: 0,
                };
            }
        } else {
            entry.push((target, r));
        }
    }

    fn try_get_memory_target_cb(&self, ty: LsType, base: Option<Opd>, co: i32) -> Option<Search> {
        let e = self.shared.get(&(base, co, ty))?;
        if e.len() == 1 && e[0].1.found() {
            return Some(e[0].1);
        }
        None
    }
}

fn s(ir: &Ir, n: NodeId, i: usize) -> R<Opd> {
    ir.op(n)
        .sources
        .get(i)
        .copied()
        .flatten()
        .ok_or_else(|| "missing source".to_string())
}

fn is_global_memory(k: StorageKind) -> bool {
    matches!(
        k,
        StorageKind::GlobalMemory
            | StorageKind::GlobalMemoryS8
            | StorageKind::GlobalMemoryS16
            | StorageKind::GlobalMemoryU8
            | StorageKind::GlobalMemoryU16
    )
}

pub fn run_pass(ir: &mut Ir, blocks: &mut [Block], env: &mut Env, hfm: &mut Hfm) -> R<()> {
    let mut ctx = Gts::default();
    for bi in 0..blocks.len() {
        let mut i = 0;
        while i < blocks[bi].ops.len() {
            let node = blocks[bi].ops[i];
            if !ir.is_op(node) {
                i += 1;
                continue;
            }
            let (inst, kind) = (ir.op(node).inst, ir.op(node).storage_kind);
            if is_global_memory(kind) {
                i = replace(ir, blocks, bi, i, env, &mut ctx, hfm)?;
            } else if inst == Inst::Store
                && (kind == StorageKind::SharedMemory || kind == StorageKind::LocalMemory)
            {
                if let Some((ty, base, co)) = try_get_memory_offsets(ir, node)? {
                    let n = ir.op(node).sources.len();
                    let value = s(ir, node, n - 1)?;
                    let r = find_unique_base_address_cb(ir, blocks, bi, &ctx, value, false)?;
                    if r.found() {
                        let target = gts_fn::pack_cb(r.sb_cb_slot, r.sb_cb_offset);
                        ctx.add_memory_target_cb(ty, base, co, target, r);
                    }
                }
                i += 1;
            } else {
                i += 1;
            }
        }
    }
    Ok(())
}

/// Returns the index to continue from.
fn replace(
    ir: &mut Ir,
    blocks: &mut [Block],
    bi: usize,
    pos: usize,
    env: &mut Env,
    ctx: &mut Gts,
    hfm: &mut Hfm,
) -> R<usize> {
    let node = blocks[bi].ops[pos];
    let global_address = s(ir, node, 0)?;
    let r = find_unique_base_address_cb(ir, blocks, bi, ctx, global_address, true)?;
    if !r.found() {
        return multi_target(ir, blocks, bi, pos, env, ctx, hfm);
    }
    let mut pos = pos;
    let mut offset = r.offset.ok_or("storage offset")?;
    // QueryHasUnalignedStorageBuffer is false for the job accessor
    if r.const_offset != 0 {
        let new_offset = ir.local();
        let c = ir.const_(r.const_offset);
        let add = ir.new_op(Inst::Add, StorageKind::None, Some(new_offset), &[offset, c]);
        blocks[bi].ops.insert(pos, add);
        pos += 1;
        offset = new_offset;
    }
    let (inst, kind) = (ir.op(node).inst, ir.op(node).storage_kind);
    let can_inline = kind == StorageKind::GlobalMemory; // the target language is SPIR-V
    if !can_inline {
        let targets = [gts_fn::pack_cb(r.sb_cb_slot, r.sb_cb_offset)];
        let id = match gts_fn::find_function(&ctx.entries, inst, kind, false, &targets) {
            Some(id) => id,
            None => {
                let Some(f) =
                    gts_fn::single_target(ir, env, inst, kind, r.sb_cb_slot, r.sb_cb_offset)?
                else {
                    return Err(
                        "unported: failed to reserve a storage buffer for a global memory operation"
                            .into(),
                    );
                };
                let id = hfm.add_function(f);
                ctx.entries
                    .push(Entry::new(id, inst, kind, false, targets.to_vec()));
                id
            }
        };
        return Ok(call_storage_op(ir, blocks, bi, pos, Some(offset), id));
    }
    let is_store = inst == Inst::Store || gts_fn::is_atomic(Inst(inst.0 & Inst::MASK));
    let Some(binding) =
        env.rm
            .try_get_storage_buffer_binding(env.acc, r.sb_cb_slot, r.sb_cb_offset, is_store)
    else {
        return Err(
            "unported: failed to reserve a storage buffer for a global memory operation".into(),
        );
    };
    let word_offset = ir.local();
    let nsrc = ir.op(node).sources.len();
    let c_binding = ir.const_(binding);
    let c0 = ir.const_(0);
    let sources: Vec<Opd> = if inst == Inst::AtomicCompareAndSwap {
        vec![
            c_binding,
            c0,
            word_offset,
            s(ir, node, nsrc - 2)?,
            s(ir, node, nsrc - 1)?,
        ]
    } else if is_store {
        vec![c_binding, c0, word_offset, s(ir, node, nsrc - 1)?]
    } else {
        vec![c_binding, c0, word_offset]
    };
    let c2 = ir.const_(2);
    let shift = ir.new_op(
        Inst::ShiftRightU32,
        StorageKind::None,
        Some(word_offset),
        &[offset, c2],
    );
    let dest = ir.op(node).dests.first().copied().flatten();
    let storage = ir.new_op(inst, StorageKind::StorageBuffer, dest, &sources);
    blocks[bi].ops.insert(pos, shift);
    blocks[bi].ops.insert(pos + 1, storage);
    utils::delete_node(ir, &mut blocks[bi], pos + 2);
    Ok(pos + 2)
}

/// `TryGenerateMultiTargetStorageOp` and `GenerateCallStorageOp` without an offset.
fn multi_target(
    ir: &mut Ir,
    blocks: &mut [Block],
    bi: usize,
    pos: usize,
    env: &mut Env,
    ctx: &mut Gts,
    hfm: &mut Hfm,
) -> R<usize> {
    let node = blocks[bi].ops[pos];
    let (inst, kind) = (ir.op(node).inst, ir.op(node).storage_kind);
    let mut global_address = s(ir, node, 0)?;
    if let Some(add) = ir.asg_operation(global_address)
        && ir.op(add).inst == Inst::Add
    {
        let (a, c) = (s(ir, add, 0)?, s(ir, add, 1)?);
        if ir.ty(a) == OperandType::Constant && ir.ty(c) == OperandType::LocalVariable {
            global_address = c;
        } else if ir.ty(a) == OperandType::LocalVariable && ir.ty(c) == OperandType::Constant {
            global_address = a;
        }
    }
    let phi_of = |ir: &Ir, o: Opd| ir.data(o).asg_op.filter(|n| ir.is_phi(*n));
    let mut visited: Vec<NodeId> = Vec::new();
    let mut queue: std::collections::VecDeque<NodeId> = Default::default();
    let mut targets: Vec<u32> = Vec::new();
    if let Some(phi) = phi_of(ir, global_address) {
        visited.push(phi);
        queue.push_back(phi);
    } else {
        let a0 = s(ir, node, 0)?;
        let r = find_unique_base_address_cb(ir, blocks, bi, ctx, a0, false)?;
        if r.found() {
            targets.push(gts_fn::pack_cb(r.sb_cb_slot, r.sb_cb_offset));
        }
    }
    while let Some(phi) = queue.pop_front() {
        let sources = match &ir.nodes[phi.0 as usize] {
            crate::ir::Node::Phi(p) => p.sources.clone(),
            _ => return Err("phi expected".into()),
        };
        for (block, source) in sources {
            let r = find_unique_base_address_cb(ir, blocks, block as usize, ctx, source, false)?;
            if r.found() {
                let t = gts_fn::pack_cb(r.sb_cb_slot, r.sb_cb_offset);
                if !targets.contains(&t) {
                    targets.push(t);
                }
            } else if let Some(p2) = phi_of(ir, source)
                && !visited.contains(&p2)
            {
                visited.push(p2);
                queue.push_back(p2);
            }
        }
    }
    targets.sort_unstable();
    if targets.is_empty() {
        let m = format!(
            "Global memory operation \"{}\" traced to no storage buffer: it goes through the memory pool table.",
            gts_fn::inst_name(inst)?
        );
        env.acc.log(&m);
    }
    let id = match gts_fn::find_function(&ctx.entries, inst, kind, true, &targets) {
        Some(id) => id,
        None => {
            let returns = !ir.op(node).dests.is_empty();
            let made = if targets.is_empty() {
                super::gts_raw::raw_target(ir, env, inst, kind, returns)?
            } else {
                gts_fn::multi_target(ir, env, inst, kind, returns, &targets)?
            };
            let Some(f) = made else {
                return Err(
                    "unported: failed to reserve a storage buffer for a global memory operation"
                        .into(),
                );
            };
            let id = hfm.add_function(f);
            ctx.entries.push(Entry::new(id, inst, kind, true, targets));
            id
        }
    };
    Ok(call_storage_op(ir, blocks, bi, pos, None, id))
}

/// `GenerateCallStorageOp`: returns the index to continue from.
fn call_storage_op(
    ir: &mut Ir,
    blocks: &mut [Block],
    bi: usize,
    pos: usize,
    offset: Option<Opd>,
    function_id: i32,
) -> usize {
    let node = blocks[bi].ops[pos];
    let ops: Vec<Opd> = ir.op(node).sources.iter().flatten().copied().collect();
    let mut sources = vec![ir.const_(function_id)];
    match offset {
        Some(o) => {
            sources.push(o);
            sources.extend_from_slice(&ops[2..]);
        }
        None => sources.extend_from_slice(&ops),
    }
    let returns = ir.op(node).dests.first().copied().flatten().is_some();
    let rv = returns.then(|| ir.local());
    let call = ir.new_op(Inst::Call, StorageKind::None, rv, &sources);
    blocks[bi].ops.insert(pos, call);
    if let Some(rv) = rv {
        ir.turn_into_copy(node, rv);
        pos + 2
    } else {
        utils::delete_node(ir, &mut blocks[bi], pos + 1);
        pos + 1
    }
}

fn get_base_address_cb_with_offset(ir: &Ir, base: Opd, offset: Opd, const_offset: i32) -> Search {
    if ir.ty(base) == OperandType::ConstantBuffer {
        let (slot, off) = (ir.cbuf_slot(base), ir.cbuf_offset(base));
        if off & 1 == 0 {
            return Search {
                sb_cb_slot: slot,
                sb_cb_offset: off,
                offset: Some(offset),
                const_offset,
            };
        }
    }
    Search::not_found()
}

fn find_from_memory(
    ir: &Ir,
    ctx: &Gts,
    op: Option<NodeId>,
    const_offset: i32,
    needs_offset: bool,
) -> R<Search> {
    if let Some(op) = op
        && let Some((ty, bo, co)) = try_get_memory_offsets(ir, op)?
        && let Some(r) = ctx.try_get_memory_target_cb(ty, bo, co)
        && (r.offset.is_some() || !needs_offset)
    {
        if const_offset != 0 {
            return Ok(Search {
                const_offset: r.const_offset + const_offset,
                ..r
            });
        }
        return Ok(r);
    }
    Ok(Search::not_found())
}

fn find_unique_base_address_cb(
    ir: &mut Ir,
    blocks: &[Block],
    bi: usize,
    ctx: &Gts,
    global_address: Opd,
    needs_offset: bool,
) -> R<Search> {
    let global_address = utils::find_last_operation(ir, blocks, global_address, bi, true)?;
    if ir.ty(global_address) == OperandType::ConstantBuffer {
        let z = ir.const_(0);
        return Ok(get_base_address_cb_with_offset(ir, global_address, z, 0));
    }
    let mut operation = ir.asg_operation(global_address);
    let Some(op) = operation.filter(|o| ir.op(*o).inst == Inst::Add) else {
        return find_from_memory(ir, ctx, operation, 0, needs_offset);
    };
    let (mut src1, mut src2) = (s(ir, op, 0)?, s(ir, op, 1)?);
    let mut const_offset = 0;
    let (t1, t2) = (ir.ty(src1), ir.ty(src2));
    if (t1 == OperandType::LocalVariable && t2 == OperandType::Constant)
        || (t2 == OperandType::LocalVariable && t1 == OperandType::Constant)
    {
        let (base_addr, offset) = if t1 == OperandType::LocalVariable {
            (
                utils::find_last_operation(ir, blocks, src1, bi, true)?,
                src2,
            )
        } else {
            (
                utils::find_last_operation(ir, blocks, src2, bi, true)?,
                src1,
            )
        };
        let r = get_base_address_cb_with_offset(ir, base_addr, offset, 0);
        if r.found() {
            return Ok(r);
        }
        const_offset = ir.value(offset);
        operation = ir.asg_operation(base_addr);
        match operation.filter(|o| ir.op(*o).inst == Inst::Add) {
            None => return find_from_memory(ir, ctx, operation, const_offset, needs_offset),
            Some(o) => {
                src1 = s(ir, o, 0)?;
                src2 = s(ir, o, 1)?;
            }
        }
    }
    let (c1, c2) = (
        ir.ty(src1) == OperandType::ConstantBuffer,
        ir.ty(src2) == OperandType::ConstantBuffer,
    );
    if c1
        && c2
        && ir.cbuf_slot(src1) == DRIVER_RESERVED_CB
        && ir.cbuf_slot(src2) != DRIVER_RESERVED_CB
    {
        return Ok(get_base_address_cb_with_offset(
            ir,
            src1,
            src2,
            const_offset,
        ));
    }
    if !c1 || (c1 && c2) || (c2 && ir.cbuf_slot(src2) == DRIVER_RESERVED_CB) {
        return Ok(get_base_address_cb_with_offset(
            ir,
            src2,
            src1,
            const_offset,
        ));
    }
    Ok(get_base_address_cb_with_offset(
        ir,
        src1,
        src2,
        const_offset,
    ))
}

fn try_get_memory_offsets(ir: &Ir, op: NodeId) -> R<Option<(LsType, Option<Opd>, i32)>> {
    let o = ir.op(op);
    if o.inst == Inst::Load || o.inst == Inst::Store {
        if o.storage_kind == StorageKind::SharedMemory {
            return Ok(shared_offsets(ir, op)?.map(|(b, c)| (LsType::Shared, Some(b), c)));
        } else if o.storage_kind == StorageKind::LocalMemory {
            let off = s(ir, op, 1)?;
            if ir.ty(off) == OperandType::Constant {
                return Ok(Some((LsType::Local, None, ir.value(off))));
            }
            return Ok(None);
        }
    }
    Ok(None)
}

fn shared_offsets(ir: &Ir, op: NodeId) -> R<Option<(Opd, i32)>> {
    let mut base: Option<Opd> = None;
    let mut const_offset = 0;
    let src1 = s(ir, op, 1)?;
    if let Some(sr) = ir.asg_operation(src1)
        && ir.op(sr).inst == Inst::ShiftRightU32
    {
        let c = s(ir, sr, 1)?;
        if ir.ty(c) == OperandType::Constant && ir.value(c) == 2 {
            base = Some(s(ir, sr, 0)?);
        }
    }
    if let Some(b) = base
        && let Some(add) = ir.asg_operation(b)
        && ir.op(add).inst == Inst::Add
    {
        let (a, c) = (s(ir, add, 0)?, s(ir, add, 1)?);
        if ir.ty(a) == OperandType::Constant && ir.ty(c) == OperandType::LocalVariable {
            const_offset = ir.value(a);
            base = Some(c);
        } else if ir.ty(a) == OperandType::LocalVariable && ir.ty(c) == OperandType::Constant {
            base = Some(a);
            const_offset = ir.value(c);
        }
    }
    Ok(match base {
        Some(b) if ir.ty(b) == OperandType::LocalVariable => Some((b, const_offset)),
        _ => None,
    })
}
