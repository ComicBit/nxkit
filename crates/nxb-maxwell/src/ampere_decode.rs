//! `Decoder.DecodeAmpere`: 128-bit machine code to functions and basic blocks (`DecoderAmpere.cs`).

use super::ampere::{AmpOp, opc};
use super::ampere_table::{self, AEmit};
use super::decoder::{
    Block, Cb1Reader, Ctx, DecodedFunction, DecodedProgram, IndexingUse, bsearch_refs, get_block,
    split, user_attribute_uses,
};
use super::table::{InstOp, props};
use crate::attr::{AttributeUsage, Stage, feature};
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

pub const INST_SIZE: u64 = 16;

/// `AmpereTable.GetOp`.
pub fn get_op(address: u64, lo: u64, hi: u64) -> InstOp {
    let opcode = (lo & 0xfff) as usize;
    match ampere_table::lookup(opcode) {
        Some(e) => InstOp {
            address,
            raw: lo,
            raw_hi: hi,
            name: e.name,
            emit: None,
            aemit: Some(e.emit),
            ampere: true,
            props: e.props,
        },
        None => InstOp {
            address,
            raw: lo,
            raw_hi: hi,
            name: super::names::InstName::Invalid,
            emit: None,
            aemit: Some(AEmit::AUnsupported),
            ampere: true,
            props: 0,
        },
    }
}

/// `AmpereTarget`: the target of BRA and CALL.
pub fn target(op: &InstOp) -> u64 {
    ((op.address as i64) + INST_SIZE as i64 + AmpOp::of(op).branch_offset()) as u64
}

/// `IsUnconditionalAmpere`.
pub fn is_unconditional(op: &AmpOp) -> bool {
    if op.opcode() == opc::CALL {
        return false;
    }
    if op.pred() != 7 || op.pred_inv() {
        return false;
    }
    if matches!(op.opcode(), opc::BRA | opc::EXIT | opc::KILL) {
        return op.bits(87, 90) == 7 && !op.bit(90);
    }
    true
}

/// `DecodeAmpere`: repeats until every call into code that never returns is decoded as a jump.
pub fn decode(
    stage: Stage,
    acc: &mut dyn Cb1Reader,
    code: &[u8],
    start: u64,
) -> Result<(DecodedProgram, IndexingUse), String> {
    let mut jumps: HashSet<u64> = HashSet::new();
    let mut indexing = IndexingUse::default();
    loop {
        let (program, idx) = decode_once(stage, acc, code, start, &jumps)?;
        indexing.input_indexing |= idx.input_indexing;
        indexing.output_indexing |= idx.output_indexing;
        let mut changed = false;
        for (i, f) in program.functions.iter().enumerate() {
            if i != 0
                && !f
                    .blocks
                    .iter()
                    .any(|b| !b.ops.is_empty() && AmpOp::of(&b.last_op()).opcode() == opc::RET)
            {
                changed |= jumps.insert(f.address);
            }
        }
        if !changed {
            return Ok((program, indexing));
        }
    }
}

/// `IsAmpereJumpCall`: a CALL whose target is no function is a jump.
pub fn is_jump_call(program: &DecodedProgram, op: &InstOp) -> bool {
    let t = target(op);
    !program.functions.iter().any(|f| f.address == t)
}

fn decode_once(
    stage: Stage,
    acc: &mut dyn Cb1Reader,
    code: &[u8],
    start: u64,
    jumps: &HashSet<u64>,
) -> Result<(DecodedProgram, IndexingUse), String> {
    let mut ctx = Ctx {
        attr: AttributeUsage::default(),
        features: 0,
        clip: 0,
        cb1: 0,
        acc,
        indexing: IndexingUse::default(),
    };
    let mut brx_tables: HashMap<u64, BrxTable> = HashMap::new();
    let mut funcs: Vec<DecodedFunction> = Vec::new();
    let mut visited_fn: HashMap<u64, usize> = HashMap::new();
    let mut queue: VecDeque<usize> = VecDeque::new();
    fn enqueue(
        addr: u64,
        funcs: &mut Vec<DecodedFunction>,
        visited: &mut HashMap<u64, usize>,
        queue: &mut VecDeque<usize>,
    ) {
        if let std::collections::hash_map::Entry::Vacant(e) = visited.entry(addr) {
            funcs.push(DecodedFunction {
                address: addr,
                blocks: vec![],
            });
            e.insert(funcs.len() - 1);
            queue.push_back(funcs.len() - 1);
        }
    }
    enqueue(0, &mut funcs, &mut visited_fn, &mut queue);
    while let Some(fi) = queue.pop_front() {
        let fa = funcs[fi].address;
        let mut called = Vec::new();
        let blocks = decode_function(
            stage,
            &mut ctx,
            code,
            start,
            fa,
            jumps,
            &mut brx_tables,
            &mut called,
        )?;
        for a in called {
            enqueue(a, &mut funcs, &mut visited_fn, &mut queue);
        }
        funcs[fi].blocks = blocks;
    }
    let max_end = funcs
        .iter()
        .flat_map(|f| f.blocks.iter().map(|b| b.end_address))
        .max()
        .unwrap_or(0);
    Ok((
        DecodedProgram {
            functions: funcs,
            attribute_usage: ctx.attr,
            used_features: ctx.features,
            clip_distances_written: ctx.clip,
            cb1_data_size: ctx.cb1,
            size: max_end + start,
        },
        ctx.indexing,
    ))
}

