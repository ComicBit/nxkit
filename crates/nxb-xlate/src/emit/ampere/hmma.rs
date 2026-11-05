//! HMMA.16816 / .1688 of `InstEmitAmpereTensor.cs`: a matrix multiply-add over the warp.

use super::operand::*;
use crate::attr::Stage;
use crate::emit::alu::mov::emit_load_subgroup_lane_id;
use crate::ir::Opd;
use crate::ir::inst::{Inst as I, IoVariable, StorageKind};

fn c(ctx: &mut Ctx, v: i32) -> Opd {
    ctx.ir.const_(v)
}

fn agpr(ctx: &mut Ctx, r: i32) -> Opd {
    if r >= RZ { c(ctx, 0) } else { gpr(ctx, r) }
}

fn half(ctx: &mut Ctx, packed: Opd, h: i32) -> Opd {
    if h == 0 {
        ctx.unpack_half2x16_low(packed)
    } else {
        ctx.unpack_half2x16_high(packed)
    }
}

pub(super) fn hmma(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let size = x.bits(75, 76) | (x.bits(78, 79) << 1);
    let f32_ = x.bit(76);
    if size > 1 || x.bits(82, 84) != 0 {
        return Err(fail(
            ctx,
            &format!("HMMA size {size} source type {}", x.bits(82, 84)),
        ));
    }
    let defs = &ctx.tc.definitions;
    if size == 1 && !f32_ && defs.stage == Stage::Compute {
        let [lx, ly, lz] = defs.compute_local_size;
        let threads = lx * ly * lz;
        let memory_id = ctx
            .rm
            .get_hmma_scratch_memory_id(1.max((threads + 31) / 32));
        let mut tid = Vec::new();
        for i in 0..3 {
            let e = c(ctx, i);
            tid.push(ctx.load_io_e(StorageKind::Input, IoVariable::ThreadId, None, e));
        }
        let d0 = ctx.ir.local();
        let d1 = ctx.ir.local();
        let kly = c(ctx, lx);
        let my = ctx.i_multiply(tid[1], kly);
        let klz = c(ctx, lx * ly);
        let mz = ctx.i_multiply(tid[2], klz);
        let s = ctx.i_add(my, mz);
        let flat = ctx.i_add(tid[0], s);
        let k5 = c(ctx, 5);
        let sh = ctx.shift_right_u32(flat, k5);
        let k8 = c(ctx, 8);
        let block = ctx.shift_left(sh, k8);
        let mid = c(ctx, memory_id);
        let lane = emit_load_subgroup_lane_id(ctx);
        let mut srcs = vec![mid, lane, block];
        for r in [
            x.ra(),
            x.ra() + 1,
            x.ra() + 2,
            x.ra() + 3,
            x.rb(),
            x.rb() + 1,
            x.rc(),
            x.rc() + 1,
        ] {
            srcs.push(agpr(ctx, r));
        }
        ctx.add2(I::MatrixMultiplyAdd, d0, d1, &srcs);
        if x.rd() < RZ {
            let d = gpr(ctx, x.rd());
            ctx.copy_to(d, d0);
        }
        if x.rd() + 1 < RZ {
            let d = gpr(ctx, x.rd() + 1);
            ctx.copy_to(d, d1);
        }
        return Ok(());
    }
    let k = if size == 1 { 16 } else { 8 };
    let a_regs = k / 4;
    let b_regs = k / 8;
    let lane = emit_load_subgroup_lane_id(ctx);
    let k2 = c(ctx, 2);
    let g = ctx.shift_right_u32(lane, k2);
    let k3 = c(ctx, 3);
    let t = ctx.bitwise_and(lane, k3);
    let mut a: Vec<Vec<Option<Opd>>> = vec![vec![None; 16]; 2];
    for j in 0..4 {
        let k2 = c(ctx, 2);
        let g4 = ctx.shift_left(g, k2);
        let jj = c(ctx, j);
        let src = ctx.i_add(g4, jj);
        for r in 0..a_regs {
            let ar = agpr(ctx, x.ra() + r);
            let v = ctx.shuffle(ar, src);
            let row = (r & 1) as usize;
            let kb = ((r >> 1) * 8 + 2 * j) as usize;
            a[row][kb] = Some(half(ctx, v, 0));
            a[row][kb + 1] = Some(half(ctx, v, 1));
        }
    }
    let mut b: Vec<Vec<Option<Opd>>> = vec![vec![None; 16]; 2];
    #[allow(clippy::needless_range_loop)]
    for cc in 0..2 {
        let k1 = c(ctx, 1);
        let t2 = ctx.shift_left(t, k1);
        let ccv = c(ctx, cc as i32);
        let col = ctx.i_add(t2, ccv);
        for j in 0..4 {
            let k2 = c(ctx, 2);
            let c4 = ctx.shift_left(col, k2);
            let jj = c(ctx, j);
            let src = ctx.i_add(c4, jj);
            for r in 0..b_regs {
                let br = agpr(ctx, x.rb() + r);
                let v = ctx.shuffle(br, src);
                let kb = (r * 8 + 2 * j) as usize;
                b[cc][kb] = Some(half(ctx, v, 0));
                b[cc][kb + 1] = Some(half(ctx, v, 1));
            }
        }
    }
    let mut acc: Vec<Opd> = Vec::new();
    for i in 0..4 {
        let (row, cc) = ((i >> 1) as usize, (i & 1) as usize);
        let mut v = if f32_ {
            agpr(ctx, x.rc() + i)
        } else {
            let r = agpr(ctx, x.rc() + row as i32);
            half(ctx, r, cc as i32)
        };
        for kk in 0..k as usize {
            let (Some(av), Some(bv)) = (a[row][kk], b[cc][kk]) else {
                return Err(fail(ctx, "HMMA fragment"));
            };
            v = ctx.fp_fused_multiply_add(av, bv, v);
        }
        acc.push(v);
    }
    if f32_ {
        for i in 0..4 {
            if x.rd() + i >= RZ {
                break;
            }
            let d = gpr(ctx, x.rd() + i);
            ctx.copy_to(d, acc[i as usize]);
        }
    } else {
        for row in 0..2 {
            if x.rd() + row >= RZ {
                break;
            }
            let v = ctx.pack_half2x16(acc[(row * 2) as usize], acc[(row * 2 + 1) as usize]);
            let d = gpr(ctx, x.rd() + row);
            ctx.copy_to(d, v);
        }
    }
    Ok(())
}
