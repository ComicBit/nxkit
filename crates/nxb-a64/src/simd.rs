//! Advanced SIMD and floating-point data processing. Only what the rewriter needs is modelled: whether the word
//! decodes, the general registers it names and how it reads and writes them, and the condition flags it uses.

use crate::insn::{Fk, Insn, Op};
use crate::util::{bits, flags, fr, gr, mk, rop};

/// An instruction whose register operands sit in the usual fields: `d` bits 4:0, `n` bits 9:5, `m` bits 20:16,
/// `a` bits 14:10, in the order of `pat` (the first one is written, the others read).
fn vo(w: u32, addr: u64, mn: &'static str, pat: &str) -> Insn {
    let ops = pat
        .chars()
        .map(|c| {
            let n = match c {
                'd' => bits(w, 4, 0),
                'n' => bits(w, 9, 5),
                'm' => bits(w, 20, 16),
                _ => bits(w, 14, 10),
            };
            rop(fr(Fk::V, n))
        })
        .collect();
    let acc: String = pat
        .chars()
        .enumerate()
        .map(|(i, _)| {
            if i == 0 && pat.starts_with('d') {
                'w'
            } else {
                'r'
            }
        })
        .collect();
    mk(w, addr, mn, ops, &acc, false, true)
}

/// The register fields of an Advanced SIMD scalar instruction.
fn scalar_simd(w: u32, addr: u64) -> Insn {
    let b = |hi, lo| bits(w, hi, lo);
    let pat = if b(28, 24) == 0b11111 {
        if b(10, 10) == 0 { "dnm" } else { "dn" }
    } else if b(21, 21) == 1 {
        match b(11, 10) {
            0 | 1 | 3 => "dnm",
            _ => "dn",
        }
    } else if b(10, 10) == 1 {
        "dn"
    } else {
        "dnm"
    };
    vo(w, addr, "simd", pat)
}

/// Decodes the group with bits 28:25 = x111.
pub(crate) fn decode(w: u32, addr: u64) -> Option<Insn> {
    let b = |hi, lo| bits(w, hi, lo);
    match b(28, 24) {
        0b11110 | 0b11111 => {
            if b(30, 30) == 1 {
                // scalar Advanced SIMD (and the scalar crypto group)
                return if b(31, 31) == 0 {
                    Some(scalar_simd(w, addr))
                } else {
                    None
                };
            }
            scalar_fp(w, addr)
        }
        0b01110 | 0b01111 => {
            if b(31, 31) != 0 {
                return None;
            }
            if b(28, 24) == 0b01110 {
                vector(w, addr)
            } else {
                Some(vo(
                    w,
                    addr,
                    "simd",
                    if b(10, 10) == 0 {
                        "dnm"
                    } else if b(22, 19) == 0 {
                        "d"
                    } else {
                        "dn"
                    },
                ))
            }
        }
        _ => None,
    }
}

fn vector(w: u32, addr: u64) -> Option<Insn> {
    let b = |hi, lo| bits(w, hi, lo);
    if b(23, 21) == 0 && b(15, 15) == 0 && b(10, 10) == 1 {
        return copy(w, addr);
    }
    if b(21, 21) == 0 {
        // permute, extract, table lookup, three-same-extra and the FP16 forms
        if b(29, 29) == 0 && b(23, 22) == 0 && b(15, 15) == 0 && b(11, 10) == 0 {
            // tbl / tbx: a list of registers, then the index
            let len = b(14, 13) + 1;
            let mut ops: Vec<Op> = vec![rop(fr(Fk::V, b(4, 0)))];
            ops.extend((0..len).map(|i| rop(fr(Fk::V, (b(9, 5) + i) % 32))));
            ops.push(rop(fr(Fk::V, b(20, 16))));
            return Some(mk(w, addr, "tbl", ops, "wrrrrr", false, true));
        }
        return Some(vo(w, addr, "simd", "dnm"));
    }
    match b(11, 10) {
        0 | 1 | 3 => Some(vo(w, addr, "simd", "dnm")),
        _ => Some(vo(w, addr, "simd", "dn")),
    }
}

