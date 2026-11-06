//! Attributes, global, local and shared memory of `InstEmitAmpereMem.cs`.

use super::operand::*;
use crate::emit::mem::attr::{emit_ald, emit_ast, emit_ipa};
use crate::emit::mem::memory::{
    emit_atomic_op, emit_load, emit_store, get_storage_kind, get_vector_count,
};
use crate::ir::Opd;
use crate::ir::inst::StorageKind;
use crate::ir::ops::regtype;
use crate::maxwell::ampere::{AmpOp, opc};
use crate::maxwell::enums::*;

fn c(ctx: &mut Ctx, v: i32) -> Opd {
    ctx.ir.const_(v)
}

pub(super) fn ipa(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let addr = x.bits(64, 72) << 2;
    if x.bits(76, 78) == 2 {
        return Err(fail(ctx, "IPA at an offset"));
    }
    let mut idx_address = None;
    if x.opcode() == opc::IPA_IDX {
        let u = ctx.ir.register(x.bits(24, 30), regtype::UNIFORM);
        let k = c(ctx, addr);
        idx_address = Some(ctx.i_add(u, k));
    }
    let ipa_op = if x.bits(78, 80) == 1 {
        IpaOp::Constant
    } else {
        IpaOp::Pass
    };
    emit_ipa(ctx, idx_address, addr, ipa_op, RZ, false, x.rd())
}

pub(super) fn ald(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    if !x.bit(77) && x.ra() != RZ {
        return Err(fail(ctx, "ALD with an offset register and an address"));
    }
    emit_ald(
        ctx,
        x.rd(),
        x.ra(),
        x.rb(),
        x.bits(40, 50),
        x.bits(74, 76),
        x.bit(76),
        x.bit(79),
        x.bit(77),
    )
}

pub(super) fn ast(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    if !x.bit(77) && x.ra() != RZ {
        return Err(fail(ctx, "AST with an offset register and an address"));
    }
    emit_ast(
        ctx,
        x.ra(),
        x.rb(),
        x.bits(40, 50),
        x.bits(74, 76),
        x.bit(76),
        x.bit(77),
    )
}

pub(super) fn isberd(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    if let Some(stage) = ctx.acc.query_ampere_stage_memory().map(<[i32]>::to_vec) {
        // A task or mesh shader's stage input: mode 0x78 the workgroup's number, 0x38 a word of the payload its task
        // wrote and 0xb8 a word it wrote itself
        let mode = x.bits(72, 80);
        if !matches!(mode, 0x78 | 0x38 | 0xb8) {
            return Err(fail(
                ctx,
                &format!("ISBERD mode {mode:02x} in a task or mesh shader"),
            ));
        }
        let addr = if mode == 0x78 {
            c(ctx, stage_word(&stage, 0))
        } else {
            let a = gpr(ctx, x.ra());
            let k = c(
                ctx,
                stage_word(&stage, if mode == 0xb8 { 2 } else { 1 })
                    + x.signed_field(40, 64) as i32,
            );
            ctx.i_add(a, k)
        };
        return emit_load(
            ctx,
            StorageKind::SharedMemory,
            LsSize2::B32,
            addr,
            x.rd(),
            0,
        );
    }
    if x.rd() < RZ {
        let d = gpr(ctx, x.rd());
        let a = gpr(ctx, x.ra());
        ctx.copy_to(d, a);
    }
    Ok(())
}

fn stage_word(stage: &[i32], i: usize) -> i32 {
    stage.get(i).copied().unwrap_or(0)
}

/// A task or mesh shader's stage output (0x927), `[Ra + offset], Rb`, mode at 72..80: 0xb8 a word of the task's payload,
/// 0xf8 a word of a mesh shader's attributes, 0x88 the primitive count byte and 0x80 a primitive index byte. Bytes go
/// through the shared store of a small integer. The zero count every thread writes first (mode 0x88 from RZ) is left out
/// because the stage memory starts at zero.
pub(super) fn stage_out(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let stage = ctx.acc.query_ampere_stage_memory().map(<[i32]>::to_vec);
    let mode = x.bits(72, 80);
    let Some(stage) = stage.filter(|_| matches!(mode, 0xb8 | 0xf8 | 0x88 | 0x80)) else {
        return Err(fail(
            ctx,
            &format!("stage output store mode {mode:02x} outside a task or mesh shader"),
        ));
    };
    if mode == 0x88 && x.rb() == RZ {
        return Ok(());
    }
    let word = mode == 0xb8 || mode == 0xf8;
    let a = gpr(ctx, x.ra());
    let k = c(
        ctx,
        stage_word(&stage, if word { 2 } else { 3 }) + x.signed_field(40, 64) as i32,
    );
    let addr = ctx.i_add(a, k);
    let size = if word { LsSize2::B32 } else { LsSize2::U8 };
    emit_store(ctx, StorageKind::SharedMemory, size, addr, x.rb(), 0)
}

