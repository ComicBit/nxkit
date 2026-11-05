//! `TensorPass` and `HelperFunctionManager.GenerateHmma16816F16`: Ampere HMMA.16816 with an f16 accumulator becomes a call of the
//! helper `dmsh_hmma16816_f16`.
use super::XformCtx;
use super::helper::{B, Function};
use crate::ir::{Block, Inst, Ir, Opd, StorageKind};

type R<T> = Result<T, String>;

/// Key of a helper in `Hfm::ids` (the scratch memory id is a parameter of the function).
const KEY_BASE: i32 = 0x4000_0000;

fn c(b: &mut B, v: i32) -> Opd {
    b.ir.const_(v)
}

fn add(b: &mut B, x: Opd, y: Opd) -> Opd {
    b.op(Inst::Add, &[x, y])
}

fn shl(b: &mut B, x: Opd, k: i32) -> Opd {
    let k = c(b, k);
    b.op(Inst::ShiftLeft, &[x, k])
}

fn unpack(b: &mut B, a: Opd, index: i32) -> Opd {
    let d = b.ir.local();
    let n =
        b.ir.new_op_multi(Inst::UnpackHalf2x16, index, &[Some(d)], &[a]);
    b.push(n);
    d
}

fn fma(b: &mut B, x: Opd, y: Opd, z: Opd) -> Opd {
    b.op(Inst(Inst::FP32) | Inst::FusedMultiplyAdd, &[x, y, z])
}

fn load(b: &mut B, id: i32, off: Opd) -> Opd {
    let idc = c(b, id);
    b.op_kind(Inst::Load, StorageKind::SharedMemory, &[idc, off])
}

fn store(b: &mut B, id: i32, off: Opd, v: Opd) {
    let idc = c(b, id);
    b.raw(Inst::Store, StorageKind::SharedMemory, None, &[idc, off, v]);
}

/// `At(row, word)`: `bse + (row << 3) + (t + word)`.
fn at(b: &mut B, bse: Opd, t: Opd, row: Opd, word: i32) -> Opd {
    let s = shl(b, row, 3);
    let w = c(b, word);
    let tw = add(b, t, w);
    let i = add(b, s, tw);
    add(b, bse, i)
}

/// `Ct(row)`: `bse + (192 + ((row << 2) + t))`.
fn ct(b: &mut B, bse: Opd, t: Opd, row: Opd) -> Opd {
    let s = shl(b, row, 2);
    let st = add(b, s, t);
    let k = c(b, 192);
    let i = add(b, k, st);
    add(b, bse, i)
}

