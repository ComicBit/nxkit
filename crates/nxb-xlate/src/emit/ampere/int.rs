//! Integer instructions of `InstEmitAmpereAlu.cs`: ISETP, IADD3, IMAD, LOP3, SHF, LEA, IMNMX, PRMT, POPC, FLO, BREV, SGXT.

#![allow(clippy::needless_late_init)]

use super::operand::*;
use crate::emit::alu::helper::get_pred_logical_op;
use crate::emit::alu::int_cmp::get_int_comparison;
use crate::emit::alu::int_logic::get_from_truth_table;
use crate::ir::Opd;
use crate::ir::ops::{FALSE, TRUE};
use crate::maxwell::ampere::AmpOp;
use crate::maxwell::enums::*;

fn c(ctx: &mut Ctx, v: i32) -> Opd {
    ctx.ir.const_(v)
}

fn sel01(ctx: &mut Ctx, cond: Opd) -> Opd {
    let one = c(ctx, 1);
    let zero = c(ctx, 0);
    ctx.conditional_select(cond, one, zero)
}

pub(super) fn isetp(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let u = is_uniform(ctx);
    let (a, b) = second(ctx, &x, u, false, false)?;
    let cmp_op = IComp(x.bits(76, 79));
    let signed = x.bit(73);
    let src_a = get(ctx, &a)?;
    let src_b = get(ctx, &b)?;
    let cmp;
    if x.bit(72) {
        let low = pred(ctx, &x, 68, 71, u);
        let eq = ctx.i_compare_equal(src_a, src_b);
        cmp = match cmp_op {
            IComp::F => c(ctx, FALSE),
            IComp::T => c(ctx, TRUE),
            IComp::Eq => ctx.bitwise_and(eq, low),
            IComp::Ne => {
                let ne = ctx.i_compare_not_equal(src_a, src_b);
                ctx.bitwise_or(ne, low)
            }
            IComp::Lt | IComp::Le => {
                let l = get_int_comparison(ctx, IComp::Lt, src_a, src_b, signed, false)?;
                let e = ctx.bitwise_and(eq, low);
                ctx.bitwise_or(l, e)
            }
            _ => {
                let g = get_int_comparison(ctx, IComp::Gt, src_a, src_b, signed, false)?;
                let e = ctx.bitwise_and(eq, low);
                ctx.bitwise_or(g, e)
            }
        };
    } else {
        cmp = get_int_comparison(ctx, cmp_op, src_a, src_b, signed, false)?;
    }
    let p = pred(ctx, &x, 87, 90, u);
    let bop = BoolOp(x.bits(74, 76));
    let v0 = get_pred_logical_op(ctx, bop, cmp, p);
    set_pred(ctx, x.bits(81, 84), v0, u);
    let n = ctx.bitwise_not(cmp);
    let v1 = get_pred_logical_op(ctx, bop, n, p);
    set_pred(ctx, x.bits(84, 87), v1, u);
    Ok(())
}

pub(super) fn iadd3(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let u = is_uniform(ctx);
    let (a, b, cc) = alu_srcs(ctx, &x, u, true, false)?;
    let ex = x.bit(74);
    let mut terms: Vec<Opd> = Vec::new();
    let mut plus_one = 0;
    for s in [a, b, cc] {
        let mut v = get(ctx, &s)?;
        if s.neg {
            v = ctx.bitwise_not(v);
            if !ex {
                plus_one += 1;
            }
        }
        terms.push(v);
    }
    if ex {
        for (lo, not_bit) in [(87u32, 90u32), (77, 80)] {
            if x.bits(lo, lo + 3) == PT && x.bit(not_bit) {
                continue;
            }
            let p = pred(ctx, &x, lo, not_bit, u);
            let t = sel01(ctx, p);
            terms.push(t);
        }
    }
    if plus_one != 0 {
        terms.push(c(ctx, plus_one));
    }
    let o0 = x.bits(81, 84);
    let o1 = x.bits(84, 87);
    let carries = o0 != PT || o1 != PT;
    let mut sum = terms[0];
    let mut carry_count = c(ctx, 0);
    for &t in &terms[1..] {
        let next = ctx.i_add(sum, t);
        if carries {
            let lt = ctx.i_compare_less_unsigned(next, sum);
            let s = sel01(ctx, lt);
            carry_count = ctx.i_add(carry_count, s);
        }
        sum = next;
    }
    set(ctx, &x, sum, u);
    if carries {
        let one = c(ctx, 1);
        let m = ctx.bitwise_and(carry_count, one);
        let z = c(ctx, 0);
        let v = ctx.i_compare_not_equal(m, z);
        set_pred(ctx, o0, v, u);
        let two = c(ctx, 2);
        let m = ctx.bitwise_and(carry_count, two);
        let z = c(ctx, 0);
        let v = ctx.i_compare_not_equal(m, z);
        set_pred(ctx, o1, v, u);
    }
    Ok(())
}