pub(super) fn lepc(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    if x.rd() < RZ {
        let d = gpr(ctx, x.rd());
        let a = c(ctx, ctx.curr_op.address as i32);
        ctx.copy_to(d, a);
    }
    if x.rd() + 1 < RZ {
        let d = gpr(ctx, x.rd() + 1);
        let z = c(ctx, 0);
        ctx.copy_to(d, z);
    }
    Ok(())
}

/// `AGlobalAddress`: the 64-bit address of a global access.
fn global_address(
    ctx: &mut Ctx,
    x: &AmpOp,
    ra_pair_bit: u32,
    ur_lo: u32,
    extra_offset: i32,
) -> (Opd, Opd) {
    let uniform_form = x.bit(91);
    let ra_pair = if uniform_form {
        x.bit(ra_pair_bit)
    } else {
        x.bit(72)
    };
    let offset = x.signed_field(40, 64) as i32 + extra_offset;
    let mut lo = gpr(ctx, x.ra());
    let mut hi = if ra_pair && x.ra() != RZ {
        gpr(ctx, x.ra() + 1)
    } else {
        c(ctx, 0)
    };
    if uniform_form {
        let ur = x.bits(ur_lo, ur_lo + 6);
        if ur != URZ {
            let u_lo = ctx.ir.register(ur, regtype::UNIFORM);
            let u_hi = if x.bit(72) {
                ctx.ir.register(ur + 1, regtype::UNIFORM)
            } else {
                c(ctx, 0)
            };
            let sum = ctx.i_add(u_lo, lo);
            let lt = ctx.i_compare_less_unsigned(sum, u_lo);
            let one = c(ctx, 1);
            let zero = c(ctx, 0);
            let carry = ctx.conditional_select(lt, one, zero);
            let h1 = ctx.i_add(u_hi, hi);
            hi = ctx.i_add(h1, carry);
            lo = sum;
        }
    }
    if offset != 0 {
        let k = c(ctx, offset);
        let sum = ctx.i_add(lo, k);
        let carry = if offset > 0 {
            let lt = ctx.i_compare_less_unsigned(sum, lo);
            let one = c(ctx, 1);
            let zero = c(ctx, 0);
            ctx.conditional_select(lt, one, zero)
        } else {
            let gt = ctx.i_compare_greater_unsigned(sum, lo);
            let m1 = c(ctx, -1);
            let zero = c(ctx, 0);
            ctx.conditional_select(gt, m1, zero)
        };
        hi = ctx.i_add(hi, carry);
        lo = sum;
    }
    (lo, hi)
}

pub(super) fn ldg(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let size = LsSize(x.bits(73, 76));
    let count = get_vector_count(size);
    let kind = get_storage_kind(size);
    let mut pred_op = None;
    if x.opcode() == 0x981 {
        let p = 7 - x.bits(64, 67);
        if p != PT || x.bit(67) {
            let pr = ctx.ir.register(p, regtype::PREDICATE);
            pred_op = Some(if x.bit(67) { ctx.bitwise_not(pr) } else { pr });
        }
    }
    // The address is read before any result is written, the zeros of a false predicate too: the destination may be the
    // address register (LDG.128 R0, [R0.64])
    let mut addrs = Vec::new();
    for index in 0..count {
        let (lo, hi) = global_address(ctx, &x, 90, 32, index * 4);
        addrs.push((ctx.copy(lo), ctx.copy(hi)));
    }
    let mut skip = None;
    if let Some(p) = pred_op {
        let mut index = 0;
        while index < count && x.rd() + index < RZ {
            let d = gpr(ctx, x.rd() + index);
            let z = c(ctx, 0);
            ctx.copy_to(d, z);
            index += 1;
        }
        let l = ctx.ir.label();
        skip = Some(l);
        ctx.branch_if_false(l, p);
    }
    for (index, (lo, hi)) in addrs.into_iter().enumerate() {
        let index = index as i32;
        if x.rd() + index >= RZ {
            break;
        }
        let v = ctx.load_o2(kind, lo, hi);
        let d = gpr(ctx, x.rd() + index);
        ctx.copy_to(d, v);
    }
    if let Some(l) = skip {
        ctx.mark_label(l);
    }
    Ok(())
}

pub(super) fn stg(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let size = LsSize(x.bits(73, 76));
    if size.0 > LsSize::B128.0 {
        return Err(fail(ctx, &format!("STG size {}", size.0)));
    }
    let count = get_vector_count(size);
    let kind = get_storage_kind(size);
    for index in 0..count {
        let is_rz = x.rb() + index >= RZ;
        let value = if is_rz {
            c(ctx, 0)
        } else {
            gpr(ctx, x.rb() + index)
        };
        let (lo, hi) = global_address(ctx, &x, 90, 64, index * 4);
        ctx.store_o3(kind, lo, hi, value);
    }
    Ok(())
}

pub(super) fn red(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let (lo, hi) = global_address(ctx, &x, 90, 64, 0);
    let rb = gpr(ctx, x.rb());
    emit_atomic_op(
        ctx,
        StorageKind::GlobalMemory,
        AtomOp(x.bits(87, 90)),
        AtomSize(x.bits(73, 76)),
        lo,
        hi,
        rb,
    );
    Ok(())
}

