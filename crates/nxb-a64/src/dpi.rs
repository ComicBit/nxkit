//! Data processing (immediate): PC-relative addressing, add and subtract, logical, move wide, bitfield, extract.

use crate::insn::{Insn, Op, Sh};
use crate::util::{bits, flags, gr, iop, mk, rop, sext};

/// The logical-immediate bit mask (`DecodeBitMasks`), or `None` for a reserved encoding.
pub(crate) fn logical_imm(n: u32, imms: u32, immr: u32, sf: bool) -> Option<u64> {
    if !sf && n == 1 {
        return None;
    }
    let v = (n << 6) | (!imms & 0x3f);
    if v == 0 {
        return None;
    }
    let len = 31 - v.leading_zeros();
    if len < 1 {
        return None;
    }
    let esize = 1u32 << len;
    let levels = esize - 1;
    let s = imms & levels;
    let r = immr & levels;
    if s == levels {
        return None;
    }
    let welem: u64 = if s + 1 >= 64 {
        u64::MAX
    } else {
        (1u64 << (s + 1)) - 1
    };
    let emask: u64 = if esize == 64 {
        u64::MAX
    } else {
        (1u64 << esize) - 1
    };
    let rot = if r == 0 {
        welem
    } else {
        ((welem >> r) | (welem << (esize - r))) & emask
    };
    let mut out = 0u64;
    let mut at = 0;
    while at < if sf { 64 } else { 32 } {
        out |= rot << at;
        at += esize;
    }
    Some(out)
}

fn imm_sh(v: i64, kind: &'static str, amt: u32) -> Op {
    Op::Imm(
        v,
        Sh {
            kind,
            amt: amt as u8,
            show0: false,
        },
    )
}

/// Decodes the data-processing (immediate) class (bits 28:26 = 100).
pub(crate) fn decode(w: u32, addr: u64) -> Option<Insn> {
    let sf = bits(w, 31, 31) == 1;
    let rd = bits(w, 4, 0);
    let rn = bits(w, 9, 5);
    match bits(w, 25, 23) {
        0 | 1 => {
            // adr / adrp
            let imm = (bits(w, 23, 5) << 2) | bits(w, 30, 29);
            let imm = sext(imm, 21);
            let (mn, t) = if sf {
                ("adrp", (addr & !0xfff).wrapping_add((imm << 12) as u64))
            } else {
                ("adr", addr.wrapping_add(imm as u64))
            };
            Some(mk(
                w,
                addr,
                mn,
                vec![rop(gr(rd, true, false)), Op::Addr(t)],
                "w.",
                false,
                false,
            ))
        }
        2 => addsub_imm(w, addr, sf, rd, rn),
        3 => None,
        4 => logical(w, addr, sf, rd, rn),
        5 => movewide(w, addr, sf, rd),
        6 => bitfield(w, addr, sf, rd, rn),
        _ => {
            // extract
            let imms = bits(w, 15, 10);
            let rm = bits(w, 20, 16);
            if bits(w, 30, 29) != 0
                || bits(w, 21, 21) != 0
                || bits(w, 22, 22) != u32::from(sf)
                || (!sf && imms >= 32)
            {
                return None;
            }
            let (d, n, m) = (gr(rd, sf, false), gr(rn, sf, false), gr(rm, sf, false));
            if rn == rm {
                Some(mk(
                    w,
                    addr,
                    "ror",
                    vec![rop(d), rop(n), iop(i64::from(imms))],
                    "wr.",
                    false,
                    false,
                ))
            } else {
                Some(mk(
                    w,
                    addr,
                    "extr",
                    vec![rop(d), rop(n), rop(m), iop(i64::from(imms))],
                    "wrr.",
                    false,
                    false,
                ))
            }
        }
    }
}

