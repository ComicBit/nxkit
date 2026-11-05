//! Branches, exception generation and system instructions.

use crate::insn::{Insn, Op, Reg};
use crate::util::{BCOND, bits, flags, gr, iop, mk, rop, sext};

/// Decodes the branch, exception and system class (bits 28:26 = 101).
pub(crate) fn decode(w: u32, addr: u64) -> Option<Insn> {
    let op0 = bits(w, 31, 29);
    let rt = bits(w, 4, 0);
    match op0 {
        0 | 4 => {
            let t = addr.wrapping_add((sext(bits(w, 25, 0), 26) << 2) as u64);
            let (mn, wr) = if op0 == 0 { ("b", false) } else { ("bl", true) };
            let mut i = mk(w, addr, mn, vec![Op::Addr(t)], ".", false, false);
            if wr {
                i.writes.push(Reg::X(30));
            }
            Some(i)
        }
        1 | 5 => {
            let sf = op0 == 5;
            if bits(w, 25, 25) == 0 {
                let t = addr.wrapping_add((sext(bits(w, 23, 5), 19) << 2) as u64);
                let mn = if bits(w, 24, 24) == 1 { "cbnz" } else { "cbz" };
                return Some(flags(
                    mk(
                        w,
                        addr,
                        mn,
                        vec![rop(gr(rt, sf, false)), Op::Addr(t)],
                        "r.",
                        false,
                        false,
                    ),
                    true,
                    false,
                ));
            }
            let t = addr.wrapping_add((sext(bits(w, 18, 5), 14) << 2) as u64);
            let b5 = bits(w, 31, 31);
            let mn = if bits(w, 24, 24) == 1 { "tbnz" } else { "tbz" };
            let bit = i64::from((b5 << 5) | bits(w, 23, 19));
            Some(mk(
                w,
                addr,
                mn,
                vec![rop(gr(rt, b5 == 1, false)), iop(bit), Op::Addr(t)],
                "r..",
                false,
                false,
            ))
        }
        2 => {
            if bits(w, 25, 25) != 0 || bits(w, 24, 24) != 0 || bits(w, 4, 4) != 0 {
                return None;
            }
            let t = addr.wrapping_add((sext(bits(w, 23, 5), 19) << 2) as u64);
            let mut i = mk(
                w,
                addr,
                BCOND[bits(w, 3, 0) as usize],
                vec![Op::Addr(t)],
                ".",
                false,
                false,
            );
            i.reads.push(Reg::Nzcv);
            Some(i)
        }
        6 => system_or_reg(w, addr),
        _ => None,
    }
}

fn system_or_reg(w: u32, addr: u64) -> Option<Insn> {
    if bits(w, 25, 25) == 1 {
        return branch_reg(w, addr);
    }
    if bits(w, 24, 24) == 1 {
        return if bits(w, 23, 22) == 0 {
            system(w, addr)
        } else {
            None
        };
    }
    // exception generation: 1101 0100 opc imm16 op2 LL
    let opc = bits(w, 23, 21);
    let ll = bits(w, 1, 0);
    let imm = i64::from(bits(w, 20, 5));
    let mn = match (opc, bits(w, 4, 2), ll) {
        (0, 0, 1) => "svc",
        (0, 0, 2) => "hvc",
        (0, 0, 3) => "smc",
        (1, 0, 0) => "brk",
        (2, 0, 0) => "hlt",
        (5, 0, 1) => "dcps1",
        (5, 0, 2) => "dcps2",
        (5, 0, 3) => "dcps3",
        _ => return None,
    };
    let mut i = mk(w, addr, mn, vec![iop(imm)], ".", false, false);
    if mn == "svc" {
        i.writes.push(Reg::X(30));
    }
    Some(i)
}

fn branch_reg(w: u32, addr: u64) -> Option<Insn> {
    let opc = bits(w, 24, 21);
    let rn = bits(w, 9, 5);
    if opc == 2
        && bits(w, 20, 16) == 0b11111
        && matches!(bits(w, 15, 10), 2 | 3)
        && rn == 31
        && bits(w, 4, 0) == 31
    {
        let mn = if bits(w, 10, 10) == 0 {
            "retaa"
        } else {
            "retab"
        };
        let mut i = mk(w, addr, mn, vec![], "", false, false);
        i.reads.extend([Reg::X(30), Reg::Sp]);
        return Some(i);
    }
    if bits(w, 20, 16) != 0b11111 || bits(w, 15, 10) != 0 || bits(w, 4, 0) != 0 {
        return None;
    }
    let x = gr(rn, true, false);
    match opc {
        0 => Some(mk(w, addr, "br", vec![rop(x)], "r", false, false)),
        1 => {
            let mut i = mk(w, addr, "blr", vec![rop(x)], "r", false, false);
            i.writes.push(Reg::X(30));
            Some(i)
        }
        2 => {
            if rn == 30 {
                Some(mk(w, addr, "ret", vec![], "", false, false))
            } else {
                Some(mk(w, addr, "ret", vec![rop(x)], "r", false, false))
            }
        }
        4 if rn == 31 => Some(mk(w, addr, "eret", vec![], "", false, false)),
        5 if rn == 31 => Some(mk(w, addr, "drps", vec![], "", false, false)),
        _ => None,
    }
}