fn copy(w: u32, addr: u64) -> Option<Insn> {
    let q = bits(w, 30, 30);
    let op = bits(w, 29, 29);
    let imm5 = bits(w, 20, 16);
    let imm4 = bits(w, 14, 11);
    let rn = bits(w, 9, 5);
    let rd = bits(w, 4, 0);
    if imm5 & 0xf == 0 {
        return None;
    }
    let sz = imm5.trailing_zeros(); // 0 b, 1 h, 2 s, 3 d
    if op == 1 {
        // ins (element): the high bit of imm4 must be zero for the used element size, rest generic
        return if q == 1 {
            Some(vo(w, addr, "mov", "dn"))
        } else {
            None
        };
    }
    let v = fr(Fk::V, rd);
    match imm4 {
        0 | 1 => {
            if imm4 == 0 {
                return Some(vo(w, addr, "dup", "dn"));
            }
            if sz == 3 && q == 0 {
                return None;
            }
            let g = gr(rn, sz == 3, false);
            Some(mk(w, addr, "dup", vec![rop(v), rop(g)], "wr", false, true))
        }
        3 => {
            if q == 0 {
                return None;
            }
            let g = gr(rn, sz == 3, false);
            Some(mk(w, addr, "mov", vec![rop(v), rop(g)], "br", false, true))
        }
        5 => {
            if sz > 2 || (sz == 2 && q == 0) {
                return None;
            }
            let g = gr(rd, q == 1, false);
            Some(mk(
                w,
                addr,
                "smov",
                vec![rop(g), rop(fr(Fk::V, rn))],
                if imm5 >> (sz + 1) == 0 { ".." } else { "wr" },
                false,
                true,
            ))
        }
        7 => {
            let ok = sz < 2 && q == 0 || sz == 2 && q == 0 || sz == 3 && q == 1;
            if !ok {
                return None;
            }
            let g = gr(rd, q == 1, false);
            let mn = if sz >= 2 { "mov" } else { "umov" };
            let acc = if imm5 == 2 { ".." } else { "wr" };
            Some(mk(
                w,
                addr,
                mn,
                vec![rop(g), rop(fr(Fk::V, rn))],
                acc,
                false,
                true,
            ))
        }
        _ => None,
    }
}

fn scalar_fp(w: u32, addr: u64) -> Option<Insn> {
    let b = |hi, lo| bits(w, hi, lo);
    let sf = b(31, 31) == 1;
    if b(29, 29) != 0 {
        return None;
    }
    let ty = b(23, 22);
    let rn = b(9, 5);
    let rd = b(4, 0);
    if b(28, 24) == 0b11111 {
        return if !sf && ty != 2 {
            Some(vo(w, addr, "fmadd", "dnma"))
        } else {
            None
        };
    }
    if b(21, 21) == 0 {
        // fixed-point conversion
        let (rmode, opcode) = (b(20, 19), b(18, 16));
        if ty == 2 || (!sf && b(15, 15) == 0) {
            return None;
        }
        let fk = fkind(ty)?;
        return match (rmode, opcode) {
            (0, 2) | (0, 3) => {
                let mn = if opcode == 2 { "scvtf" } else { "ucvtf" };
                Some(mk(
                    w,
                    addr,
                    mn,
                    vec![rop(fr(fk, rd)), rop(gr(rn, sf, false))],
                    if fk == Fk::H { ".." } else { "wr" },
                    false,
                    true,
                ))
            }
            (3, 0) | (3, 1) => {
                let mn = if opcode == 0 { "fcvtzs" } else { "fcvtzu" };
                Some(mk(
                    w,
                    addr,
                    mn,
                    vec![rop(gr(rd, sf, false)), rop(fr(fk, rn))],
                    if fk == Fk::H { ".." } else { "wr" },
                    false,
                    true,
                ))
            }
            _ => None,
        };
    }
    if sf && b(14, 10) != 0 {
        return None;
    }
    if b(14, 10) == 0b10000 {
        return if !sf && ty != 2 {
            Some(vo(w, addr, "fmov", "dn"))
        } else {
            None
        };
    }
    if b(13, 10) == 0b1000 {
        if sf || ty == 2 || b(15, 14) != 0 || b(2, 0) != 0 {
            return None;
        }
        let mn = if b(4, 4) == 1 { "fcmpe" } else { "fcmp" };
        let pat = if b(3, 3) == 1 { "n" } else { "nm" };
        return Some(flags(vo(w, addr, mn, pat), false, true));
    }
    if b(12, 10) == 0b100 {
        return if !sf && ty != 2 && b(9, 5) == 0 {
            Some(vo(w, addr, "fmov", "d"))
        } else {
            None
        };
    }
    match b(11, 10) {
        1 => {
            if sf || ty == 2 {
                return None;
            }
            let mn = if b(4, 4) == 1 { "fccmpe" } else { "fccmp" };
            Some(flags(vo(w, addr, mn, "nm"), ty != 3, true))
        }
        2 => {
            if sf || ty == 2 || b(15, 12) > 8 {
                return None;
            }
            Some(vo(w, addr, "fadd", "dnm"))
        }
        3 => {
            if sf || ty == 2 {
                return None;
            }
            Some(flags(vo(w, addr, "fcsel", "dnm"), ty != 3, false))
        }
        _ => fp_int(w, addr, (sf, ty, rn, rd)),
    }
}