#[allow(clippy::too_many_arguments)]
fn decode_function(
    stage: Stage,
    ctx: &mut Ctx,
    code: &[u8],
    start: u64,
    fn_addr: u64,
    jumps: &HashSet<u64>,
    brx_tables: &mut HashMap<u64, BrxTable>,
    called: &mut Vec<u64>,
) -> Result<Vec<Block>, String> {
    let mut all: Vec<Block> = Vec::new();
    let mut order: Vec<usize> = Vec::new();
    let mut work: VecDeque<usize> = VecDeque::new();
    let mut seen: HashMap<u64, usize> = HashMap::new();
    get_block(fn_addr, &mut all, &mut work, &mut seen);
    loop {
        while let Some(cur) = work.pop_front() {
            let sorted: Vec<&Block> = order.iter().map(|&i| &all[i]).collect();
            let (found, nidx) = bsearch_refs(&sorted, all[cur].address);
            if found {
                let n = order[nidx];
                if all[n].address == all[cur].address {
                    return Err("Found duplicate block address on the list.".into());
                }
                split(&mut all, n, cur)?;
                order.insert(nidx + 1, cur);
                continue;
            }
            let mut limit = u64::MAX;
            if nidx != order.len() {
                let n = &all[order[nidx]];
                let next = nidx + 1;
                if n.address < all[cur].address && next < order.len() {
                    limit = all[order[next]].address;
                } else if n.address > all[cur].address {
                    limit = n.address;
                }
            }
            fill_block(stage, ctx, code, start, &mut all[cur], limit);
            if !all[cur].ops.is_empty() {
                let last_op = all[cur].last_op();
                let last = AmpOp::of(&last_op);
                let jump_call = last.opcode() == opc::CALL && jumps.contains(&target(&last_op));
                if last.opcode() == opc::CALL && !jump_call {
                    called.push(target(&last_op));
                } else if last.opcode() == opc::BRA || jump_call {
                    let s = get_block(target(&last_op), &mut all, &mut work, &mut seen);
                    all[cur].succs.push(s);
                    all[s].preds.push(cur);
                }
                if !is_unconditional(&last) && !(jump_call && last.pred() == 7 && !last.pred_inv())
                {
                    let e = all[cur].end_address;
                    let s = get_block(e, &mut all, &mut work, &mut seen);
                    all[cur].succs.insert(0, s);
                    all[s].preds.push(cur);
                }
            }
            if !order.is_empty() {
                let n = &all[order[nidx]];
                let at = nidx + usize::from(n.address < all[cur].address);
                order.insert(at, cur);
            } else {
                order.push(cur);
            }
        }
        if !find_brx_targets(
            ctx, code, start, &mut all, &order, &mut work, &mut seen, brx_tables,
        ) {
            break;
        }
    }
    let mut pos = vec![0usize; all.len()];
    for (p, &i) in order.iter().enumerate() {
        pos[i] = p;
    }
    let mut out = Vec::new();
    for &i in &order {
        let mut b = all[i].clone();
        b.preds = b.preds.iter().map(|&x| pos[x]).collect();
        b.succs = b.succs.iter().map(|&x| pos[x]).collect();
        out.push(b);
    }
    Ok(out)
}

/// `FillBlockAmpere`.
fn fill_block(stage: Stage, ctx: &mut Ctx, code: &[u8], start: u64, block: &mut Block, limit: u64) {
    let mut address = block.address;
    loop {
        if address + INST_SIZE > limit {
            break;
        }
        let at = (start + address) as usize;
        let Some(w) = code.get(at..at + 16) else {
            break;
        };
        let word = |o: usize| {
            u64::from_le_bytes([
                w[o],
                w[o + 1],
                w[o + 2],
                w[o + 3],
                w[o + 4],
                w[o + 5],
                w[o + 6],
                w[o + 7],
            ])
        };
        let (lo, hi) = (word(0), word(8));
        let op = get_op(address, lo, hi);
        let a = AmpOp::new(lo, hi);
        match a.opcode() {
            opc::IPA => {
                user_attribute_uses(stage, ctx, a.bits(64, 72) << 2, 1, false, false, false)
            }
            opc::IPA_IDX => user_attribute_uses(stage, ctx, 0, 1, false, true, false),
            opc::ALD => user_attribute_uses(
                stage,
                ctx,
                a.bits(40, 50),
                a.bits(74, 76) + 1,
                a.bit(79),
                a.ra() != 255,
                a.bit(76),
            ),
            opc::AST => user_attribute_uses(
                stage,
                ctx,
                a.bits(40, 50),
                a.bits(74, 76) + 1,
                true,
                a.ra() != 255,
                a.bit(76),
            ),
            0x389 | 0x589 | 0x989 | 0xf89 => ctx.features |= feature::SHUFFLE,
            opc::LDL | 0x387 | 0x987 => ctx.features |= feature::LOCAL_MEMORY,
            0x927 | 0x984 | 0x388 | 0x988 | 0x98c | 0x38c | 0xf8c => {
                ctx.features |= feature::SHARED_MEMORY
            }
            0x986 | 0x386 | 0x98e | 0x9a8 | 0x3a8 | 0x9a9 | 0x59f | 0xb9b | 0x99b | 0x99d
            | 0x9a0 | 0x9a2 => ctx.features |= feature::STORE,
            _ => {}
        }
        block.ops.push(op);
        address += INST_SIZE;
        if op.has(props::BRA) {
            break;
        }
    }
    block.end_address = address;
}