/// `AImadCarryIn`.
fn imad_carry_in(ctx: &mut Ctx, x: &AmpOp, u: bool) -> Option<Opd> {
    if !x.bit(74) || (x.bits(87, 90) == PT && x.bit(90)) {
        return None;
    }
    let p = pred(ctx, x, 87, 90, u);
    Some(sel01(ctx, p))
}

pub(super) fn imad(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let u = is_uniform(ctx);
    let (a, b, cc) = alu_srcs(ctx, &x, u, true, false)?;
    if x.bits(81, 84) != PT {
        return Err(fail(ctx, "IMAD predicate output"));
    }
    let ga = get(ctx, &a)?;
    let ib = int(ctx, &b)?;
    let mut res = ctx.i_multiply(ga, ib);
    if x.bit(74) && cc.neg {
        let g = get(ctx, &cc)?;
        let n = ctx.bitwise_not(g);
        res = ctx.i_add(res, n);
    } else if !cc.is_zero_reg() {
        let ic = int(ctx, &cc)?;
        res = ctx.i_add(res, ic);
    }
    if let Some(ci) = imad_carry_in(ctx, &x, u) {
        res = ctx.i_add(res, ci);
    }
    set(ctx, &x, res, u);
    Ok(())
}

fn carry_of(ctx: &mut Ctx, sum: Opd, base: Opd) -> Opd {
    let lt = ctx.i_compare_less_unsigned(sum, base);
    sel01(ctx, lt)
}

pub(super) fn imad_wide(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let u = is_uniform(ctx);
    let (a, b, cc) = alu_srcs(ctx, &x, u, true, false)?;
    if b.neg || cc.neg {
        return Err(fail(ctx, "IMAD.WIDE negation"));
    }
    let src_a = get(ctx, &a)?;
    let src_b = get(ctx, &b)?;
    let signed = x.bit(73);
    let mut lo = ctx.i_multiply(src_a, src_b);
    let mut hi = if signed {
        ctx.multiply_high_s32(src_a, src_b)
    } else {
        ctx.multiply_high_u32(src_a, src_b)
    };
    let mut carry_out = c(ctx, 0);
    let carry_in = imad_carry_in(ctx, &x, u);
    if !cc.is_zero_reg() || carry_in.is_some() {
        let c_lo = get(ctx, &cc)?;
        let mut sum = ctx.i_add(lo, c_lo);
        let mut carry = carry_of(ctx, sum, lo);
        if let Some(ci) = carry_in {
            let next = ctx.i_add(sum, ci);
            let c2 = carry_of(ctx, next, sum);
            carry = ctx.bitwise_or(carry, c2);
            sum = next;
        }
        let ch = get_high(ctx, &cc);
        let hi1 = ctx.i_add(hi, ch);
        let hi2 = ctx.i_add(hi1, carry);
        let a1 = carry_of(ctx, hi1, hi);
        let a2 = carry_of(ctx, hi2, hi1);
        carry_out = ctx.bitwise_or(a1, a2);
        hi = hi2;
        lo = sum;
    }
    let z = c(ctx, 0);
    let nz = ctx.i_compare_not_equal(carry_out, z);
    set_pred(ctx, x.bits(81, 84), nz, false);
    let rd = rd_of(&x, u);
    let zero = if u { URZ } else { RZ };
    let ty = if u {
        crate::ir::ops::regtype::UNIFORM
    } else {
        crate::ir::ops::regtype::GPR
    };
    if rd < zero {
        let d = ctx.ir.register(rd, ty);
        ctx.copy_to(d, lo);
        if rd + 1 < zero {
            let d = ctx.ir.register(rd + 1, ty);
            ctx.copy_to(d, hi);
        }
    }
    Ok(())
}