fn fkind(ty: u32) -> Option<Fk> {
    match ty {
        0 => Some(Fk::S),
        1 => Some(Fk::D),
        3 => Some(Fk::H),
        _ => None,
    }
}

fn fp_int(w: u32, addr: u64, f: (bool, u32, u32, u32)) -> Option<Insn> {
    let (sf, ty, rn, rd) = f;
    if bits(w, 15, 10) != 0 {
        return None;
    }
    let rmode = bits(w, 20, 19);
    let opcode = bits(w, 18, 16);
    // type 10 (the upper half of a 128-bit register) only exists for fmov with rmode 01 on 64-bit registers
    if ty == 2 {
        if !(sf && rmode == 1 && opcode >= 6) {
            return None;
        }
        let to_gpr = opcode == 6;
        let v = fr(Fk::V, if to_gpr { rn } else { rd });
        let g = gr(if to_gpr { rd } else { rn }, true, false);
        return Some(if to_gpr {
            mk(w, addr, "fmov", vec![rop(g), rop(v)], "wr", false, true)
        } else {
            mk(w, addr, "fmov", vec![rop(v), rop(g)], "br", false, true)
        });
    }
    let fk = fkind(ty)?;
    let (mn, to_gpr): (&'static str, bool) = match (rmode, opcode) {
        (0, 0) => ("fcvtns", true),
        (0, 1) => ("fcvtnu", true),
        (0, 2) => ("scvtf", false),
        (0, 3) => ("ucvtf", false),
        (0, 4) => ("fcvtas", true),
        (0, 5) => ("fcvtau", true),
        (0, 6) => ("fmov", true),
        (0, 7) => ("fmov", false),
        (1, 0) => ("fcvtps", true),
        (1, 1) => ("fcvtpu", true),
        (2, 0) => ("fcvtms", true),
        (2, 1) => ("fcvtmu", true),
        (3, 0) => ("fcvtzs", true),
        (3, 1) => ("fcvtzu", true),
        (3, 6) if !sf && ty == 1 => ("fjcvtzs", true),
        _ => return None,
    };
    if mn == "fmov" {
        // sf and type must agree: w with single, x with double, either with half
        let ok = (!sf && (ty == 0 || ty == 3)) || (sf && (ty == 1 || ty == 3));
        if !ok {
            return None;
        }
    }
    if to_gpr {
        Some(mk(
            w,
            addr,
            mn,
            vec![rop(gr(rd, sf, false)), rop(fr(fk, rn))],
            if fk == Fk::H { ".." } else { "wr" },
            false,
            true,
        ))
    } else {
        Some(mk(
            w,
            addr,
            mn,
            vec![rop(fr(fk, rd)), rop(gr(rn, sf, false))],
            if fk == Fk::H { ".." } else { "wr" },
            false,
            true,
        ))
    }
}
