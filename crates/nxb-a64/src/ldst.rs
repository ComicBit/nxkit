//! Loads and stores: single registers, pairs, exclusives, atomics and literal loads. Advanced SIMD
//! structure loads and stores are in `simdls`.

use crate::insn::{Fk, Insn, Op, Reg, Sh, Wb};
use crate::util::{bits, fr, gr, iop, mk, rop, sext};

fn memop(base: Reg, disp: i64, pre: bool) -> Op {
    Op::Mem {
        base,
        index: None,
        disp,
        wb: if pre { Wb::Pre } else { Wb::None },
    }
}

/// Decodes the load and store class (bits 28 = 0 and 26.. as dispatched by `decode`).
pub(crate) fn decode(w: u32, addr: u64) -> Option<Insn> {
    let b = |hi, lo| bits(w, hi, lo);
    if w & 0x3fff_fc00 == 0x38bf_c000 {
        // ldapr, ldaprb, ldaprh (RCpc)
        let size = b(31, 30);
        let mn = ["ldaprb", "ldaprh", "ldapr", "ldapr"][size as usize];
        let t = gr(b(4, 0), size == 3, false);
        let m = memop(gr(b(9, 5), true, true), 0, false);
        return Some(mk(w, addr, mn, vec![rop(t), m], "w.", false, false));
    }
    if b(31, 24) == 0xd9 && b(21, 21) == 1 {
        return tags(w, addr);
    }
    if b(29, 24) == 0b001000 && b(26, 26) == 0 {
        return exclusive(w, addr);
    }
    if b(29, 27) == 0b011 && b(25, 24) == 0 {
        return literal(w, addr);
    }
    if b(29, 27) == 0b101 {
        return pair(w, addr);
    }
    if b(29, 27) == 0b111 {
        return single(w, addr);
    }
    if b(29, 27) == 0b001 && b(26, 26) == 1 {
        return crate::simdls::decode(w, addr);
    }
    None
}

fn tags(w: u32, addr: u64) -> Option<Insn> {
    let opc = bits(w, 23, 22);
    let op2 = bits(w, 11, 10);
    let imm = sext(bits(w, 20, 12), 9) * 16;
    let t = gr(bits(w, 4, 0), true, op2 != 0 || opc == 0 && false);
    let base = gr(bits(w, 9, 5), true, true);
    if op2 == 0 {
        if opc != 1 {
            return None;
        }
        return Some(mk(
            w,
            addr,
            "ldg",
            vec![rop(t), memop(base, imm, false)],
            "b.",
            false,
            false,
        ));
    }
    let mn = ["stg", "stzg", "st2g", "stz2g"][opc as usize];
    let t = gr(bits(w, 4, 0), true, true);
    Some(match op2 {
        1 => mk(
            w,
            addr,
            mn,
            vec![rop(t), memop(base, 0, false), iop(imm)],
            "...",
            true,
            false,
        ),
        2 => mk(
            w,
            addr,
            mn,
            vec![rop(t), memop(base, imm, false)],
            "..",
            false,
            false,
        ),
        _ => mk(
            w,
            addr,
            mn,
            vec![rop(t), memop(base, imm, true)],
            "..",
            true,
            false,
        ),
    })
}

fn literal(w: u32, addr: u64) -> Option<Insn> {
    let opc = bits(w, 31, 30);
    let v = bits(w, 26, 26);
    let rt = bits(w, 4, 0);
    let t = addr.wrapping_add((sext(bits(w, 23, 5), 19) << 2) as u64);
    let (mn, reg, simd) = if v == 0 {
        match opc {
            0 => ("ldr", gr(rt, false, false), false),
            1 => ("ldr", gr(rt, true, false), false),
            2 => ("ldrsw", gr(rt, true, false), false),
            _ => {
                return Some(mk(
                    w,
                    addr,
                    "prfm",
                    vec![Op::Name(prf(rt)), Op::Addr(t)],
                    "..",
                    false,
                    false,
                ));
            }
        }
    } else {
        let k = [Fk::S, Fk::D, Fk::Q].get(opc as usize).copied()?;
        ("ldr", fr(k, rt), true)
    };
    Some(mk(
        w,
        addr,
        mn,
        vec![rop(reg), Op::Addr(t)],
        "w.",
        false,
        simd,
    ))
}

