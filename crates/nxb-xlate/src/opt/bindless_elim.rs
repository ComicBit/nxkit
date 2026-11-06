//! `BindlessElimination`.
use super::bindless_array::MINIMUM_ARRAY_LENGTH;
use super::bindless_convert::try_convert_bindless;
use super::tex::{self, ATOMIC_METAL, BINDLESS};
use super::{Env, utils};
use crate::ir::{Block, Inst, Ir, NodeId, Opd, OperandType, StorageKind};
use crate::translation::props::TextureFlags;
use crate::types::SamplerType;
use std::collections::HashSet;

type R<T> = Result<T, String>;

const NVN_TEXTURE_BUFFER_INDEX: i32 = 2;
const NVN_SLOT_ARRAY_LENGTH: i32 = 32;
/// c[2]'s separate sampler table, 128 slots of 8 bytes. A word there is a sampler handle, never a texture's.
const NVN_SEPARATE_SAMPLER_BASE: i32 = 0x568;
const NVN_SEPARATE_SAMPLER_END: i32 = NVN_SEPARATE_SAMPLER_BASE + 8 * 128;

#[derive(PartialEq, Eq, Clone, Copy)]
enum Table {
    None,
    Set,
    Deferred,
}

fn s(ir: &Ir, n: NodeId, i: usize) -> R<Opd> {
    ir.op(n)
        .sources
        .get(i)
        .copied()
        .flatten()
        .ok_or_else(|| "missing source".to_string())
}

fn asg_phi(ir: &Ir, o: Opd) -> Option<NodeId> {
    ir.data(o).asg_op.filter(|n| ir.is_phi(*n))
}

fn op_of(ir: &Ir, o: Opd, inst: Inst) -> Option<NodeId> {
    ir.asg_operation(o).filter(|n| ir.op(*n).inst == inst)
}

pub fn run_block(
    ir: &mut Ir,
    blocks: &mut [Block],
    bi: usize,
    env: &mut Env,
    defer_several: bool,
) -> R<()> {
    let mut i = 0;
    while i < blocks[bi].ops.len() {
        let node = blocks[bi].ops[i];
        let next = blocks[bi].ops.get(i + 1).copied();
        let Some(t) = tex::tex(ir, node) else {
            i += 1;
            continue;
        };
        if t.flags & BINDLESS == 0 {
            i += 1;
            continue;
        }
        process(ir, blocks, bi, node, env, defer_several)?;
        i = match next {
            Some(nx) => blocks[bi]
                .ops
                .iter()
                .position(|x| *x == nx)
                .ok_or("next node vanished")?,
            None => blocks[bi].ops.len(),
        };
    }
    Ok(())
}

fn pos_of(blocks: &[Block], bi: usize, node: NodeId) -> R<usize> {
    blocks[bi]
        .ops
        .iter()
        .position(|x| *x == node)
        .ok_or_else(|| "node not in block".to_string())
}

fn process(
    ir: &mut Ir,
    blocks: &mut [Block],
    bi: usize,
    node: NodeId,
    env: &mut Env,
    defer_several: bool,
) -> R<()> {
    if try_convert_bindless(ir, blocks, bi, env, node)? {
        return Ok(());
    }
    let table = try_table_handle(ir, blocks, bi, env, node, defer_several)?;
    if table == Table::Deferred {
        return Ok(());
    }
    let converted = table == Table::Set && try_convert_bindless(ir, blocks, bi, env, node)?;
    let t = tex::tex(ir, node).ok_or("tex")?.clone();
    let inst = ir.op(node).inst;
    if !converted && tex::masked(inst) == Inst::ImageAtomic && t.flags & ATOMIC_METAL != 0 {
        return Err("Metal image atomics require a statically resolved image handle.".into());
    }
    if !converted && !generate_bindless_access(ir, blocks, bi, env, node)? {
        // the note names the GLSL type of the operation: not ported (the corpus never gets here)
        return Err("unported: bindless access without a resolvable handle".into());
        #[allow(unreachable_code)]
        let dests = ir.op(node).dests.clone();
        let mut pos = pos_of(blocks, bi, node)?;
        for d in dests {
            let c0 = ir.const_(0);
            let cp = ir.new_op(Inst::Copy, StorageKind::None, d, &[c0]);
            blocks[bi].ops.insert(pos, cp);
            pos += 1;
        }
        utils::delete_node(ir, &mut blocks[bi], pos);
    }
    Ok(())
}