fn system(w: u32, addr: u64) -> Option<Insn> {
    let l = bits(w, 21, 21);
    let op0 = bits(w, 20, 19);
    let op1 = bits(w, 18, 16);
    let crn = bits(w, 15, 12);
    let crm = bits(w, 11, 8);
    let op2 = bits(w, 7, 5);
    let rt = bits(w, 4, 0);
    let x = gr(rt, true, false);
    if l == 0 && op0 == 0 {
        if crn == 2 && op1 == 3 && rt == 31 {
            // hints
            let imm = (crm << 3) | op2;
            let mn = match imm {
                0 => "nop",
                1 => "yield",
                2 => "wfe",
                3 => "wfi",
                4 => "sev",
                5 => "sevl",
                7 => "xpaclri",
                8 => "pacia1716",
                10 => "pacib1716",
                12 => "autia1716",
                14 => "autib1716",
                20 => "csdb",
                24 => "paciaz",
                25 => "paciasp",
                26 => "pacibz",
                27 => "pacibsp",
                28 => "autiaz",
                29 => "autiasp",
                30 => "autibz",
                31 => "autibsp",
                _ => "hint",
            };
            let ops = if mn == "hint" {
                vec![iop(i64::from(imm))]
            } else {
                vec![]
            };
            let mut i = mk(w, addr, mn, ops, ".", false, false);
            match imm {
                7 => {
                    i.reads.push(Reg::X(30));
                    i.writes.push(Reg::X(30));
                }
                8 | 10 | 12 | 14 => {
                    i.reads.extend([Reg::X(16), Reg::X(17)]);
                    i.writes.push(Reg::X(17));
                }
                24 | 26 | 28 | 30 => {
                    i.reads.push(Reg::X(30));
                    i.writes.push(Reg::X(30));
                }
                25 | 27 | 29 | 31 => {
                    i.reads.extend([Reg::X(30), Reg::Sp]);
                    i.writes.push(Reg::X(30));
                }
                _ => {}
            }
            return Some(i);
        }
        if crn == 3 && op1 == 3 {
            let mn = match op2 {
                2 => "clrex",
                4 => "dsb",
                5 => "dmb",
                6 => "isb",
                _ => return None,
            };
            let ops = if mn == "clrex" && crm == 15 {
                vec![]
            } else {
                vec![Op::Name(format!("#{crm}"))]
            };
            return Some(mk(w, addr, mn, ops, ".", false, false));
        }
        if crn == 4 {
            return Some(mk(
                w,
                addr,
                "msr",
                vec![Op::Name("pstate".into()), iop(i64::from(crm))],
                ".",
                false,
                false,
            ));
        }
        return None;
    }
    if op0 == 0 {
        return None;
    }
    if op0 == 1 {
        // sys / sysl
        if l == 1 {
            return Some(mk(
                w,
                addr,
                "sysl",
                vec![rop(x), iop(i64::from(op1)), Op::Other],
                "b..",
                false,
                false,
            ));
        }
        let mn = match (op1, crn, crm, op2) {
            (0, 7, 5, 0) | (0, 7, 1, 0) | (3, 7, 5, 1) => "ic",
            (3, 7, 4, 1)
            | (0, 7, 6, 1)
            | (0, 7, 6, 2)
            | (3, 7, 10, 1)
            | (0, 7, 10, 2)
            | (3, 7, 11, 1)
            | (3, 7, 14, 1)
            | (0, 7, 14, 2) => "dc",
            _ if crn == 7 && matches!(crm, 8 | 9) => "at",
            _ if crn == 8 => "tlbi",
            _ => "sys",
        };
        let acc = if mn == "sys" { ".r" } else { ".." };
        return Some(mk(
            w,
            addr,
            mn,
            vec![Op::Name("op".into()), rop(x)],
            acc,
            false,
            false,
        ));
    }
    // msr / mrs (register)
    let name = Op::Name(format!("s{}_{}_c{}_c{}_{}", op0, op1, crn, crm, op2));
    if l == 1 {
        Some(mk(w, addr, "mrs", vec![rop(x), name], "w.", false, false))
    } else {
        Some(mk(w, addr, "msr", vec![name, rop(x)], ".r", false, false))
    }
}
