//! `GlobalToStorage.TryGenerateRawStorageOp`: a global memory access traced to no storage buffer goes through the
//! host's memory pool table.
use super::Env;
use super::gts_fn::{
    bit_offset, finish_function, function_name, inst_name, is_atomic, is_small_int,
};
use crate::ir::{Inst, Opd, StorageKind};
use crate::xform::helper::{B, Function};

type R<T> = Result<T, String>;

/// `GmemMapCbSlot`: the sentinel constant buffer slot of the memory pool table and the write log.
const GMEM_MAP_CB_SLOT: i32 = 0xff;
const GMEM_MAP_LOG2: i32 = 13;
const GMEM_MAP_ENTRY_WORDS: i32 = 8;
const GMEM_HINT_SHIFT: i32 = 21;
const GMEM_LOG_CB_OFFSET: i32 = 1;

fn map_word(b: &mut B, binding: i32, entry: Opd, word: i32) -> Opd {
    let (c_b, c0) = (b.ir.const_(binding), b.ir.const_(0));
    let w = b.ir.const_(word);
    let idx = b.op(Inst::Add, &[entry, w]);
    b.op_kind(Inst::Load, StorageKind::StorageBuffer, &[c_b, c0, idx])
}

/// `TryGenerateRawStorageOp`: `Ok(None)` when a storage buffer cannot be reserved. Sets `UsesPhysicalPointers`.
pub(super) fn raw_target(
    ir: &mut crate::ir::Ir,
    env: &mut Env,
    inst: Inst,
    kind: StorageKind,
    returns_value: bool,
) -> R<Option<Function>> {
    let small = is_small_int(kind);
    if small && is_atomic(inst) {
        let m = format!(
            "Global memory operation \"{}\" on a small integer traced to no storage buffer: not implemented.",
            inst_name(inst)?
        );
        env.acc.log(&m);
        return Ok(None);
    }
    let Some(map_binding) =
        env.rm
            .try_get_storage_buffer_binding(env.acc, GMEM_MAP_CB_SLOT, 0, false)
    else {
        return Ok(None);
    };
    let writes = inst == Inst::Store || is_atomic(inst);
    let mut log_binding = 0;
    if writes {
        let Some(l) = env.rm.try_get_storage_buffer_binding(
            env.acc,
            GMEM_MAP_CB_SLOT,
            GMEM_LOG_CB_OFFSET,
            true,
        ) else {
            return Ok(None);
        };
        log_binding = l;
    }
    env.rm.properties.uses_physical_pointers = true;
    let in_args = if inst == Inst::AtomicCompareAndSwap {
        4
    } else if writes {
        3
    } else {
        2
    };
    let mut b = B::new(ir);
    let addr_low = b.ir.argument(0);
    let addr_high = b.ir.argument(1);
    // The hint of the address's 2 MiB chunk names an entry holding the whole chunk. An address too high for the tag
    // searches.
    let c_up = b.ir.const_(32 - GMEM_HINT_SHIFT);
    let chunk_hi = b.op(Inst::ShiftLeft, &[addr_high, c_up]);
    let c_sh = b.ir.const_(GMEM_HINT_SHIFT);
    let chunk_lo = b.op(Inst::ShiftRightU32, &[addr_low, c_sh]);
    let chunk = b.op(Inst::BitwiseOr, &[chunk_hi, chunk_lo]);
    let index_mask = b.ir.const_((1 << GMEM_MAP_LOG2) - 1);
    let chunk_index = b.op(Inst::BitwiseAnd, &[chunk, index_mask]);
    let c_words = b.ir.const_(GMEM_MAP_ENTRY_WORDS);
    let hint_entry = b.op(Inst::Multiply, &[chunk_index, c_words]);
    let hint = map_word(&mut b, map_binding, hint_entry, 6);
    let hint_index = b.op(Inst::BitwiseAnd, &[hint, index_mask]);
    let c_zero = b.ir.const_(0);
    let used = b.op(Inst::CompareNotEqual, &[hint_index, c_zero]);
    let c_log2 = b.ir.const_(GMEM_MAP_LOG2);
    let hint_chunk = b.op(Inst::ShiftRightU32, &[hint, c_log2]);
    let same_chunk = b.op(Inst::CompareEqual, &[hint_chunk, chunk]);
    let named = b.op(Inst::BitwiseAnd, &[used, same_chunk]);
    let c_limit = b.ir.const_(1 << (GMEM_HINT_SHIFT - GMEM_MAP_LOG2));
    let low_enough = b.op(Inst::CompareLessU32, &[addr_high, c_limit]);
    let hint_fits = b.op(Inst::BitwiseAnd, &[named, low_enough]);
    let selected = b.ir.local();
    let found_label = b.ir.label();
    b.raw(Inst::Copy, StorageKind::None, Some(selected), &[hint_index]);
    b.branch_if_true(found_label, hint_fits);
    // The last entry whose start is at or below the address (entry 0 starts at 0, so there always is one)
    let mut index = b.ir.const_(0);
    let mut step = 1 << (GMEM_MAP_LOG2 - 1);
    while step != 0 {
        let cs = b.ir.const_(step);
        let candidate = b.op(Inst::Add, &[index, cs]);
        let cw = b.ir.const_(GMEM_MAP_ENTRY_WORDS);
        let entry = b.op(Inst::Multiply, &[candidate, cw]);
        let start_low = map_word(&mut b, map_binding, entry, 0);
        let start_high = map_word(&mut b, map_binding, entry, 1);
        let lt = b.op(Inst::CompareLessU32, &[start_high, addr_high]);
        let eq = b.op(Inst::CompareEqual, &[start_high, addr_high]);
        let le = b.op(Inst::CompareLessOrEqualU32, &[start_low, addr_low]);
        let and = b.op(Inst::BitwiseAnd, &[eq, le]);
        let at_or_below = b.op(Inst::BitwiseOr, &[lt, and]);
        index = b.op(Inst::ConditionalSelect, &[at_or_below, candidate, index]);
        step >>= 1;
    }
    b.raw(Inst::Copy, StorageKind::None, Some(selected), &[index]);
    b.mark_label(found_label);
    let cw = b.ir.const_(GMEM_MAP_ENTRY_WORDS);
    let found = b.op(Inst::Multiply, &[selected, cw]);
    let end_low = map_word(&mut b, map_binding, found, 2);
    let end_high = map_word(&mut b, map_binding, found, 3);
    let lt = b.op(Inst::CompareLessU32, &[addr_high, end_high]);
    let eq = b.op(Inst::CompareEqual, &[addr_high, end_high]);
    let lt2 = b.op(Inst::CompareLessU32, &[addr_low, end_low]);
    let and = b.op(Inst::BitwiseAnd, &[eq, lt2]);
    let in_pool = b.op(Inst::BitwiseOr, &[lt, and]);
    let skip = b.ir.label();
    b.branch_if_false(skip, in_pool);
    if writes {
        // The write log is a bitmap of the table entries the stores and atomics wrote through. A bit is set once, so a
        // store usually costs a load and no atomic.
        let c5 = b.ir.const_(5);
        let word = b.op(Inst::ShiftRightU32, &[selected, c5]);
        let c1 = b.ir.const_(1);
        let c31 = b.ir.const_(31);
        let low5 = b.op(Inst::BitwiseAnd, &[selected, c31]);
        let bit = b.op(Inst::ShiftLeft, &[c1, low5]);
        let logged = b.ir.label();
        let (c_b, c0) = (b.ir.const_(log_binding), b.ir.const_(0));
        let old = b.op_kind(Inst::Load, StorageKind::StorageBuffer, &[c_b, c0, word]);
        let seen = b.op(Inst::BitwiseAnd, &[old, bit]);
        let z = b.ir.const_(0);
        let is_set = b.op(Inst::CompareNotEqual, &[seen, z]);
        b.branch_if_true(logged, is_set);
        let (c_b, c0) = (b.ir.const_(log_binding), b.ir.const_(0));
        b.op_kind(
            Inst::AtomicOr,
            StorageKind::StorageBuffer,
            &[c_b, c0, word, bit],
        );
        b.mark_label(logged);
    }
    let word_low = if small {
        let m = b.ir.const_(!3);
        b.op(Inst::BitwiseAnd, &[addr_low, m])
    } else {
        addr_low
    };
    let delta_low = map_word(&mut b, map_binding, found, 4);
    let host_low = b.op(Inst::Add, &[word_low, delta_low]);
    let wrapped = b.op(Inst::CompareLessU32, &[host_low, word_low]);
    let (k1, k0) = (b.ir.const_(1), b.ir.const_(0));
    let carry = b.op(Inst::ConditionalSelect, &[wrapped, k1, k0]);
    let delta_high = map_word(&mut b, map_binding, found, 5);
    let sum = b.op(Inst::Add, &[addr_high, delta_high]);
    let host_high = b.op(Inst::Add, &[sum, carry]);
    let mut result = None;
    if inst == Inst::Load {
        let mut v = b.op_kind(
            Inst::Load,
            StorageKind::GlobalMemory,
            &[host_low, host_high],
        );
        if small {
            let bo = bit_offset(&mut b, addr_low);
            v = match kind {
                StorageKind::GlobalMemoryS8 | StorageKind::GlobalMemoryS16 => {
                    let sh = b.op(Inst::ShiftRightS32, &[v, bo]);
                    let z = b.ir.const_(0);
                    let w = b.ir.const_(if kind == StorageKind::GlobalMemoryS8 {
                        8
                    } else {
                        16
                    });
                    b.op(Inst::BitfieldExtractS32, &[sh, z, w])
                }
                _ => {
                    let sh = b.op(Inst::ShiftRightU32, &[v, bo]);
                    let m = b.ir.const_(if kind == StorageKind::GlobalMemoryU8 {
                        0xff
                    } else {
                        0xffff
                    });
                    b.op(Inst::BitwiseAnd, &[sh, m])
                }
            };
        }
        result = Some(v);
    } else if inst == Inst::Store {
        let value = b.ir.argument(2);
        if small {
            let bit_size = if matches!(
                kind,
                StorageKind::GlobalMemoryS8 | StorageKind::GlobalMemoryU8
            ) {
                8
            } else {
                16
            };
            let bo = bit_offset(&mut b, addr_low);
            let head = b.ir.label();
            b.mark_label(head);
            let old = b.op_kind(
                Inst::Load,
                StorageKind::GlobalMemory,
                &[host_low, host_high],
            );
            let size = b.ir.const_(bit_size);
            let new = b.op(Inst::BitfieldInsert, &[old, value, bo, size]);
            let cas = b.op_kind(
                Inst::AtomicCompareAndSwap,
                StorageKind::GlobalMemory,
                &[host_low, host_high, old, new],
            );
            let fail = b.op(Inst::CompareNotEqual, &[cas, old]);
            b.branch_if_true(head, fail);
        } else {
            b.raw(
                Inst::Store,
                StorageKind::GlobalMemory,
                None,
                &[host_low, host_high, value],
            );
        }
    } else if inst == Inst::AtomicCompareAndSwap {
        let (a2, a3) = (b.ir.argument(2), b.ir.argument(3));
        result = Some(b.op_kind(
            Inst::AtomicCompareAndSwap,
            StorageKind::GlobalMemory,
            &[host_low, host_high, a2, a3],
        ));
    } else {
        let a2 = b.ir.argument(2);
        result = Some(b.op_kind(inst, StorageKind::GlobalMemory, &[host_low, host_high, a2]));
    }
    b.ret(result);
    b.mark_label(skip);
    if returns_value {
        let z = b.ir.const_(0);
        b.ret(Some(z));
    } else {
        b.ret(None);
    }
    let name = format!("{}Raw", function_name(inst, kind, true, &[])?);
    Ok(Some(finish_function(b, &name, returns_value, in_args)?))
}
