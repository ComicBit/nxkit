//! Bit-level integer instructions of `InstEmitAmpereAlu.cs`: PRMT, POPC, FLO, BREV, SGXT.
#![allow(clippy::needless_late_init)]

use super::operand::*;
use crate::ir::Opd;
use crate::ir::ops::OperandType;

fn c(ctx: &mut Ctx, v: i32) -> Opd {
    ctx.ir.const_(v)
}

pub(super) fn prmt(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let u = is_uniform(ctx);
    let (a, b, cc) = alu_srcs(ctx, &x, u, false, false)?;
    if x.bits(72, 75) != 0 {
        return Err(fail(ctx, &format!("PRMT mode {}", x.bits(72, 75))));
    }
    let src_a = get(ctx, &a)?;
    let sel = get(ctx, &b)?;
    let src_c = get(ctx, &cc)?;
    let mut res = c(ctx, 0);
    for i in 0..4 {
        let byte_value;
        if ctx.ir.ty(sel) == OperandType::Constant {
            let s = (ctx.ir.value(sel) >> (i * 4)) & 0xf;
            let src = if s & 4 != 0 { src_c } else { src_a };
            let off = c(ctx, (s & 3) * 8);
            let eight = c(ctx, 8);
            let mut bv = ctx.bitfield_extract_u32(src, off, eight);
            if s & 8 != 0 {
                let k24 = c(ctx, 24);
                let sh = ctx.shift_left(bv, k24);
                let k31 = c(ctx, 31);
                bv = ctx.shift_right_s32(sh, k31);
            }
            byte_value = bv;
        } else {
            let off = c(ctx, i * 4);
            let four = c(ctx, 4);
            let s = ctx.bitfield_extract_u32(sel, off, four);
            let k4 = c(ctx, 4);
            let m4 = ctx.bitwise_and(s, k4);
            let z = c(ctx, 0);
            let ne4 = ctx.i_compare_not_equal(m4, z);
            let src = ctx.conditional_select(ne4, src_c, src_a);
            let k3 = c(ctx, 3);
            let m3 = ctx.bitwise_and(s, k3);
            let k3b = c(ctx, 3);
            let off2 = ctx.shift_left(m3, k3b);
            let eight = c(ctx, 8);
            let bv = ctx.bitfield_extract_u32(src, off2, eight);
            let k24 = c(ctx, 24);
            let sh = ctx.shift_left(bv, k24);
            let k31 = c(ctx, 31);
            let sign = ctx.shift_right_s32(sh, k31);
            let k8 = c(ctx, 8);
            let m8 = ctx.bitwise_and(s, k8);
            let z2 = c(ctx, 0);
            let ne8 = ctx.i_compare_not_equal(m8, z2);
            byte_value = ctx.conditional_select(ne8, sign, bv);
        }
        let ff = c(ctx, 0xff);
        let masked = ctx.bitwise_and(byte_value, ff);
        let k = c(ctx, i * 8);
        let sh = ctx.shift_left(masked, k);
        res = ctx.bitwise_or(res, sh);
    }
    set(ctx, &x, res, u);
    Ok(())
}

pub(super) fn popc(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let u = is_uniform(ctx);
    let s = one(ctx, &x, u, false, false)?;
    let mut src = get(ctx, &s)?;
    if x.bit(63) {
        src = ctx.bitwise_not(src);
    }
    let v = ctx.bit_count(src);
    set(ctx, &x, v, u);
    Ok(())
}

pub(super) fn flo(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let u = is_uniform(ctx);
    if x.bits(81, 84) != PT {
        return Err(fail(ctx, "FLO predicate output"));
    }
    let s = one(ctx, &x, u, false, false)?;
    let mut src = get(ctx, &s)?;
    if x.bit(63) {
        src = ctx.bitwise_not(src);
    }
    let mut res = if x.bit(73) {
        ctx.find_msbs32(src)
    } else {
        ctx.find_msbu32(src)
    };
    if x.bit(74) {
        let m1 = c(ctx, -1);
        let valid = ctx.i_compare_not_equal(res, m1);
        let k = c(ctx, 31);
        let xr = ctx.bitwise_exclusive_or(res, k);
        res = ctx.conditional_select(valid, xr, res);
    }
    set(ctx, &x, res, u);
    Ok(())
}

pub(super) fn brev(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let u = is_uniform(ctx);
    let s = one(ctx, &x, u, false, false)?;
    let g = get(ctx, &s)?;
    let v = ctx.bitfield_reverse(g);
    set(ctx, &x, v, u);
    Ok(())
}

pub(super) fn sgxt(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let u = is_uniform(ctx);
    let (a, b) = second(ctx, &x, u, false, false)?;
    let mut n = get(ctx, &b)?;
    n = if x.bit(75) {
        let k = c(ctx, 31);
        ctx.bitwise_and(n, k)
    } else {
        let k = c(ctx, 32);
        ctx.i_minimum_u32(n, k)
    };
    let src = get(ctx, &a)?;
    let z = c(ctx, 0);
    let mut res = if x.bit(73) {
        ctx.bitfield_extract_s32(src, z, n)
    } else {
        ctx.bitfield_extract_u32(src, z, n)
    };
    let z0 = c(ctx, 0);
    let eq = ctx.i_compare_equal(n, z0);
    let z1 = c(ctx, 0);
    res = ctx.conditional_select(eq, z1, res);
    set(ctx, &x, res, u);
    Ok(())
}
