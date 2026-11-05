//! Data processing (register): logical and arithmetic with shifted or extended registers, carry, conditional
//! compare and select, one, two and three source operations.

use crate::insn::{Insn, Op, Reg};
use crate::util::{COND, bits, flags, gr, iop, mk, rop, rsh};

const SHIFTS: [&str; 4] = ["lsl", "lsr", "asr", "ror"];
const EXTS: [&str; 8] = [
    "uxtb", "uxth", "uxtw", "uxtx", "sxtb", "sxth", "sxtw", "sxtx",
];

fn shifted(r: Reg, kind: u32, amt: u32) -> Op {
    let k = SHIFTS[kind as usize];
    if kind == 0 && amt == 0 {
        rop(r)
    } else {
        rsh(r, k, amt, true)
    }
}

/// Decodes the data-processing (register) class (bits 28:25 = x101).
pub(crate) fn decode(w: u32, addr: u64) -> Option<Insn> {
    let sf = bits(w, 31, 31) == 1;
    let rd = bits(w, 4, 0);
    let rn = bits(w, 9, 5);
    let rm = bits(w, 20, 16);
    if bits(w, 28, 28) == 0 {
        return if bits(w, 24, 24) == 0 {
            logical_shifted(w, addr, sf, rd, rn, rm)
        } else if bits(w, 21, 21) == 0 {
            addsub_shifted(w, addr, sf, rd, rn, rm)
        } else {
            addsub_ext(w, addr, sf, rd, rn, rm)
        };
    }
    let op = bits(w, 30, 30);
    let s = bits(w, 29, 29);
    let (d, n, m) = (gr(rd, sf, false), gr(rn, sf, false), gr(rm, sf, false));
    match bits(w, 24, 21) {
        0 => {
            // add/subtract with carry
            if bits(w, 15, 10) != 0 {
                return None;
            }
            if op == 1 && rn == 31 {
                let mn = if s == 1 { "ngcs" } else { "ngc" };
                return Some(flags(
                    mk(w, addr, mn, vec![rop(d), rop(m)], "wr", false, false),
                    true,
                    s == 1,
                ));
            }
            let mn = ["adc", "adcs", "sbc", "sbcs"][(op * 2 + s) as usize];
            Some(flags(
                mk(
                    w,
                    addr,
                    mn,
                    vec![rop(d), rop(n), rop(m)],
                    "wrr",
                    false,
                    false,
                ),
                true,
                s == 1,
            ))
        }
        2 => {
            // conditional compare
            if s == 0 || bits(w, 10, 10) != 0 || bits(w, 4, 4) != 0 {
                return None;
            }
            let mn = if op == 1 { "ccmp" } else { "ccmn" };
            let cond = Op::Cond(COND[bits(w, 15, 12) as usize]);
            let nzcv = iop(i64::from(bits(w, 3, 0)));
            let src = if bits(w, 11, 11) == 1 {
                iop(i64::from(rm))
            } else {
                rop(m)
            };
            let acc = if bits(w, 11, 11) == 1 { "r..." } else { "rr.." };
            Some(flags(
                mk(
                    w,
                    addr,
                    mn,
                    vec![rop(n), src, nzcv, cond],
                    acc,
                    false,
                    false,
                ),
                true,
                true,
            ))
        }
        4 => csel(w, addr, sf, (rd, rn, rm), (op, s)),
        6 => {
            if s != 0 {
                return None;
            }
            if op == 0 {
                two_source(w, addr, sf, (d, n, m))
            } else {
                one_source(w, addr, sf, (d, n), rm)
            }
        }
        8..=15 => three_source(w, addr, sf, (rd, rn, rm)),
        _ => None,
    }
}