fn phi_leaves(ir: &Ir, operand: Opd) -> Vec<Opd> {
    let mut leaves = vec![];
    let mut visited: HashSet<NodeId> = HashSet::new();
    let mut pending = vec![operand];
    while let Some(cur) = pending.pop() {
        if let Some(phi) = asg_phi(ir, cur) {
            if visited.insert(phi) {
                for src in ir.node_sources(phi).into_iter().flatten() {
                    pending.push(src);
                }
            }
        } else {
            leaves.push(cur);
        }
    }
    leaves
}

/// A c[2] word that can name a texture (a sampler word alone, on a path the read does not take, is no candidate).
fn is_table_word(ir: &Ir, o: Opd) -> bool {
    is_c2_word(ir, o) && !is_sampler_word(ir, o)
}

fn is_c2_word(ir: &Ir, o: Opd) -> bool {
    ir.ty(o) == OperandType::ConstantBuffer && ir.cbuf_slot(o) == NVN_TEXTURE_BUFFER_INDEX
}

pub(super) fn is_sampler_word(ir: &Ir, o: Opd) -> bool {
    if !is_c2_word(ir, o) {
        return false;
    }
    let offset = ir.cbuf_offset(o) * 4;
    (NVN_SEPARATE_SAMPLER_BASE..NVN_SEPARATE_SAMPLER_END).contains(&offset)
}

fn add_candidate(ir: &Ir, found: &mut Vec<(Opd, Option<Opd>)>, texture: Opd, sampler: Option<Opd>) {
    for (t, sm) in found.iter() {
        let same = ir.value(*t) == ir.value(texture)
            && match (sm, sampler) {
                (None, None) => true,
                (Some(a), Some(b)) => ir.ty(*a) == ir.ty(b) && ir.value(*a) == ir.value(b),
                _ => false,
            };
        if same {
            return;
        }
    }
    found.push((texture, sampler));
}

fn same_shape(a: SamplerType, b: SamplerType) -> bool {
    let shape = SamplerType::MASK | SamplerType::ARRAY | SamplerType::MULTISAMPLE;
    let of = |t: SamplerType| {
        let v = t.0 & shape;
        if v == SamplerType::TEXTURE_BUFFER.0 {
            SamplerType::TEXTURE_1D.0
        } else {
            v
        }
    };
    of(a) == of(b)
}

fn candidate_handle(
    ir: &mut Ir,
    blocks: &mut [Block],
    bi: usize,
    node: NodeId,
    cand: (Opd, Option<Opd>),
) -> R<Opd> {
    let Some(sampler) = cand.1 else {
        return Ok(cand.0);
    };
    let combined = ir.local();
    let or = ir.new_op(
        Inst::BitwiseOr,
        StorageKind::None,
        Some(combined),
        &[cand.0, sampler],
    );
    let pos = pos_of(blocks, bi, node)?;
    blocks[bi].ops.insert(pos, or);
    Ok(combined)
}

