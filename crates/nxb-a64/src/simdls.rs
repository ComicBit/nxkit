//! Advanced SIMD load and store structures (`ld1` to `ld4`, `st1` to `st4`, `ld1r` and friends).

use crate::insn::{Fk, Insn, Op, Reg};
use crate::util::{bits, fr, gr, iop, mk, rop};

/// Decodes `0 Q 001100/001101 ...`.
pub(crate) fn decode(w: u32, addr: u64) -> Option<Insn> {
    if bits(w, 31, 31) != 0 {
        return None;
    }
    let q = bits(w, 30, 30);
    let post = bits(w, 23, 23) == 1;
    let l = bits(w, 22, 22);
    let rn = bits(w, 9, 5);
    let rt = bits(w, 4, 0);
    let rm = bits(w, 20, 16);
    let base = gr(rn, true, true);
    let single = bits(w, 24, 24) == 1;
    let (mn, nregs, bytes): (&'static str, u32, u32) = if !single {
        if bits(w, 21, 21) != 0 || (!post && rm != 0) {
            return None;
        }
        let size = bits(w, 11, 10);
        let (n, sel) = match bits(w, 15, 12) {
            0 => (4, 4),
            2 => (4, 1),
            4 => (3, 3),
            6 => (3, 1),
            7 => (1, 1),
            8 => (2, 2),
            10 => (2, 1),
            _ => return None,
        };
        if sel > 1 && size == 3 && q == 0 {
            return None;
        }
        let names = if l == 1 {
            ["", "ld1", "ld2", "ld3", "ld4"]
        } else {
            ["", "st1", "st2", "st3", "st4"]
        };
        (names[sel as usize], n, n * if q == 1 { 16 } else { 8 })
    } else {
        let opc = bits(w, 15, 13);
        let r = bits(w, 21, 21);
        let s = bits(w, 12, 12);
        let size = bits(w, 11, 10);
        if !post && rm != 0 {
            return None;
        }
        let sel = ((opc & 1) << 1 | r) + 1;
        let names = if l == 1 {
            ["", "ld1", "ld2", "ld3", "ld4"]
        } else {
            ["", "st1", "st2", "st3", "st4"]
        };
        let names_r = ["", "ld1r", "ld2r", "ld3r", "ld4r"];
        if opc >= 6 {
            if l == 0 || s != 0 || opc == 7 && false {
                return None;
            }
            (names_r[sel as usize], sel, sel << size)
        } else {
            let esz = match opc >> 1 {
                0 => 0,
                1 => {
                    if size & 1 != 0 {
                        return None;
                    }
                    1
                }
                _ => match size {
                    0 => 2,
                    1 if s == 0 => 3,
                    _ => return None,
                },
            };
            (names[sel as usize], sel, sel << esz)
        }
    };
    let mut ops: Vec<Op> = (0..nregs).map(|i| rop(fr(Fk::V, (rt + i) % 32))).collect();
    let n = ops.len();
    let mut acc = if l == 1 { "b".repeat(n) } else { "r".repeat(n) };
    ops.push(Op::Mem {
        base,
        index: None,
        disp: 0,
        wb: crate::insn::Wb::None,
    });
    acc.push('.');
    if post {
        if rm == 31 {
            ops.push(iop(i64::from(bytes)));
            acc.push('.');
        } else {
            ops.push(rop(Reg::X(rm as u8)));
            acc.push(if single && nregs == 1 {
                if l == 1 { 'r' } else { 'b' }
            } else {
                '.'
            });
        }
    }
    Some(mk(w, addr, mn, ops, &acc, post, true))
}
