//! Moves, special registers, constants and predicates of `InstEmitAmpereAlu.cs`.

use super::operand::*;
use crate::attr::Stage;
use crate::emit::alu::int_logic::emit_p2r;
use crate::emit::alu::mov::{emit_r2p, emit_sreg};
use crate::emit::alu::video::{emit_ballot, emit_vote};
use crate::emit::mem::memory::{emit_load_constant, extract_small_int, get_bit_offset};
use crate::ir::Opd;
use crate::ir::inst::{IoVariable, StorageKind};
use crate::ir::ops::{FALSE, TRUE, regtype};
use crate::maxwell::ampere::AmpOp;
use crate::maxwell::ampere_table::reads_constant_bank;
use crate::maxwell::enums::*;

fn c(ctx: &mut Ctx, v: i32) -> Opd {
    ctx.ir.const_(v)
}

pub(super) fn mov(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let s = one(ctx, &x, false, false, false)?;
    let v = get(ctx, &s)?;
    set(ctx, &x, v, false);
    Ok(())
}

pub(super) fn umov(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let src = if x.form() == 4 {
        c(ctx, x.imm32() as i32)
    } else {
        ctx.ir.register(x.bits(32, 38), regtype::UNIFORM)
    };
    set(ctx, &x, src, true);
    Ok(())
}

pub(super) fn r2ur(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let r = gpr(ctx, x.ra());
    set(ctx, &x, r, true);
    Ok(())
}

pub(super) fn sel(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let u = is_uniform(ctx);
    let (a, b) = second(ctx, &x, u, false, false)?;
    let p = pred(ctx, &x, 87, 90, u);
    let ga = get(ctx, &a)?;
    let gb = get(ctx, &b)?;
    let v = ctx.conditional_select(p, ga, gb);
    set(ctx, &x, v, u);
    Ok(())
}

pub(super) fn cs2r(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let sr = x.bits(72, 80);
    for k in 0..(if x.bit(80) { 2 } else { 1 }) {
        if x.rd() + k >= RZ {
            break;
        }
        let v = if sr == 255 {
            c(ctx, 0)
        } else {
            emit_sreg(ctx, SReg(sr + k))?
        };
        let d = gpr(ctx, x.rd() + k);
        ctx.copy_to(d, v);
    }
    Ok(())
}

pub(super) fn s2r(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let v = emit_sreg(ctx, SReg(x.bits(72, 80)))?;
    let u = is_uniform(ctx);
    set(ctx, &x, v, u);
    Ok(())
}

pub(super) fn uldc(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    if x.opcode() == 0xab9 && x.bit(91) {
        return Err(fail(ctx, "ULDC with its bank in a uniform register"));
    }
    if x.opcode() == 0xabb {
        let idx = ctx.ir.register(x.bits(24, 30), regtype::UNIFORM);
        let slot = c(ctx, x.bits(54, 59));
        load_constant(
            ctx,
            &x,
            slot,
            idx,
            x.signed_field(38, 54) as i32,
            true,
            None,
        )
    } else {
        let (bank, offset) = map_c0(ctx, x.bits(54, 59), x.signed_field(38, 54) as i32);
        let slot = c(ctx, bank);
        let z = c(ctx, 0);
        load_constant(ctx, &x, slot, z, offset, true, None)
    }
}

pub(super) fn ldc(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let mode = x.bits(78, 80);
    if x.bit(91) || mode == 1 {
        return Err(fail(
            ctx,
            "LDC with a bindless bank or the .IL address mode",
        ));
    }
    if mode != 0 {
        // .IS and .ISL: the bank is the instruction's plus Ra's upper half, the offset Ra's lower half plus the instruction's
        let ra = gpr(ctx, x.ra());
        let bank = c(ctx, x.bits(54, 59));
        let sh = c(ctx, 16);
        let hi = ctx.shift_right_u32(ra, sh);
        let slot = ctx.i_add(bank, hi);
        let m = c(ctx, 0xffff);
        let idx = ctx.bitwise_and(ra, m);
        let banks = runtime_banks(ctx, x.bits(54, 59));
        return load_constant(
            ctx,
            &x,
            slot,
            idx,
            x.signed_field(38, 54) as i32,
            false,
            Some(banks),
        );
    }
    if x.ra() == RZ {
        let (bank, offset) = map_c0(ctx, x.bits(54, 59), x.signed_field(38, 54) as i32);
        let slot = c(ctx, bank);
        let z = c(ctx, 0);
        load_constant(ctx, &x, slot, z, offset, false, None)
    } else {
        let idx = gpr(ctx, x.ra());
        let slot = c(ctx, x.bits(54, 59));
        load_constant(
            ctx,
            &x,
            slot,
            idx,
            x.signed_field(38, 54) as i32,
            false,
            None,
        )
    }
}

