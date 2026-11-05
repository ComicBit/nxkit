//! Surfaces of `InstEmitAmpereMem.cs`: SULD, SUST, SUATOM. The shared emitters bind the handle in the default bank (c[2]);
//! a handle in another bank fails closed.

use super::operand::*;
use super::tex::tex_handle;
use crate::emit::mem::surface::emit_suatom;
use crate::emit::mem::surface_ldst::{emit_suld, emit_sust};
use crate::emit::mem::tex::DEFAULT_CBUF_SLOT;
use crate::maxwell::enums::*;

pub(super) fn surface(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let (handle, cbuf_slot) = tex_handle(ctx, &x);
    if cbuf_slot != DEFAULT_CBUF_SLOT {
        return Err(format!(
            "unported: Ampere surface handle in bank {cbuf_slot}"
        ));
    }
    let formatted = matches!(x.opcode(), 0xb97 | 0xb9b);
    let dim = SuDim(x.bits(61, 64));
    let size = SuSize(if formatted { 0 } else { x.bits(73, 76) });
    let mask = if formatted { x.bits(72, 76) } else { 0 };
    if matches!(x.opcode(), 0xb97 | 0xb99) {
        emit_suld(
            ctx,
            CacheOpLd::Ca,
            dim,
            size,
            handle,
            mask,
            x.ra(),
            x.rd(),
            0,
            formatted,
            false,
            false,
        )
    } else {
        emit_sust(
            ctx,
            CacheOpSt::Wb,
            dim,
            size,
            handle,
            mask,
            x.ra(),
            x.rb(),
            0,
            formatted,
            false,
            false,
        )
    }
}

pub(super) fn suatom(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let (handle, cbuf_slot) = tex_handle(ctx, &x);
    if cbuf_slot != DEFAULT_CBUF_SLOT {
        return Err(fail(ctx, &format!("SUATOM handle in bank {cbuf_slot}")));
    }
    if x.bits(81, 84) != 7 {
        return Err(fail(ctx, "SUATOM with a predicate result"));
    }
    emit_suatom(
        ctx,
        SuDim(x.bits(61, 64)),
        0,
        SuatomOp(x.bits(87, 91)),
        SuatomSize(x.bits(73, 76)),
        handle,
        x.ra(),
        x.rb(),
        0,
        x.rd(),
        false,
        false,
        false,
        false,
    )
}