fn addsub_imm(w: u32, addr: u64, sf: bool, rd: u32, rn: u32) -> Option<Insn> {
    let op = bits(w, 30, 30);
    let s = bits(w, 29, 29);
    let sh = bits(w, 22, 22);
    let imm = i64::from(bits(w, 21, 10));
    let shift = if sh == 1 {
        imm_sh(imm, "lsl", 12)
    } else {
        iop(imm)
    };
    let n = gr(rn, sf, true);
    if s == 1 && rd == 31 {
        let mn = if op == 1 { "cmp" } else { "cmn" };
        let acc = if op == 1 || sf { "w." } else { "r." };
        return Some(flags(
            mk(w, addr, mn, vec![rop(n), shift], acc, false, false),
            false,
            true,
        ));
    }
    let d = gr(rd, sf, s == 0);
    if op == 0 && s == 0 && imm == 0 && sh == 0 && (rd == 31 || rn == 31) {
        return Some(mk(w, addr, "mov", vec![rop(d), rop(n)], "wr", false, false));
    }
    let mn = ["add", "adds", "sub", "subs"][(op * 2 + s) as usize];
    let acc = if s == 1 && op == 0 && !sf {
        "rr."
    } else {
        "wr."
    };
    Some(flags(
        mk(w, addr, mn, vec![rop(d), rop(n), shift], acc, false, false),
        false,
        s == 1,
    ))
}

fn logical(w: u32, addr: u64, sf: bool, rd: u32, rn: u32) -> Option<Insn> {
    let opc = bits(w, 30, 29);
    let imm = logical_imm(bits(w, 22, 22), bits(w, 15, 10), bits(w, 21, 16), sf)?;
    let hex = if !sf {
        iop(i64::from(imm as u32))
    } else if opc == 3 {
        iop(imm as i64)
    } else {
        Op::Hex(imm as i64)
    };
    let n = gr(rn, sf, false);
    if opc == 3 && rd == 31 {
        return Some(flags(
            mk(w, addr, "tst", vec![rop(n), hex], "w.", false, false),
            false,
            true,
        ));
    }
    let d = gr(rd, sf, opc != 3);
    if opc == 1 && rn == 31 {
        let mv = if sf {
            imm as i64
        } else {
            i64::from(imm as u32 as i32)
        };
        return Some(mk(
            w,
            addr,
            "mov",
            vec![rop(d), iop(mv)],
            "w.",
            false,
            false,
        ));
    }
    let mn = ["and", "orr", "eor", "ands"][opc as usize];
    Some(flags(
        mk(w, addr, mn, vec![rop(d), rop(n), hex], "wr.", false, false),
        false,
        opc == 3,
    ))
}

fn movewide(w: u32, addr: u64, sf: bool, rd: u32) -> Option<Insn> {
    let opc = bits(w, 30, 29);
    let hw = bits(w, 22, 21);
    if opc == 1 || (!sf && hw >= 2) {
        return None;
    }
    let imm = u64::from(bits(w, 20, 5));
    let shift = hw * 16;
    let d = gr(rd, sf, false);
    let mask = if sf { u64::MAX } else { 0xffff_ffff };
    let val = imm << shift;
    let signed = |v: u64| {
        if sf {
            v as i64
        } else {
            i64::from(v as u32 as i32)
        }
    };
    match opc {
        2 => {
            if !(val == 0 && shift != 0) {
                return Some(mk(
                    w,
                    addr,
                    "mov",
                    vec![rop(d), iop(signed(val))],
                    "w.",
                    false,
                    false,
                ));
            }
            Some(mk(
                w,
                addr,
                "movz",
                vec![rop(d), imm_sh(imm as i64, "lsl", shift)],
                "w.",
                false,
                false,
            ))
        }
        0 => {
            // movn: the alias holds when no movz could make the value
            let inv = !val & mask;
            let movz_possible = (0..4).any(|k| inv & !(0xffffu64 << (16 * k)) == 0);
            if !movz_possible {
                return Some(mk(
                    w,
                    addr,
                    "mov",
                    vec![rop(d), iop(signed(inv))],
                    "w.",
                    false,
                    false,
                ));
            }
            Some(mk(
                w,
                addr,
                "movn",
                vec![rop(d), imm_sh(imm as i64, "lsl", shift)],
                "w.",
                false,
                false,
            ))
        }
        _ => Some(mk(
            w,
            addr,
            "movk",
            vec![rop(d), imm_sh(imm as i64, "lsl", shift)],
            "b.",
            false,
            false,
        )),
    }
}