fn try_table_handle(
    ir: &mut Ir,
    blocks: &mut [Block],
    bi: usize,
    env: &mut Env,
    node: NodeId,
    defer_several: bool,
) -> R<Table> {
    let handle = s(ir, node, 0)?;
    let inst = ir.op(node).inst;
    if asg_phi(ir, handle).is_none() || is_bindless_handle_allowed(ir, handle) {
        return Ok(Table::None);
    }
    let mut found: Vec<(Opd, Option<Opd>)> = vec![];
    for leaf in phi_leaves(ir, handle) {
        if is_table_word(ir, leaf) {
            add_candidate(ir, &mut found, leaf, None);
        } else if !tex::is_image(inst)
            && let Some(or_op) = op_of(ir, leaf, Inst::BitwiseOr)
        {
            let (o0, o1) = (s(ir, or_op, 0)?, s(ir, or_op, 1)?);
            for texture in phi_leaves(ir, o0) {
                for sampler in phi_leaves(ir, o1) {
                    // either order, as in bindless_convert
                    let (t, sm) = if is_sampler_word(ir, texture) && !is_sampler_word(ir, sampler) {
                        (sampler, texture)
                    } else {
                        (texture, sampler)
                    };
                    if is_table_word(ir, t)
                        && (is_c2_word(ir, sm)
                            || (ir.ty(sm) == OperandType::Constant
                                && (ir.value(sm) & 0xfffff) == 0))
                    {
                        add_candidate(ir, &mut found, t, Some(sm));
                    }
                }
            }
        } else if op_of(ir, leaf, Inst::Load).is_some() {
            return Ok(Table::None);
        }
    }
    if found.len() > 1 {
        if tex::is_image_store(inst) {
            return Ok(Table::None);
        }
        if defer_several {
            return Ok(Table::Deferred);
        }
        let op_ty = tex::tex(ir, node).ok_or("tex")?.ty;
        let is_image = tex::is_image(inst);
        let mut keep = vec![];
        for c in &found {
            let types =
                env.rm
                    .get_texture_types(NVN_TEXTURE_BUFFER_INDEX, ir.cbuf_offset(c.0), is_image);
            if types.iter().all(|t| same_shape(*t, op_ty)) {
                keep.push(*c);
            }
        }
        found = keep;
    }
    if found.is_empty() {
        return Ok(Table::None);
    }
    let last = found.len() - 1;
    let mut matches: Vec<Opd> = vec![];
    let mut results: Vec<Vec<Opd>> = vec![];
    let t0 = tex::tex(ir, node).ok_or("tex")?.clone();
    let n_src = ir.op(node).sources.len();
    let n_dst = ir.op(node).dests.len();
    for &cand in found.iter().take(last) {
        let candidate = candidate_handle(ir, blocks, bi, node, cand)?;
        let mut sources = Vec::with_capacity(n_src);
        for si in 0..n_src {
            sources.push(if si == 0 { candidate } else { s(ir, node, si)? });
        }
        let res: Vec<Opd> = (0..n_dst).map(|_| ir.local()).collect();
        let dests: Vec<Option<Opd>> = res.iter().map(|r| Some(*r)).collect();
        let read = ir.new_tex_op(
            ir.op(node).inst,
            t0.ty,
            t0.format,
            t0.flags,
            t0.set,
            t0.binding,
            ir.op(node).index,
            &dests,
            &sources,
        );
        let mut pos = pos_of(blocks, bi, node)?;
        blocks[bi].ops.insert(pos, read);
        let mut read_pos = pos;
        pos += 1;
        let _ = pos;
        if !try_convert_bindless(ir, blocks, bi, env, read)? {
            for r in &res {
                let c0 = ir.const_(0);
                let cp = ir.new_op(Inst::Copy, StorageKind::None, Some(*r), &[c0]);
                blocks[bi].ops.insert(read_pos, cp);
                read_pos += 1;
            }
            utils::delete_node(ir, &mut blocks[bi], read_pos);
        }
        let m = ir.local();
        let cmp = ir.new_op(
            Inst::CompareEqual,
            StorageKind::None,
            Some(m),
            &[handle, candidate],
        );
        let pos = pos_of(blocks, bi, node)?;
        blocks[bi].ops.insert(pos, cmp);
        matches.push(m);
        results.push(res);
    }
    let ch = candidate_handle(ir, blocks, bi, node, found[last])?;
    ir.set_source(node, 0, Some(ch));
    let mut after = pos_of(blocks, bi, node)?;
    let mut di = 0;
    while di < ir.op(node).dests.len() && last > 0 {
        let dest = ir.op(node).dests[di];
        let mut value = ir.local();
        ir.set_dest(node, di, Some(value));
        for index in (0..last).rev() {
            let selected = if index == 0 {
                dest.ok_or("null destination")?
            } else {
                ir.local()
            };
            let sel = ir.new_op(
                Inst::ConditionalSelect,
                StorageKind::None,
                Some(selected),
                &[matches[index], results[index][di], value],
            );
            after += 1;
            blocks[bi].ops.insert(after, sel);
            value = selected;
        }
        di += 1;
    }
    env.acc.log(&if found.len() == 1 {
        "Bindless handle taken from the one c[2] handle among its phi inputs.".to_string()
    } else {
        format!(
            "Bindless handle chosen at run time among {} c[2] handles of its phi inputs.",
            found.len()
        )
    });
    Ok(Table::Set)
}