pub(super) fn atomg(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let (lo, hi) = global_address(ctx, &x, 70, 64, 0);
    let rb = gpr(ctx, x.rb());
    let res = emit_atomic_op(
        ctx,
        StorageKind::GlobalMemory,
        AtomOp(x.bits(87, 91)),
        AtomSize(x.bits(73, 76)),
        lo,
        hi,
        rb,
    );
    set(ctx, &x, res, false);
    Ok(())
}

pub(super) fn membar(ctx: &mut Ctx) -> R {
    ctx.memory_barrier();
    Ok(())
}

pub(super) fn bar(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    if x.bits(77, 80) != 0 && x.bits(77, 80) != 4 {
        return Err(fail(ctx, &format!("BAR mode {}", x.bits(77, 80))));
    }
    ctx.barrier();
    Ok(())
}

/// `AStride`.
fn stride(ctx: &mut Ctx, x: &AmpOp, ra: Opd) -> Opd {
    let s = x.bits(78, 80);
    if s == 0 {
        ra
    } else {
        let k = c(ctx, s + 1);
        ctx.shift_left(ra, k)
    }
}

/// `ALocalAddress`.
pub(super) fn local_address(ctx: &mut Ctx, x: &AmpOp, ur_lo: u32, use_stride: bool) -> Opd {
    let ra = gpr(ctx, x.ra());
    let mut addr = if x.ra() == RZ {
        c(ctx, 0)
    } else if use_stride {
        stride(ctx, x, ra)
    } else {
        ra
    };
    if x.bit(91) && x.bits(ur_lo, ur_lo + 6) != URZ {
        let u = ctx.ir.register(x.bits(ur_lo, ur_lo + 6), regtype::UNIFORM);
        addr = ctx.i_add(addr, u);
    }
    addr
}

pub(super) fn ld_local_shared(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let kind = if x.opcode() == opc::LDL {
        StorageKind::LocalMemory
    } else {
        StorageKind::SharedMemory
    };
    let addr = local_address(ctx, &x, 32, true);
    emit_load(
        ctx,
        kind,
        LsSize2(x.bits(73, 76)),
        addr,
        x.rd(),
        x.signed_field(40, 64) as i32,
    )
}

pub(super) fn st_local_shared(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let kind = if matches!(x.opcode(), 0x387 | 0x987) {
        StorageKind::LocalMemory
    } else {
        StorageKind::SharedMemory
    };
    let addr = local_address(ctx, &x, 64, true);
    emit_store(
        ctx,
        kind,
        LsSize2(x.bits(73, 76)),
        addr,
        x.rb(),
        x.signed_field(40, 64) as i32,
    )
}

pub(super) fn atoms(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let one = x.opcode() == 0xf8c;
    if if one {
        x.bits(87, 91) != 10 || !x.bit(91) || x.rd() != RZ
    } else {
        x.bit(91)
    } {
        return Err(fail(ctx, "ATOMS with a uniform register"));
    }
    let la = local_address(ctx, &x, 64, true);
    let k = c(ctx, x.signed_field(40, 64) as i32);
    let sum = ctx.i_add(la, k);
    let two = c(ctx, 2);
    let offset = ctx.shift_right_u32(sum, two);
    let id = c(ctx, ctx.rm.shared_memory_id);
    if one {
        let one_v = c(ctx, 1);
        emit_atomic_op(
            ctx,
            StorageKind::SharedMemory,
            AtomOp::Add,
            AtomSize(x.bits(73, 76)),
            id,
            offset,
            one_v,
        );
        return Ok(());
    }
    let value = if x.rb() >= RZ {
        c(ctx, 0)
    } else {
        gpr(ctx, x.rb())
    };
    let res = emit_atomic_op(
        ctx,
        StorageKind::SharedMemory,
        AtomOp(x.bits(87, 91)),
        AtomSize(x.bits(73, 76)),
        id,
        offset,
        value,
    );
    set(ctx, &x, res, false);
    Ok(())
}

pub(super) fn atoms_cas(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let ra = if x.ra() >= RZ {
        c(ctx, 0)
    } else {
        let r = gpr(ctx, x.ra());
        stride(ctx, &x, r)
    };
    let k = c(ctx, x.signed_field(40, 64) as i32);
    let sum = ctx.i_add(ra, k);
    let two = c(ctx, 2);
    let offset = ctx.shift_right_u32(sum, two);
    let e = if x.rb() >= RZ {
        c(ctx, 0)
    } else {
        gpr(ctx, x.rb())
    };
    let expected = ctx.copy(e);
    let value = if x.rc() >= RZ {
        c(ctx, 0)
    } else {
        gpr(ctx, x.rc())
    };
    let id = c(ctx, ctx.rm.shared_memory_id);
    let old =
        ctx.atomic_compare_and_swap_o4(StorageKind::SharedMemory, id, offset, expected, value);
    let eq = ctx.i_compare_equal(old, expected);
    let one = c(ctx, 1);
    let zero = c(ctx, 0);
    let v = ctx.conditional_select(eq, one, zero);
    set(ctx, &x, v, false);
    Ok(())
}