fn bitfield(w: u32, addr: u64, sf: bool, rd: u32, rn: u32) -> Option<Insn> {
    let opc = bits(w, 30, 29);
    let immr = bits(w, 21, 16);
    let imms = bits(w, 15, 10);
    if opc == 3 || bits(w, 22, 22) != u32::from(sf) || (!sf && (immr >= 32 || imms >= 32)) {
        return None;
    }
    let width = if sf { 64 } else { 32 };
    let d = gr(rd, sf, false);
    let n = gr(rn, sf, false);
    let i = |v: u32| iop(i64::from(v));
    let two = |mn| {
        mk(
            w,
            addr,
            mn,
            vec![rop(d), rop(gr(rn, false, false))],
            "br",
            false,
            false,
        )
    };
    match opc {
        0 => {
            // sbfm
            if imms == width - 1 {
                return Some(mk(
                    w,
                    addr,
                    "asr",
                    vec![rop(d), rop(n), i(immr)],
                    "br.",
                    false,
                    false,
                ));
            }
            if immr == 0 {
                match imms {
                    7 => return Some(two("sxtb")),
                    15 => return Some(two("sxth")),
                    31 if sf => return Some(two("sxtw")),
                    _ => {}
                }
            }
            if imms >= immr {
                return Some(mk(
                    w,
                    addr,
                    "sbfx",
                    vec![rop(d), rop(n), i(immr), i(imms - immr + 1)],
                    "br..",
                    false,
                    false,
                ));
            }
            Some(mk(
                w,
                addr,
                "sbfiz",
                vec![rop(d), rop(n), i(width - immr), i(imms + 1)],
                "br..",
                false,
                false,
            ))
        }
        2 => {
            // ubfm
            if imms != width - 1 && imms + 1 == immr {
                return Some(mk(
                    w,
                    addr,
                    "lsl",
                    vec![rop(d), rop(n), i(width - 1 - imms)],
                    "br.",
                    false,
                    false,
                ));
            }
            if imms == width - 1 {
                return Some(mk(
                    w,
                    addr,
                    "lsr",
                    vec![rop(d), rop(n), i(immr)],
                    "br.",
                    false,
                    false,
                ));
            }
            if immr == 0 && !sf {
                match imms {
                    7 => return Some(two("uxtb")),
                    15 => return Some(two("uxth")),
                    _ => {}
                }
            }
            if imms >= immr {
                return Some(mk(
                    w,
                    addr,
                    "ubfx",
                    vec![rop(d), rop(n), i(immr), i(imms - immr + 1)],
                    "br..",
                    false,
                    false,
                ));
            }
            Some(mk(
                w,
                addr,
                "ubfiz",
                vec![rop(d), rop(n), i(width - immr), i(imms + 1)],
                "br..",
                false,
                false,
            ))
        }
        _ => {
            // bfm
            if rn == 31 && (width - immr) % width + imms < width {
                return Some(mk(
                    w,
                    addr,
                    "bfc",
                    vec![rop(d), i((width - immr) % width), i(imms + 1)],
                    "b..",
                    false,
                    false,
                ));
            }
            if imms >= immr {
                return Some(mk(
                    w,
                    addr,
                    "bfxil",
                    vec![rop(d), rop(n), i(immr), i(imms - immr + 1)],
                    "br..",
                    false,
                    false,
                ));
            }
            Some(mk(
                w,
                addr,
                "bfi",
                vec![rop(d), rop(n), i(width - immr), i(imms + 1)],
                "br..",
                false,
                false,
            ))
        }
    }
}