fn generate_bindless_access(
    ir: &mut Ir,
    blocks: &mut [Block],
    bi: usize,
    env: &mut Env,
    node: NodeId,
) -> R<bool> {
    if !crate::translation::host::SUPPORTS_SEPARATE_SAMPLER {
        return Ok(false);
    }
    let handle = s(ir, node, 0)?;
    if !is_bindless_handle_allowed(ir, handle) {
        return Ok(false);
    }
    let t = tex::tex(ir, node).ok_or("tex")?.clone();
    let inst = ir.op(node).inst;
    let texture_handle = ir.local();
    let sampler_handle = ir.local();
    let texture_index = ir.local();
    let mut pos = pos_of(blocks, bi, node)?;
    let c = ir.const_(0xfffff);
    let n1 = ir.new_op(
        Inst::BitwiseAnd,
        StorageKind::None,
        Some(texture_handle),
        &[handle, c],
    );
    let c = ir.const_(20);
    let n2 = ir.new_op(
        Inst::ShiftRightU32,
        StorageKind::None,
        Some(sampler_handle),
        &[handle, c],
    );
    let pooled =
        !tex::is_image(inst) && (t.ty.0 & SamplerType::MASK) != SamplerType::TEXTURE_BUFFER.0;
    let tex_pool_len = if pooled {
        MINIMUM_ARRAY_LENGTH.max(env.acc.query_texture_array_length_from_pool())
    } else {
        NVN_SLOT_ARRAY_LENGTH
    };
    let c = ir.const_(tex_pool_len - 1);
    let n3 = ir.new_op(
        Inst::MinimumU32,
        StorageKind::None,
        Some(texture_index),
        &[texture_handle, c],
    );
    for n in [n1, n2, n3] {
        blocks[bi].ops.insert(pos, n);
        pos += 1;
    }
    ir.set_source(node, 0, Some(texture_index));
    let has_sampler = !tex::is_image(inst);
    let direct = tex::pack_offsets(0, 0, tex::DIRECT);
    let tex_sb = env.rm.get_texture_or_image_binding(
        env.acc,
        inst,
        t.ty,
        t.format,
        TextureFlags(t.flags & !BINDLESS),
        0,
        direct,
        tex_pool_len,
        has_sampler,
    )?;
    if has_sampler {
        let sampler_index = ir.local();
        let sampler_pool_len = if pooled {
            MINIMUM_ARRAY_LENGTH.max(env.acc.query_sampler_array_length_from_pool())
        } else {
            NVN_SLOT_ARRAY_LENGTH
        };
        let c = ir.const_(sampler_pool_len - 1);
        let n4 = ir.new_op(
            Inst::MinimumU32,
            StorageKind::None,
            Some(sampler_index),
            &[sampler_handle, c],
        );
        blocks[bi].ops.insert(pos, n4);
        ir.insert_source(node, 1, sampler_index);
        let sampler_sb = env.rm.get_texture_or_image_binding(
            env.acc,
            inst,
            SamplerType::NONE,
            t.format,
            TextureFlags::NONE,
            0,
            direct,
            sampler_pool_len,
            false,
        )?;
        tex::turn_into_array(ir, node, tex_sb)?;
        let td = tex::tex_mut(ir, node)?;
        td.sampler_set = sampler_sb.set;
        td.sampler_binding = sampler_sb.binding;
    } else {
        tex::turn_into_array(ir, node, tex_sb)?;
    }
    Ok(true)
}

fn is_bindless_handle_allowed(ir: &Ir, h: Opd) -> bool {
    if let Some(phi) = asg_phi(ir, h) {
        for src in ir.node_sources(phi).into_iter().flatten() {
            if asg_phi(ir, src).is_none() && !is_bindless_access_allowed(ir, src) {
                return false;
            }
        }
        return true;
    }
    is_bindless_access_allowed(ir, h)
}

fn is_bindless_access_allowed(ir: &Ir, h: Opd) -> bool {
    if ir.ty(h) == OperandType::ConstantBuffer {
        return true;
    }
    match ir.asg_operation(h) {
        Some(op) => {
            let o = ir.op(op);
            o.inst == Inst::Load
                && (o.storage_kind == StorageKind::Input
                    || o.storage_kind == StorageKind::StorageBuffer)
        }
        None => false,
    }
}