fn logical_shifted(w: u32, addr: u64, sf: bool, rd: u32, rn: u32, rm: u32) -> Option<Insn> {
    let opc = bits(w, 30, 29);
    let nbit = bits(w, 21, 21);
    let amt = bits(w, 15, 10);
    if !sf && amt >= 32 {
        return None;
    }
    let kind = bits(w, 23, 22);
    let (d, n, m) = (gr(rd, sf, false), gr(rn, sf, false), gr(rm, sf, false));
    let msh = shifted(m, kind, amt);
    if opc == 3 && nbit == 0 && rd == 31 {
        return Some(flags(
            mk(w, addr, "tst", vec![rop(n), msh], "wr", false, false),
            false,
            true,
        ));
    }
    if opc == 1 && rn == 31 {
        if nbit == 0 && amt == 0 && kind == 0 {
            return Some(mk(w, addr, "mov", vec![rop(d), rop(m)], "wr", false, false));
        }
        if nbit == 1 {
            return Some(mk(w, addr, "mvn", vec![rop(d), msh], "wr", false, false));
        }
    }
    let mn =
        [["and", "orr", "eor", "ands"], ["bic", "orn", "eon", "bics"]][nbit as usize][opc as usize];
    Some(flags(
        mk(w, addr, mn, vec![rop(d), rop(n), msh], "wrr", false, false),
        false,
        opc == 3,
    ))
}

fn addsub_shifted(w: u32, addr: u64, sf: bool, rd: u32, rn: u32, rm: u32) -> Option<Insn> {
    let op = bits(w, 30, 30);
    let s = bits(w, 29, 29);
    let kind = bits(w, 23, 22);
    let amt = bits(w, 15, 10);
    if kind == 3 || (!sf && amt >= 32) {
        return None;
    }
    let (d, n, m) = (gr(rd, sf, false), gr(rn, sf, false), gr(rm, sf, false));
    let msh = shifted(m, kind, amt);
    if s == 1 && rd == 31 {
        let mn = if op == 1 { "cmp" } else { "cmn" };
        return Some(flags(
            mk(w, addr, mn, vec![rop(n), msh], "wr", false, false),
            false,
            true,
        ));
    }
    if op == 1 && rn == 31 {
        let mn = if s == 1 { "negs" } else { "neg" };
        return Some(flags(
            mk(w, addr, mn, vec![rop(d), msh], "wr", false, false),
            false,
            s == 1,
        ));
    }
    let mn = ["add", "adds", "sub", "subs"][(op * 2 + s) as usize];
    Some(flags(
        mk(w, addr, mn, vec![rop(d), rop(n), msh], "wrr", false, false),
        false,
        s == 1,
    ))
}

fn addsub_ext(w: u32, addr: u64, sf: bool, rd: u32, rn: u32, rm: u32) -> Option<Insn> {
    let op = bits(w, 30, 30);
    let s = bits(w, 29, 29);
    let opt = bits(w, 23, 22);
    let ext = bits(w, 15, 13);
    let amt = bits(w, 12, 10);
    if opt != 0 || amt > 4 {
        return None;
    }
    let n = gr(rn, sf, true);
    let xm = sf && ext & 3 == 3;
    let m = gr(rm, xm, false);
    let lsl_form = (n == Reg::Sp && ext == 3) || (n == Reg::Wsp && ext == 2);
    let d = gr(rd, sf, s == 0);
    let lsl_form = lsl_form || (d == Reg::Sp && ext == 3) || (d == Reg::Wsp && ext == 2);
    let msh = if lsl_form {
        if amt == 0 {
            rop(m)
        } else {
            rsh(m, "lsl", amt, false)
        }
    } else {
        rsh(m, EXTS[ext as usize], amt, false)
    };
    if s == 1 && rd == 31 {
        let mn = if op == 1 { "cmp" } else { "cmn" };
        let lsl = (n == Reg::Sp && ext == 3) || (n == Reg::Wsp && ext == 2);
        let msh = if lsl {
            if amt == 0 {
                rop(m)
            } else {
                rsh(m, "lsl", amt, false)
            }
        } else {
            rsh(m, EXTS[ext as usize], amt, false)
        };
        return Some(flags(
            mk(w, addr, mn, vec![rop(n), msh], "wr", false, false),
            false,
            true,
        ));
    }
    let mn = ["add", "adds", "sub", "subs"][(op * 2 + s) as usize];
    Some(flags(
        mk(w, addr, mn, vec![rop(d), rop(n), msh], "wrr", false, false),
        false,
        s == 1,
    ))
}