fn generate(ir: &mut Ir, id: i32) -> R<Function> {
    let mut b = B::new(ir);
    let lane = b.ir.argument(0);
    let bse = b.ir.argument(1);
    let k2 = c(&mut b, 2);
    let g = b.op(Inst::ShiftRightU32, &[lane, k2]);
    let k3 = c(&mut b, 3);
    let t = b.op(Inst::BitwiseAnd, &[lane, k3]);
    let k8 = c(&mut b, 8);
    let g8 = add(&mut b, g, k8);
    let a = |b: &mut B, n: i32| b.ir.argument(n);
    let o = at(&mut b, bse, t, g, 0);
    let v = a(&mut b, 2);
    store(&mut b, id, o, v);
    let o = at(&mut b, bse, t, g8, 0);
    let v = a(&mut b, 3);
    store(&mut b, id, o, v);
    let o = at(&mut b, bse, t, g, 4);
    let v = a(&mut b, 4);
    store(&mut b, id, o, v);
    let o = at(&mut b, bse, t, g8, 4);
    let v = a(&mut b, 5);
    store(&mut b, id, o, v);
    for (word, arg) in [(0, 6), (4, 7)] {
        let x = at(&mut b, bse, t, g, word);
        let k = c(&mut b, 128);
        let o = add(&mut b, k, x);
        let v = a(&mut b, arg);
        store(&mut b, id, o, v);
    }
    let o = ct(&mut b, bse, t, g);
    let v = a(&mut b, 8);
    store(&mut b, id, o, v);
    let o = ct(&mut b, bse, t, g8);
    let v = a(&mut b, 9);
    store(&mut b, id, o, v);
    b.raw(Inst::SubgroupBarrier, StorageKind::None, None, &[]);
    let mut d = Vec::new();
    for q in 0..2 {
        let row = if q == 0 { g } else { g8 };
        let cw = a(&mut b, 8 + q);
        let lo = unpack(&mut b, cw, 0);
        let hi = unpack(&mut b, cw, 1);
        let mut acc = [lo, hi];
        let rs = shl(&mut b, row, 3);
        let row_base = add(&mut b, bse, rs);
        for j in 0..2 {
            let ts = shl(&mut b, t, 1);
            let jj = c(&mut b, j);
            let col = add(&mut b, ts, jj);
            let cs = shl(&mut b, col, 3);
            let k = c(&mut b, 128);
            let kc = add(&mut b, k, cs);
            let col_base = add(&mut b, bse, kc);
            for kw in 0..8 {
                let kk = c(&mut b, kw);
                let ao = add(&mut b, row_base, kk);
                let aw = load(&mut b, id, ao);
                let kk2 = c(&mut b, kw);
                let bo = add(&mut b, col_base, kk2);
                let bw = load(&mut b, id, bo);
                let (al, bl) = (unpack(&mut b, aw, 0), unpack(&mut b, bw, 0));
                acc[j as usize] = fma(&mut b, al, bl, acc[j as usize]);
                let (ah, bh) = (unpack(&mut b, aw, 1), unpack(&mut b, bw, 1));
                acc[j as usize] = fma(&mut b, ah, bh, acc[j as usize]);
            }
        }
        d.push(b.op(Inst::PackHalf2x16, &[acc[0], acc[1]]));
    }
    b.raw(Inst::SubgroupBarrier, StorageKind::None, None, &[]);
    let arg10 = a(&mut b, 10);
    let copy =
        b.ir.new_op(Inst::Copy, StorageKind::None, Some(arg10), &[d[1]]);
    b.push(copy);
    b.ret(Some(d[0]));
    b.finish_with("dmsh_hmma16816_f16", true, 10, 1)
}

/// `TensorPass.RunPass` over all blocks.
pub fn run_pass(ir: &mut Ir, blocks: &mut [Block], ctx: &mut XformCtx) -> R<()> {
    for block in blocks.iter_mut() {
        let mut i = 0;
        while i < block.ops.len() {
            let node = block.ops[i];
            if ir.is_op(node) && ir.op(node).inst == Inst::MatrixMultiplyAdd {
                let o = ir.op(node);
                let (Some(mem), Some(d0), Some(d1)) = (
                    o.sources.first().copied().flatten(),
                    o.dests.first().copied().flatten(),
                    o.dests.get(1).copied().flatten(),
                ) else {
                    return Err("MatrixMultiplyAdd without operands".into());
                };
                let srcs: Vec<Opd> = o.sources[1..11].iter().flatten().copied().collect();
                if srcs.len() != 10 {
                    return Err("MatrixMultiplyAdd sources".into());
                }
                let mem_id = ir.value(mem);
                let key = KEY_BASE + mem_id;
                let fid = if let Some(id) = ctx.hfm.ids.get(&key) {
                    *id
                } else {
                    let f = generate(ir, mem_id)?;
                    let id = ctx.hfm.add_function(f);
                    ctx.hfm.ids.insert(key, id);
                    id
                };
                ir.clear_dests(node);
                let fc = ir.const_(fid);
                let mut args = vec![fc];
                args.extend(srcs);
                args.push(d1);
                let call = ir.new_op_multi(Inst::Call, 0, &[Some(d0)], &args);
                block.ops.insert(i, call);
                crate::opt::utils::delete_node(ir, block, i + 1);
            }
            i += 1;
        }
    }
    Ok(())
}
