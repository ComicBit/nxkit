//! Packed half instructions of `InstEmitAmpereAlu.cs`: HADD2, HMUL2, HFMA2 and the half lanes they read.

use super::operand::*;
use crate::ir::Opd;
use crate::maxwell::ampere::AmpOp;

/// `AHalfLanes`: a half-pair operand's two lanes after its swizzle, abs / neg applied. The swizzle is 0 as is, 1 `.F32`
/// (the 32-bit float itself in both lanes), 2 low twice and 3 high twice. A constant has its swizzle too. Only an
/// immediate has none, because those bits are its own.
pub(super) fn half_lanes(ctx: &mut Ctx, s: &ASrc, swizzle: i32) -> Result<[Opd; 2], String> {
    let v = get(ctx, s)?;
    let (lo, hi);
    if s.kind == AKind::Imm || swizzle == 0 {
        lo = ctx.unpack_half2x16_low(v);
        hi = ctx.unpack_half2x16_high(v);
    } else if swizzle == 1 {
        lo = v;
        hi = v;
    } else if swizzle == 3 {
        let h = ctx.unpack_half2x16_high(v);
        lo = h;
        hi = h;
    } else {
        let l = ctx.unpack_half2x16_low(v);
        lo = l;
        hi = l;
    }
    let a = ctx.fp_abs_neg(lo, s.abs, s.neg);
    let b = ctx.fp_abs_neg(hi, s.abs, s.neg);
    Ok([a, b])
}

type Lanes = [Opd; 2];

/// `AHalfSrcs`.
pub(super) fn half_srcs(ctx: &mut Ctx, x: &AmpOp) -> Result<(Lanes, Lanes, Lanes), String> {
    let (mut a, b, c) = alu_srcs(ctx, x, false, false, false)?;
    a.neg = x.bit(72);
    a.abs = x.bit(73);
    let b_in32 = matches!(x.form(), 1 | 4 | 5 | 6);
    let (mut slot32, mut slot64) = if b_in32 { (b, c) } else { (c, b) };
    if slot32.kind != AKind::Imm {
        slot32.neg = x.bit(63);
        slot32.abs = x.bit(62);
    }
    slot64.neg = x.bit(84);
    slot64.abs = x.bit(83);
    let ha = half_lanes(ctx, &a, x.bits(74, 76))?;
    let h32 = half_lanes(ctx, &slot32, x.bits(60, 62))?;
    let h64 = half_lanes(ctx, &slot64, x.bits(81, 83))?;
    Ok(if b_in32 {
        (ha, h32, h64)
    } else {
        (ha, h64, h32)
    })
}

/// `AHalfSecond`.
pub(super) fn half_second(ctx: &mut Ctx, x: &AmpOp) -> Result<(Lanes, Lanes), String> {
    let (a, b, c) = half_srcs(ctx, x)?;
    Ok((
        a,
        if matches!(x.form(), 1 | 4 | 5 | 6) {
            b
        } else {
            c
        },
    ))
}

/// `AHalfResult`.
pub(super) fn half_result(ctx: &mut Ctx, x: &AmpOp, lo: Opd, hi: Opd) -> R {
    let sat = x.bit(77);
    if x.bit(85) {
        return Err(fail(ctx, "BF16 half pair"));
    }
    let (mut lo, mut hi) = (lo, hi);
    if x.bit(79) && matches!(x.opcode() & 0x1ff, 0x031 | 0x032) {
        let z = ctx.ir.const_f(0.0);
        lo = ctx.fp_maximum(lo, z);
        let z = ctx.ir.const_f(0.0);
        hi = ctx.fp_maximum(hi, z);
    }
    if x.bit(78) {
        let v = ctx.fp_saturate_if(lo, sat);
        set(ctx, x, v, false);
        return Ok(());
    }
    let l = ctx.fp_saturate_if(lo, sat);
    let h = ctx.fp_saturate_if(hi, sat);
    let v = ctx.pack_half2x16(l, h);
    set(ctx, x, v, false);
    Ok(())
}

pub(super) fn hadd2(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let (a, b) = half_second(ctx, &x)?;
    let lo = ctx.fp_add(a[0], b[0]);
    let hi = ctx.fp_add(a[1], b[1]);
    half_result(ctx, &x, lo, hi)
}

pub(super) fn hmul2(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let (a, b) = half_second(ctx, &x)?;
    let lo = ctx.fp_multiply(a[0], b[0]);
    let hi = ctx.fp_multiply(a[1], b[1]);
    half_result(ctx, &x, lo, hi)
}

pub(super) fn hfma2(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let (a, mut b, c) = half_srcs(ctx, &x)?;
    if x.bit(86) {
        let n = ctx.fp_negate(b[1]);
        b = [n, b[0]];
    }
    let lo = ctx.fp_fused_multiply_add(a[0], b[0], c[0]);
    let hi = ctx.fp_fused_multiply_add(a[1], b[1], c[1]);
    half_result(ctx, &x, lo, hi)
}