fn csel(w: u32, addr: u64, sf: bool, regs: (u32, u32, u32), os: (u32, u32)) -> Option<Insn> {
    let (rd, rn, rm) = regs;
    let (op, s) = os;
    let o2 = bits(w, 11, 10);
    if s != 0 || o2 > 1 {
        return None;
    }
    let cond = bits(w, 15, 12);
    let (d, n, m) = (gr(rd, sf, false), gr(rn, sf, false), gr(rm, sf, false));
    let inv = Op::Cond(COND[(cond ^ 1) as usize]);
    let c = Op::Cond(COND[cond as usize]);
    let alias_ok = cond < 14;
    let same = rn == rm;
    let kind = op * 2 + o2;
    let plain = |mn| {
        flags(
            mk(
                w,
                addr,
                mn,
                vec![rop(d), rop(n), rop(m), c.clone()],
                "wrr.",
                false,
                false,
            ),
            true,
            false,
        )
    };
    Some(match kind {
        0 => plain("csel"),
        1 if alias_ok && same && rn == 31 => flags(
            mk(w, addr, "cset", vec![rop(d), inv], "w.", false, false),
            true,
            false,
        ),
        1 if alias_ok && same => flags(
            mk(
                w,
                addr,
                "cinc",
                vec![rop(d), rop(n), inv],
                "wr.",
                false,
                false,
            ),
            true,
            false,
        ),
        1 => plain("csinc"),
        2 if alias_ok && same && rn == 31 => flags(
            mk(w, addr, "csetm", vec![rop(d), inv], "w.", false, false),
            true,
            false,
        ),
        2 if alias_ok && same => flags(
            mk(
                w,
                addr,
                "cinv",
                vec![rop(d), rop(n), inv],
                "wr.",
                false,
                false,
            ),
            true,
            false,
        ),
        2 => plain("csinv"),
        _ if alias_ok && same => flags(
            mk(
                w,
                addr,
                "cneg",
                vec![rop(d), rop(n), inv],
                "wr.",
                false,
                false,
            ),
            true,
            false,
        ),
        _ => plain("csneg"),
    })
}

fn two_source(w: u32, addr: u64, sf: bool, regs: (Reg, Reg, Reg)) -> Option<Insn> {
    let (d, n, m) = regs;
    let opc = bits(w, 15, 10);
    let mn = match opc {
        2 => "udiv",
        3 => "sdiv",
        8 => "lsl",
        9 => "lsr",
        10 => "asr",
        11 => "ror",
        12 if sf => {
            let m = gr(bits(w, 20, 16), true, true);
            return Some(mk(
                w,
                addr,
                "pacga",
                vec![rop(d), rop(n), rop(m)],
                "br.",
                false,
                false,
            ));
        }
        16..=23 => {
            let k = opc & 7;
            let c = k >> 2;
            let sz = k & 3;
            if (sz == 3) != sf {
                return None;
            }
            let names = [
                ["crc32b", "crc32h", "crc32w", "crc32x"],
                ["crc32cb", "crc32ch", "crc32cw", "crc32cx"],
            ];
            // the data register is x only for the 64-bit forms
            let m = gr(bits(w, 20, 16), sf, false);
            return Some(mk(
                w,
                addr,
                names[c as usize][sz as usize],
                vec![
                    rop(gr(bits(w, 4, 0), false, false)),
                    rop(gr(bits(w, 9, 5), false, false)),
                    rop(m),
                ],
                "wrr",
                false,
                false,
            ));
        }
        _ => return None,
    };
    Some(mk(
        w,
        addr,
        mn,
        vec![rop(d), rop(n), rop(m)],
        "wrr",
        false,
        false,
    ))
}