#[derive(Default)]
struct BrxTable {
    base: i32,
    entry: i32,
    ended: bool,
    targets: HashSet<u64>,
}

/// `FindBrxTargetsAmpere`.
#[allow(clippy::too_many_arguments)]
fn find_brx_targets(
    ctx: &mut Ctx,
    code: &[u8],
    start: u64,
    all: &mut Vec<Block>,
    order: &[usize],
    work: &mut VecDeque<usize>,
    seen: &mut HashMap<u64, usize>,
    tables: &mut HashMap<u64, BrxTable>,
) -> bool {
    let mut has_new = false;
    for &bi in order {
        let last_op = all[bi].last_op();
        let op = AmpOp::of(&last_op);
        if matches!(op.opcode(), opc::BRX | opc::BRXU) && !tables.contains_key(&last_op.address) {
            let b = find_brx_table(all, bi, op.ra(), op.opcode() == opc::BRXU);
            if b >= 0 {
                tables.insert(
                    last_op.address,
                    BrxTable {
                        base: b,
                        ..Default::default()
                    },
                );
            }
        }
    }
    let bases: BTreeSet<i32> = tables.values().map(|t| t.base).collect();
    for &bi in order {
        let last_op = all[bi].last_op();
        let brx = AmpOp::of(&last_op);
        if !matches!(brx.opcode(), opc::BRX | opc::BRXU) {
            continue;
        }
        let Some(table) = tables.get_mut(&last_op.address) else {
            continue;
        };
        if table.ended {
            continue;
        }
        let next = last_op.address as i64 + INST_SIZE as i64 + brx.branch_offset();
        let mut table_end = ctx.acc.constant_buffer1_size();
        if let Some(b) = bases.range(table.base + 1..).next() {
            table_end = table_end.min(*b);
        }
        while table.base + table.entry * 4 + 4 <= table_end {
            let v = ctx.read_cb1(table.base + table.entry * 4) as i32;
            let target = next + i64::from(v);
            table.entry += 1;
            if target == 0 {
                continue;
            }
            let in_code = target >= 0
                && code
                    .get((start + target as u64) as usize..)
                    .is_some_and(|c| c.len() >= 16);
            if target < 0 || target % INST_SIZE as i64 != 0 || !in_code {
                table.ended = true;
                break;
            }
            if table.targets.insert(target as u64) {
                let t = get_block(target as u64, all, work, seen);
                all[t].preds.push(bi);
                all[bi].succs.push(t);
                has_new = true;
                break;
            }
        }
    }
    has_new
}

/// `FindBrxTableAmpere`: the c[0x1] byte offset of the LDC that last wrote `reg` before the block ends on every path to
/// it, or -1 when paths differ or something else wrote it.
fn find_brx_table(all: &[Block], block: usize, reg: i32, uniform: bool) -> i32 {
    let mut visited: HashSet<usize> = HashSet::from([block]);
    let mut pending = vec![block];
    let mut table = -1;
    while let Some(cur) = pending.pop() {
        let mut written = false;
        for op in all[cur].ops.iter().rev() {
            if written {
                break;
            }
            let a = AmpOp::of(op);
            let want = if uniform { 0xabb } else { opc::LDC };
            if a.opcode() == want && a.rd() == reg {
                let offset = if a.bits(54, 59) == 1 && a.bits(78, 80) == 0 {
                    a.signed_field(38, 54) as i32
                } else {
                    -1
                };
                if offset < 0 || (table >= 0 && table != offset) {
                    return -1;
                }
                table = offset;
                written = true;
            } else if a.rd() == reg
                && ampere_table::writes_rd(a.opcode())
                && ampere_table::has_uniform_guard(a.opcode()) == uniform
            {
                return -1;
            }
        }
        if !written {
            if all[cur].preds.is_empty() {
                return -1;
            }
            for &p in &all[cur].preds {
                if visited.insert(p) {
                    pending.push(p);
                }
            }
        }
    }
    table
}