/// `ARuntimeBanks`: the banks a run-time bank of LDC .IS can be. The shader's own constant segment (c1), the
/// instruction's bank and every bank the other constant reads name. c0 is left out.
fn runtime_banks(ctx: &Ctx, bank: i32) -> u32 {
    let mut banks = 1u32 << 1 | 1u32 << bank;
    for f in &ctx.tc.program.functions {
        for b in &f.blocks {
            for op in &b.ops {
                let y = AmpOp::of(op);
                if reads_constant_bank(&y) && !y.bit(91) {
                    let (bank, _) = map_c0(ctx, y.bits(54, 59), y.signed_field(38, 54) as i32);
                    banks |= 1u32 << bank;
                }
            }
        }
    }
    banks & !1u32
}

fn load_constant(
    ctx: &mut Ctx,
    x: &AmpOp,
    slot: Opd,
    index: Opd,
    offset: i32,
    uniform: bool,
    banks: Option<u32>,
) -> R {
    let size = x.bits(73, 76);
    if size > 5 {
        return Err(fail(ctx, &format!("constant load size {size}")));
    }
    let k = c(ctx, offset);
    let addr = ctx.i_add(index, k);
    let two = c(ctx, 2);
    let word_offset = ctx.shift_right_u32(addr, two);
    let rd = rd_of(x, uniform);
    let zero = if uniform { URZ } else { RZ };
    let ty = if uniform {
        regtype::UNIFORM
    } else {
        regtype::GPR
    };
    let mut k = 0;
    while k < (if size == 5 { 2 } else { 1 }) && rd + k < zero {
        let kk = c(ctx, k);
        let off = ctx.i_add(word_offset, kk);
        let mut value = emit_load_constant(ctx, slot, off, banks)?;
        if size < 4 {
            let bo = get_bit_offset(ctx, addr);
            value = extract_small_int(ctx, size, bo, value);
        }
        let d = ctx.ir.register(rd + k, ty);
        ctx.copy_to(d, value);
        k += 1;
    }
    Ok(())
}

/// `APixelCovered`: 0x925 with 78..81 = 1 reads 1 for a covered pixel and 0 for a helper invocation. Any other form is refused.
pub(super) fn pixel_covered(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    if x.bits(78, 81) != 1
        || x.bits(81, 84) != 7
        || x.bits(24, 32) != 0
        || x.bits(32, 48) != 0
        || ctx.tc.definitions.stage != Stage::Fragment
    {
        let form = x.bits(78, 81);
        return Err(fail(
            ctx,
            &format!("0x925 form {form} outside a covered pixel's read in a pixel shader"),
        ));
    }
    let helper = ctx.load_io(StorageKind::Input, IoVariable::HelperInvocation, None);
    let zero = c(ctx, 0);
    let one = c(ctx, 1);
    let v = ctx.conditional_select(helper, zero, one);
    set(ctx, &x, v, false);
    Ok(())
}

pub(super) fn r2p(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let mask = if x.opcode() == 0x804 {
        c(ctx, x.imm32() as i32)
    } else {
        gpr(ctx, x.rb())
    };
    let a = gpr(ctx, x.ra());
    emit_r2p(ctx, a, mask, ByteSel(x.bits(76, 78)), false);
    Ok(())
}

pub(super) fn p2r(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let mask = if x.opcode() == 0x803 {
        c(ctx, x.imm32() as i32)
    } else {
        gpr(ctx, x.rb())
    };
    let a = gpr(ctx, x.ra());
    let d = gpr(ctx, x.rd());
    emit_p2r(ctx, a, d, mask, ByteSel(x.bits(76, 78)), false);
    Ok(())
}

