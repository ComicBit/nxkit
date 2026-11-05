//! Shared helpers of the decoder: registers by field, bit access, instruction construction.

use crate::insn::{Fk, Insn, NOSH, Op, Reg, Sh};

/// Bits `hi..=lo` of `w`.
pub(crate) fn bits(w: u32, hi: u32, lo: u32) -> u32 {
    (w >> lo) & (u32::MAX >> (31 - (hi - lo)))
}

/// Sign-extends the low `n` bits of `v`.
pub(crate) fn sext(v: u32, n: u32) -> i64 {
    let s = 64 - n;
    ((u64::from(v) << s) as i64) >> s
}

/// A general register by field: `sf` selects `x` or `w`, `sp` lets 31 mean the stack pointer (else the zero register).
pub(crate) fn gr(n: u32, sf: bool, sp: bool) -> Reg {
    let n8 = n as u8;
    match (n, sf, sp) {
        (31, true, true) => Reg::Sp,
        (31, true, false) => Reg::Xzr,
        (31, false, true) => Reg::Wsp,
        (31, false, false) => Reg::Wzr,
        (_, true, _) => Reg::X(n8),
        (_, false, _) => Reg::W(n8),
    }
}

/// A scalar FP or vector register.
pub(crate) fn fr(k: Fk, n: u32) -> Reg {
    Reg::F(k, n as u8)
}

/// A plain register operand.
pub(crate) fn rop(r: Reg) -> Op {
    Op::Reg(r, NOSH)
}

/// A register operand with a shift or extend.
pub(crate) fn rsh(r: Reg, kind: &'static str, amt: u32, show0: bool) -> Op {
    Op::Reg(
        r,
        Sh {
            kind,
            amt: amt as u8,
            show0,
        },
    )
}

/// A plain immediate operand.
pub(crate) fn iop(v: i64) -> Op {
    Op::Imm(v, NOSH)
}

/// Builds an instruction. `acc` has one character per operand: `r` read, `w` written, `b` both, `.` neither
/// (memory operands read their base and index by themselves). `wb` writes the base register back.
pub(crate) fn mk(
    w: u32,
    addr: u64,
    mn: &'static str,
    ops: Vec<Op>,
    acc: &str,
    wb: bool,
    simd: bool,
) -> Insn {
    let mut reads = Vec::new();
    let mut writes = Vec::new();
    let add = |v: &mut Vec<Reg>, r: Reg| {
        if !matches!(r, Reg::Xzr | Reg::Wzr) && !v.contains(&r) {
            v.push(r);
        }
    };
    for (i, o) in ops.iter().enumerate() {
        let a = acc.as_bytes().get(i).copied().unwrap_or(b'.');
        match o {
            Op::Reg(r, _) => {
                if a == b'r' || a == b'b' {
                    add(&mut reads, *r);
                }
                if a == b'w' || a == b'b' {
                    add(&mut writes, *r);
                }
            }
            Op::Mem { base, index, .. } => {
                add(&mut reads, *base);
                if let Some((r, _)) = index {
                    add(&mut reads, *r);
                }
                if wb {
                    add(&mut writes, *base);
                }
            }
            _ => {}
        }
    }
    Insn {
        addr,
        word: w,
        mn,
        ops,
        reads,
        writes,
        wb,
        simd,
    }
}

/// Adds implicit flag reads and writes.
pub(crate) fn flags(mut i: Insn, read: bool, write: bool) -> Insn {
    if read {
        i.reads.push(Reg::Nzcv);
    }
    if write {
        i.writes.push(Reg::Nzcv);
    }
    i
}

/// Condition names as Capstone prints them in `b.cond`, `csel` and friends.
pub(crate) const COND: [&str; 16] = [
    "eq", "ne", "hs", "lo", "mi", "pl", "vs", "vc", "hi", "ls", "ge", "lt", "gt", "le", "al", "nv",
];

/// The `b.cond` mnemonics.
pub(crate) const BCOND: [&str; 16] = [
    "b.eq", "b.ne", "b.hs", "b.lo", "b.mi", "b.pl", "b.vs", "b.vc", "b.hi", "b.ls", "b.ge", "b.lt",
    "b.gt", "b.le", "b.al", "b.nv",
];