fn one_source(w: u32, addr: u64, sf: bool, regs: (Reg, Reg), opcode2: u32) -> Option<Insn> {
    let (d, n) = regs;
    let opc = bits(w, 15, 10);
    if opcode2 == 1 {
        return pac(w, addr, sf, opc);
    }
    if opcode2 != 0 {
        return None;
    }
    let mn = match (opc, sf) {
        (0, _) => "rbit",
        (1, _) => "rev16",
        (2, false) => "rev",
        (2, true) => "rev32",
        (3, true) => "rev",
        (4, _) => "clz",
        (5, _) => "cls",
        _ => return None,
    };
    Some(mk(w, addr, mn, vec![rop(d), rop(n)], "wr", false, false))
}

/// Pointer authentication, one source: `pacia`, `autib`, `paciza`, `xpaci` and friends.
fn pac(w: u32, addr: u64, sf: bool, opc: u32) -> Option<Insn> {
    if !sf {
        return None;
    }
    let rd = bits(w, 4, 0);
    let rn = bits(w, 9, 5);
    let d = gr(rd, true, false);
    let names = [
        "pacia", "pacib", "pacda", "pacdb", "autia", "autib", "autda", "autdb", "paciza", "pacizb",
        "pacdza", "pacdzb", "autiza", "autizb", "autdza", "autdzb", "xpaci", "xpacd",
    ];
    match opc {
        0..=7 => Some(mk(
            w,
            addr,
            names[opc as usize],
            vec![rop(d), rop(gr(rn, true, true))],
            "br",
            false,
            false,
        )),
        8..=15 if rn == 31 => Some(mk(
            w,
            addr,
            names[opc as usize],
            vec![rop(d)],
            "b",
            false,
            false,
        )),
        16 | 17 if rn == 31 => Some(mk(
            w,
            addr,
            names[opc as usize],
            vec![rop(d)],
            "b",
            false,
            false,
        )),
        _ => None,
    }
}

fn three_source(w: u32, addr: u64, sf: bool, regs: (u32, u32, u32)) -> Option<Insn> {
    let (rd, rn, rm) = regs;
    let ra = bits(w, 14, 10);
    let o0 = bits(w, 15, 15);
    let op31 = bits(w, 23, 21);
    if bits(w, 30, 29) != 0 {
        return None;
    }
    let x = |n| gr(n, true, false);
    let wr = |n| gr(n, false, false);
    match (op31, o0) {
        (0, _) => {
            let (d, n, m, a) = (
                gr(rd, sf, false),
                gr(rn, sf, false),
                gr(rm, sf, false),
                gr(ra, sf, false),
            );
            if ra == 31 {
                let mn = if o0 == 0 { "mul" } else { "mneg" };
                return Some(mk(
                    w,
                    addr,
                    mn,
                    vec![rop(d), rop(n), rop(m)],
                    "wrr",
                    false,
                    false,
                ));
            }
            let mn = if o0 == 0 { "madd" } else { "msub" };
            Some(mk(
                w,
                addr,
                mn,
                vec![rop(d), rop(n), rop(m), rop(a)],
                "wrrr",
                false,
                false,
            ))
        }
        (1 | 5, _) if sf => {
            let (names, alias) = if op31 == 1 {
                (["smaddl", "smsubl"], ["smull", "smnegl"])
            } else {
                (["umaddl", "umsubl"], ["umull", "umnegl"])
            };
            if ra == 31 {
                return Some(mk(
                    w,
                    addr,
                    alias[o0 as usize],
                    vec![rop(x(rd)), rop(wr(rn)), rop(wr(rm))],
                    "wrr",
                    false,
                    false,
                ));
            }
            Some(mk(
                w,
                addr,
                names[o0 as usize],
                vec![rop(x(rd)), rop(wr(rn)), rop(wr(rm)), rop(x(ra))],
                "wrrr",
                false,
                false,
            ))
        }
        (2 | 6, 0) if sf && ra == 31 => {
            let mn = if op31 == 2 { "smulh" } else { "umulh" };
            Some(mk(
                w,
                addr,
                mn,
                vec![rop(x(rd)), rop(x(rn)), rop(x(rm))],
                "wrr",
                false,
                false,
            ))
        }
        _ => None,
    }
}