/// `AUp2ur`: bit k of Rd is the uniform predicate UPk (7 is UPT) where bit k of the mask is set.
pub(super) fn up2ur(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    if x.bits(24, 30) != URZ || x.imm32() & !0xff != 0 {
        return Err(fail(
            ctx,
            "UP2UR with a merge source or a mask past the predicates",
        ));
    }
    let mut res = c(ctx, 0);
    for k in 0..8 {
        if x.imm32() >> k & 1 != 0 {
            let p = pred_reg(ctx, k, true);
            let bit = c(ctx, 1 << k);
            let zero = c(ctx, 0);
            let v = ctx.conditional_select(p, bit, zero);
            res = ctx.bitwise_or(res, v);
        }
    }
    set(ctx, &x, res, true);
    Ok(())
}

pub(super) fn plop3(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let u = is_uniform(ctx);
    let a = pred(ctx, &x, 87, 90, u);
    let b = pred(ctx, &x, 77, 80, u);
    let cc = pred(ctx, &x, 68, 71, u || x.bit(67));
    let lut0 = x.bits(64, 67) | (x.bits(72, 77) << 3);
    let lut1 = x.bits(16, 24);
    let v0 = pred_lut(ctx, a, b, cc, lut0);
    set_pred(ctx, x.bits(81, 84), v0, u);
    let v1 = pred_lut(ctx, a, b, cc, lut1);
    set_pred(ctx, x.bits(84, 87), v1, u);
    Ok(())
}

/// `APredLut`: a three-input boolean function by its truth table (a = 0xf0, b = 0xcc, c = 0xaa).
fn pred_lut(ctx: &mut Ctx, a: Opd, b: Opd, cc: Opd, lut: i32) -> Opd {
    let lut = lut & 0xff;
    if lut == 0 {
        return c(ctx, FALSE);
    }
    if lut == 0xff {
        return c(ctx, TRUE);
    }
    let mut res: Option<Opd> = None;
    for m in 0..8 {
        let bit = (if m & 4 != 0 { 0xf0 } else { 0x0f })
            & (if m & 2 != 0 { 0xcc } else { 0x33 })
            & (if m & 1 != 0 { 0xaa } else { 0x55 });
        if lut & bit == 0 {
            continue;
        }
        let ta = if m & 4 != 0 { a } else { ctx.bitwise_not(a) };
        let tb = if m & 2 != 0 { b } else { ctx.bitwise_not(b) };
        let ab = ctx.bitwise_and(ta, tb);
        let tc = if m & 1 != 0 { cc } else { ctx.bitwise_not(cc) };
        let term = ctx.bitwise_and(ab, tc);
        res = Some(match res {
            None => term,
            Some(r) => ctx.bitwise_or(r, term),
        });
    }
    res.unwrap_or_else(|| c(ctx, FALSE))
}

pub(super) fn vote(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let to_uniform = x.opcode() == 0x886;
    let p = pred(ctx, &x, 87, 90, false);
    let vote = emit_vote(ctx, VoteMode(x.bits(72, 74)), p)?;
    if let Some(v) = vote {
        set_pred(ctx, x.bits(81, 84), v, to_uniform);
    }
    let rd = rd_of(&x, to_uniform);
    if rd != if to_uniform { URZ } else { RZ } {
        let b = emit_ballot(ctx, p)?;
        let d = ctx.ir.register(
            rd,
            if to_uniform {
                regtype::UNIFORM
            } else {
                regtype::GPR
            },
        );
        ctx.copy_to(d, b);
    }
    Ok(())
}

pub(super) fn shfl(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let lane_imm = matches!(x.opcode(), 0x989 | 0xf89);
    let c_imm = matches!(x.opcode(), 0x589 | 0xf89);
    let src_a = gpr(ctx, x.ra());
    let lane = if lane_imm {
        c(ctx, x.bits(53, 58))
    } else {
        gpr(ctx, x.rb())
    };
    let cc = if c_imm {
        c(ctx, x.bits(40, 53))
    } else {
        gpr(ctx, x.rc())
    };
    let (res, valid) = match ShflMode(x.bits(58, 60)) {
        ShflMode::Idx => ctx.shuffle_3(src_a, lane, cc),
        ShflMode::Up => ctx.shuffle_up_3(src_a, lane, cc),
        ShflMode::Down => ctx.shuffle_down_3(src_a, lane, cc),
        _ => ctx.shuffle_xor_3(src_a, lane, cc),
    };
    set(ctx, &x, res, false);
    set_pred(ctx, x.bits(81, 84), valid, false);
    Ok(())
}