/// The prefetch operation name.
fn prf(rt: u32) -> String {
    let ty = ["pld", "pli", "pst"].get((rt >> 3) as usize).copied();
    let tg = ["l1", "l2", "l3"].get(((rt >> 1) & 3) as usize).copied();
    match (ty, tg) {
        (Some(t), Some(g)) => format!("{t}{g}{}", if rt & 1 == 1 { "strm" } else { "keep" }),
        _ => format!("#{rt:#x}"),
    }
}

fn exclusive(w: u32, addr: u64) -> Option<Insn> {
    let size = bits(w, 31, 30);
    let o2 = bits(w, 23, 23);
    let l = bits(w, 22, 22);
    let o1 = bits(w, 21, 21);
    let rs = bits(w, 20, 16);
    let o0 = bits(w, 15, 15);
    let rt2 = bits(w, 14, 10);
    let rn = bits(w, 9, 5);
    let rt = bits(w, 4, 0);
    let base = gr(rn, true, true);
    let x = size == 3;
    let t = gr(rt, x, false);
    let m = memop(base, 0, false);
    let names = |a: [&'static str; 4]| a[size as usize];
    match (o2, o1) {
        (0, 0) => {
            if l == 0 {
                let mn = match (o0, size) {
                    (0, 0) => "stxrb",
                    (0, 1) => "stxrh",
                    (0, _) => "stxr",
                    (_, 0) => "stlxrb",
                    (_, 1) => "stlxrh",
                    _ => "stlxr",
                };
                return Some(mk(
                    w,
                    addr,
                    mn,
                    vec![rop(gr(rs, false, false)), rop(t), m],
                    "wr.",
                    false,
                    false,
                ));
            }
            let mn = match (o0, size) {
                (0, 0) => "ldxrb",
                (0, 1) => "ldxrh",
                (0, _) => "ldxr",
                (_, 0) => "ldaxrb",
                (_, 1) => "ldaxrh",
                _ => "ldaxr",
            };
            Some(mk(w, addr, mn, vec![rop(t), m], "w.", false, false))
        }
        (0, 1) => {
            if size < 2 {
                return casp(w, addr, size == 1, (l == 1, o0 == 1), (rs, rt, rn));
            }
            let t2 = gr(rt2, x, false);
            if l == 0 {
                let mn = if o0 == 0 { "stxp" } else { "stlxp" };
                Some(mk(
                    w,
                    addr,
                    mn,
                    vec![rop(gr(rs, false, false)), rop(t), rop(t2), m],
                    "wrr.",
                    false,
                    false,
                ))
            } else {
                let mn = if o0 == 0 { "ldxp" } else { "ldaxp" };
                Some(mk(
                    w,
                    addr,
                    mn,
                    vec![rop(t), rop(t2), m],
                    "ww.",
                    false,
                    false,
                ))
            }
        }
        (1, 0) => {
            let mn = match (l, o0) {
                (0, 0) => names(["stllrb", "stllrh", "stllr", "stllr"]),
                (0, _) => names(["stlrb", "stlrh", "stlr", "stlr"]),
                (_, 0) => names(["ldlarb", "ldlarh", "ldlar", "ldlar"]),
                _ => names(["ldarb", "ldarh", "ldar", "ldar"]),
            };
            let acc = if l == 0 { "r." } else { "w." };
            Some(mk(w, addr, mn, vec![rop(t), m], acc, false, false))
        }
        _ => cas(w, addr, (size, l == 1, o0 == 1), (rs, rt, rn)),
    }
}

fn cas(w: u32, addr: u64, f: (u32, bool, bool), regs: (u32, u32, u32)) -> Option<Insn> {
    let (size, a, r) = f;
    let (rs, rt, rn) = regs;
    let x = size == 3;
    let m = memop(gr(rn, true, true), 0, false);
    let idx = usize::from(a) + 2 * usize::from(r);
    let names: [[&'static str; 4]; 4] = [
        ["casb", "cash", "cas", "cas"],
        ["casab", "casah", "casa", "casa"],
        ["caslb", "caslh", "casl", "casl"],
        ["casalb", "casalh", "casal", "casal"],
    ];
    Some(mk(
        w,
        addr,
        names[idx][size as usize],
        vec![rop(gr(rs, x, false)), rop(gr(rt, x, false)), m],
        "...",
        false,
        false,
    ))
}

fn casp(w: u32, addr: u64, sz: bool, f: (bool, bool), regs: (u32, u32, u32)) -> Option<Insn> {
    let (a, r) = f;
    let (rs, rt, rn) = regs;
    if rs & 1 == 1 || rt & 1 == 1 {
        return None;
    }
    let names = [["casp", "caspl"], ["caspa", "caspal"]];
    let g = |n| rop(gr(n, sz, false));
    Some(mk(
        w,
        addr,
        names[usize::from(a)][usize::from(r)],
        vec![
            g(rs),
            g(rs + 1),
            g(rt),
            g(rt + 1),
            memop(gr(rn, true, true), 0, false),
        ],
        ".....",
        false,
        false,
    ))
}

fn pair(w: u32, addr: u64) -> Option<Insn> {
    let opc = bits(w, 31, 30);
    let v = bits(w, 26, 26);
    let mode = bits(w, 24, 23);
    let l = bits(w, 22, 22);
    let imm7 = sext(bits(w, 21, 15), 7);
    let rt2 = bits(w, 14, 10);
    let rn = bits(w, 9, 5);
    let rt = bits(w, 4, 0);
    let base = gr(rn, true, true);
    let (mn, scale, a, b, simd): (&'static str, u32, Reg, Reg, bool) = if v == 0 {
        match (opc, l) {
            (0, _) => (
                pn(mode, l),
                2,
                gr(rt, false, false),
                gr(rt2, false, false),
                false,
            ),
            (1, 1) if mode != 0 => ("ldpsw", 2, gr(rt, true, false), gr(rt2, true, false), false),
            (2, _) => (
                pn(mode, l),
                3,
                gr(rt, true, false),
                gr(rt2, true, false),
                false,
            ),
            _ => return None,
        }
    } else {
        let (k, sc) = [(Fk::S, 2), (Fk::D, 3), (Fk::Q, 4)]
            .get(opc as usize)
            .copied()?;
        (pn(mode, l), sc, fr(k, rt), fr(k, rt2), true)
    };
    let off = imm7 << scale;
    let acc = if l == 1 { "ww." } else { "rr." };
    match mode {
        0 | 2 => Some(mk(
            w,
            addr,
            mn,
            vec![rop(a), rop(b), memop(base, off, false)],
            acc,
            false,
            simd,
        )),
        3 => Some(mk(
            w,
            addr,
            mn,
            vec![rop(a), rop(b), memop(base, off, true)],
            acc,
            true,
            simd,
        )),
        _ => Some(mk(
            w,
            addr,
            mn,
            vec![rop(a), rop(b), memop(base, 0, false), iop(off)],
            &format!("{acc}."),
            true,
            simd,
        )),
    }
}

fn pn(mode: u32, l: u32) -> &'static str {
    match (mode, l) {
        (0, 0) => "stnp",
        (0, _) => "ldnp",
        (_, 0) => "stp",
        _ => "ldp",
    }
}

fn single(w: u32, addr: u64) -> Option<Insn> {
    let size = bits(w, 31, 30);
    let v = bits(w, 26, 26);
    let opc = bits(w, 23, 22);
    let rn = bits(w, 9, 5);
    let rt = bits(w, 4, 0);
    let base = gr(rn, true, true);
    let unsigned_off = bits(w, 24, 24) == 1;
    let (mode, imm): (u32, i64) = if unsigned_off {
        (4, i64::from(bits(w, 21, 10)))
    } else if bits(w, 21, 21) == 1 {
        match bits(w, 11, 10) {
            0 => return atomic(w, addr),
            2 => (5, 0),
            _ => return None,
        }
    } else {
        (bits(w, 11, 10), sext(bits(w, 20, 12), 9))
    };
    // the transfer register and mnemonic
    let (mn, reg, load, simd, scale) = transfer(size, v, opc, mode, rt)?;
    let m = match mode {
        4 => memop(base, imm << scale, false),
        5 => {
            let opt = bits(w, 15, 13);
            let s = bits(w, 12, 12);
            let (kind, xr) = match opt {
                2 => ("uxtw", false),
                3 => ("lsl", true),
                6 => ("sxtw", false),
                7 => ("sxtx", true),
                _ => return None,
            };
            let ir = gr(bits(w, 20, 16), xr, false);
            let sh = if kind == "lsl" && s == 0 {
                Sh {
                    kind: "",
                    amt: 0,
                    show0: false,
                }
            } else {
                Sh {
                    kind,
                    amt: if s == 1 { scale as u8 } else { 0 },
                    show0: s == 1,
                }
            };
            Op::Mem {
                base,
                index: Some((ir, sh)),
                disp: 0,
                wb: Wb::None,
            }
        }
        0 | 2 => memop(base, imm, false),
        3 => memop(base, imm, true),
        _ => memop(base, 0, false),
    };
    let acc = if mn.starts_with("prf") {
        ".."
    } else if load {
        "w."
    } else {
        "r."
    };
    let first = if mn.starts_with("prf") {
        Op::Name(prf(rt))
    } else {
        rop(reg)
    };
    if mode == 1 {
        return Some(mk(
            w,
            addr,
            mn,
            vec![first, m, Op::Post(imm)],
            &format!("{acc}."),
            true,
            simd,
        ));
    }
    Some(mk(w, addr, mn, vec![first, m], acc, mode == 3, simd))
}

type Transfer = (&'static str, Reg, bool, bool, u32);

fn transfer(size: u32, v: u32, opc: u32, mode: u32, rt: u32) -> Option<Transfer> {
    // mode: 0 unscaled, 1 post, 2 unprivileged, 3 pre, 4 unsigned offset, 5 register offset
    if v == 1 {
        if mode == 2 {
            return None;
        }
        let (k, sc, load) = match (size, opc) {
            (0, 0) => (Fk::B, 0, false),
            (0, 1) => (Fk::B, 0, true),
            (1, 0) => (Fk::H, 1, false),
            (1, 1) => (Fk::H, 1, true),
            (2, 0) => (Fk::S, 2, false),
            (2, 1) => (Fk::S, 2, true),
            (3, 0) => (Fk::D, 3, false),
            (3, 1) => (Fk::D, 3, true),
            (0, 2) => (Fk::Q, 4, false),
            (0, 3) => (Fk::Q, 4, true),
            _ => return None,
        };
        let mn = match (mode, load) {
            (0, true) => "ldur",
            (0, false) => "stur",
            (_, true) => "ldr",
            (_, false) => "str",
        };
        return Some((mn, fr(k, rt), load, true, sc));
    }
    // general registers
    let unpriv = mode == 2;
    let unsc = mode == 0;
    let n = |s: &'static str, u: &'static str, t: &'static str| -> &'static str {
        if unsc {
            u
        } else if unpriv {
            t
        } else {
            s
        }
    };
    let r = match (size, opc) {
        (0, 0) => (n("strb", "sturb", "sttrb"), gr(rt, false, false), false, 0),
        (0, 1) => (n("ldrb", "ldurb", "ldtrb"), gr(rt, false, false), true, 0),
        (0, 2) => (n("ldrsb", "ldursb", "ldtrsb"), gr(rt, true, false), true, 0),
        (0, _) => (
            n("ldrsb", "ldursb", "ldtrsb"),
            gr(rt, false, false),
            true,
            0,
        ),
        (1, 0) => (n("strh", "sturh", "sttrh"), gr(rt, false, false), false, 1),
        (1, 1) => (n("ldrh", "ldurh", "ldtrh"), gr(rt, false, false), true, 1),
        (1, 2) => (n("ldrsh", "ldursh", "ldtrsh"), gr(rt, true, false), true, 1),
        (1, _) => (
            n("ldrsh", "ldursh", "ldtrsh"),
            gr(rt, false, false),
            true,
            1,
        ),
        (2, 0) => (n("str", "stur", "sttr"), gr(rt, false, false), false, 2),
        (2, 1) => (n("ldr", "ldur", "ldtr"), gr(rt, false, false), true, 2),
        (2, 2) => (n("ldrsw", "ldursw", "ldtrsw"), gr(rt, true, false), true, 2),
        (3, 0) => (n("str", "stur", "sttr"), gr(rt, true, false), false, 3),
        (3, 1) => (n("ldr", "ldur", "ldtr"), gr(rt, true, false), true, 3),
        (3, 2) if !unpriv && matches!(mode, 0 | 4 | 5) => {
            (if unsc { "prfum" } else { "prfm" }, Reg::Xzr, true, 3)
        }
        _ => return None,
    };
    Some((r.0, r.1, r.2, false, r.3))
}

fn atomic(w: u32, addr: u64) -> Option<Insn> {
    let size = bits(w, 31, 30);
    let a = bits(w, 23, 23);
    let r = bits(w, 22, 22);
    let rs = bits(w, 20, 16);
    let o3 = bits(w, 15, 15);
    let opc = bits(w, 14, 12);
    let rn = bits(w, 9, 5);
    let rt = bits(w, 4, 0);
    let x = size == 3;
    let sfx = ["b", "h", "", ""][size as usize];
    let base = gr(rn, true, true);
    let m = memop(base, 0, false);
    let ar = (a * 2 + r) as usize;
    let sfx_ar = ["", "l", "a", "al"][ar];
    let stem = if o3 == 1 {
        if opc != 0 {
            return None;
        }
        "swp"
    } else {
        [
            "ldadd", "ldclr", "ldeor", "ldset", "ldsmax", "ldsmin", "ldumax", "ldumin",
        ][opc as usize]
    };
    let name = leak(format!("{stem}{sfx_ar}{sfx}"));
    if o3 == 0 && rt == 31 && a == 0 {
        let st = leak(format!("st{}{}{}", &stem[2..], ["", "l"][r as usize], sfx));
        return Some(mk(
            w,
            addr,
            st,
            vec![rop(gr(rs, x, false)), m],
            "..",
            false,
            false,
        ));
    }
    Some(mk(
        w,
        addr,
        name,
        vec![rop(gr(rs, x, false)), rop(gr(rt, x, false)), m],
        "...",
        false,
        false,
    ))
}

/// Interns a mnemonic built at run time (atomics: a few dozen distinct names).
fn leak(s: String) -> &'static str {
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};
    static POOL: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let pool = POOL.get_or_init(|| Mutex::new(HashSet::new()));
    let Ok(mut g) = pool.lock() else {
        return "?";
    };
    if let Some(e) = g.get(s.as_str()) {
        return e;
    }
    let l: &'static str = Box::leak(s.into_boxed_str());
    g.insert(l);
    l
}