pub(super) fn imad_hi(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let (a, b, cc) = alu_srcs(ctx, &x, false, true, false)?;
    if cc.neg {
        return Err(fail(ctx, "IMAD.HI negation"));
    }
    let src_a = get(ctx, &a)?;
    let src_b = int(ctx, &b)?;
    let mut res = if x.bit(73) {
        ctx.multiply_high_s32(src_a, src_b)
    } else {
        ctx.multiply_high_u32(src_a, src_b)
    };
    let mut carry_out = c(ctx, 0);
    let carry_in = imad_carry_in(ctx, &x, false);
    if !cc.is_zero_reg() || carry_in.is_some() {
        let lo = ctx.i_multiply(src_a, src_b);
        let gc = get(ctx, &cc)?;
        let mut sum = ctx.i_add(lo, gc);
        let mut carry = carry_of(ctx, sum, lo);
        if let Some(ci) = carry_in {
            let next = ctx.i_add(sum, ci);
            let c2 = carry_of(ctx, next, sum);
            carry = ctx.bitwise_or(carry, c2);
            sum = next;
        }
        let _ = sum;
        let ch = get_high(ctx, &cc);
        let hi1 = ctx.i_add(res, ch);
        let hi2 = ctx.i_add(hi1, carry);
        let a1 = carry_of(ctx, hi1, res);
        let a2 = carry_of(ctx, hi2, hi1);
        carry_out = ctx.bitwise_or(a1, a2);
        res = hi2;
    }
    let z = c(ctx, 0);
    let nz = ctx.i_compare_not_equal(carry_out, z);
    set_pred(ctx, x.bits(81, 84), nz, false);
    set(ctx, &x, res, false);
    Ok(())
}

pub(super) fn lop3(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let u = is_uniform(ctx);
    let (a, b, cc) = alu_srcs(ctx, &x, u, false, false)?;
    let ga = get(ctx, &a)?;
    let gb = get(ctx, &b)?;
    let gc = get(ctx, &cc)?;
    let res = get_from_truth_table(ctx, ga, gb, gc, x.bits(72, 80))
        .ok_or("Lop3: no expression for the truth table")?;
    set(ctx, &x, res, u);
    let pd = x.bits(81, 84);
    if pd != PT {
        let z = c(ctx, 0);
        let nz = ctx.i_compare_not_equal(res, z);
        let q = pred(ctx, &x, 87, 90, u);
        let v = if x.bit(80) {
            ctx.bitwise_and(nz, q)
        } else {
            ctx.bitwise_or(nz, q)
        };
        set_pred(ctx, pd, v, u);
    }
    Ok(())
}

pub(super) fn shf(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let u = is_uniform(ctx);
    let (a, b, cc) = alu_srcs(ctx, &x, u, false, false)?;
    let ty = x.bits(73, 75);
    let wide = ty <= 1;
    let signed = ty == 0 || ty == 2;
    let wrap = x.bit(75);
    let right = x.bit(76);
    let high = x.bit(80);
    let max = if wide { 64 } else { 32 };
    let lo = get(ctx, &a)?;
    let mut sh = get(ctx, &b)?;
    let hi = get(ctx, &cc)?;
    if wrap {
        let m = c(ctx, max - 1);
        sh = ctx.bitwise_and(sh, m);
    } else {
        let m = c(ctx, max);
        sh = ctx.i_minimum_u32(sh, m);
    }
    let res;
    if !right && !high {
        let k = c(ctx, 32);
        let lt = ctx.i_compare_less_unsigned(sh, k);
        let shl = ctx.shift_left(lo, sh);
        let z = c(ctx, 0);
        res = ctx.conditional_select(lt, shl, z);
    } else if right && high {
        let shr = if signed {
            let k = c(ctx, 31);
            let m = ctx.i_minimum_u32(sh, k);
            ctx.shift_right_s32(hi, m)
        } else {
            ctx.shift_right_u32(hi, sh)
        };
        res = if signed {
            shr
        } else {
            let k = c(ctx, 32);
            let lt = ctx.i_compare_less_unsigned(sh, k);
            let z = c(ctx, 0);
            ctx.conditional_select(lt, shr, z)
        };
    } else {
        let z0 = c(ctx, 0);
        let inner = ctx.i_compare_equal(sh, z0);
        let k32 = c(ctx, 32);
        let under32 = ctx.i_compare_less_unsigned(sh, k32);
        let k32b = c(ctx, 32);
        let back = ctx.i_subtract(k32b, sh);
        let k32c = c(ctx, 32);
        let over = ctx.i_subtract(sh, k32c);
        if !right {
            let h1 = ctx.shift_left(hi, sh);
            let l1 = ctx.shift_right_u32(lo, back);
            let mut v = ctx.bitwise_or(h1, l1);
            v = ctx.conditional_select(inner, hi, v);
            let k = c(ctx, 32);
            let lt = ctx.i_compare_less_unsigned(over, k);
            let sl = ctx.shift_left(lo, over);
            let z = c(ctx, 0);
            let big = ctx.conditional_select(lt, sl, z);
            res = ctx.conditional_select(under32, v, big);
        } else {
            let l1 = ctx.shift_right_u32(lo, sh);
            let h1 = ctx.shift_left(hi, back);
            let mut v = ctx.bitwise_or(l1, h1);
            v = ctx.conditional_select(inner, lo, v);
            let shifted_hi = if signed {
                let k = c(ctx, 31);
                let m = ctx.i_minimum_u32(over, k);
                ctx.shift_right_s32(hi, m)
            } else {
                ctx.shift_right_u32(hi, over)
            };
            let big = if signed {
                shifted_hi
            } else {
                let k = c(ctx, 32);
                let lt = ctx.i_compare_less_unsigned(over, k);
                let z = c(ctx, 0);
                ctx.conditional_select(lt, shifted_hi, z)
            };
            res = ctx.conditional_select(under32, v, big);
        }
    }
    set(ctx, &x, res, u);
    Ok(())
}

pub(super) fn lea(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let u = is_uniform(ctx);
    let (a, b, cc) = alu_srcs(ctx, &x, u, false, false)?;
    let shift = x.bits(75, 80);
    let high = x.bit(80);
    let ex = x.bit(74);
    let neg_inter = x.bit(72);
    let mut src_b = get(ctx, &b)?;
    if matches!(x.form(), 1 | 5 | 6) && x.bit(63) {
        src_b = ctx.i_negate(src_b);
    }
    let lo = get(ctx, &a)?;
    let mut t;
    if !high {
        let k = c(ctx, shift);
        t = ctx.shift_left(lo, k);
    } else if shift == 0 {
        t = get(ctx, &cc)?;
    } else {
        let gc = get(ctx, &cc)?;
        let k = c(ctx, shift);
        let l = ctx.shift_left(gc, k);
        let k2 = c(ctx, 32 - shift);
        let r = ctx.shift_right_u32(lo, k2);
        t = ctx.bitwise_or(l, r);
    }
    let mut plus_one = 0;
    if neg_inter {
        t = ctx.bitwise_not(t);
        if !ex {
            plus_one = 1;
        }
    }
    let mut sum = ctx.i_add(t, src_b);
    let mut carry = ctx.i_compare_less_unsigned(sum, t);
    if plus_one != 0 {
        let one = c(ctx, 1);
        let next = ctx.i_add(sum, one);
        let c2 = ctx.i_compare_less_unsigned(next, sum);
        carry = ctx.bitwise_or(carry, c2);
        sum = next;
    }
    if ex && !(x.bits(87, 90) == PT && x.bit(90)) {
        let p = pred(ctx, &x, 87, 90, u);
        let s = sel01(ctx, p);
        let next = ctx.i_add(sum, s);
        let c2 = ctx.i_compare_less_unsigned(next, sum);
        carry = ctx.bitwise_or(carry, c2);
        sum = next;
    }
    set(ctx, &x, sum, u);
    set_pred(ctx, x.bits(81, 84), carry, u);
    Ok(())
}

pub(super) fn imnmx(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let u = is_uniform(ctx);
    let (a, b) = second(ctx, &x, u, false, false)?;
    let sa = get(ctx, &a)?;
    let sb = get(ctx, &b)?;
    let signed = x.bit(73);
    let (mn, mx) = if signed {
        (ctx.i_minimum_s32(sa, sb), ctx.i_maximum_s32(sa, sb))
    } else {
        (ctx.i_minimum_u32(sa, sb), ctx.i_maximum_u32(sa, sb))
    };
    let p = pred(ctx, &x, 87, 90, u);
    let v = ctx.conditional_select(p, mn, mx);
    set(ctx, &x, v, u);
    Ok(())
}

pub(super) fn iabs(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let s = one(ctx, &x, false, false, false)?;
    let g = get(ctx, &s)?;
    let v = ctx.i_absolute(g);
    set(ctx, &x, v, false);
    Ok(())
}
