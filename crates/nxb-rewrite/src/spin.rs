//! Spin waits, part one: recognizing a loop that provably cannot leave by itself (`spin_*` of `patch_nso.py`).

use std::collections::{BTreeSet, HashMap, HashSet};

use nxb_a64::{Insn, Op, Reg, decode};

use crate::pat::{memop, nops};
use crate::rw::Rw;

/// A normalized register: general registers by number, vector registers 100 + n, `sp` 200, the flags 201.
pub(crate) type SR = u16;
pub(crate) const NZCV: SR = 201;

/// `spin_norm` of a register (`None` for the zero registers).
pub(crate) fn norm(r: &Reg) -> Option<SR> {
    match r {
        Reg::X(n) | Reg::W(n) => Some(SR::from(*n)),
        Reg::Sp | Reg::Wsp => Some(200),
        Reg::Xzr | Reg::Wzr => None,
        Reg::F(_, n) => Some(100 + SR::from(*n)),
        Reg::Nzcv => Some(NZCV),
    }
}

/// What a node of the region is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum NK {
    Rmw,
    Acq,
    Rel,
    Tst,
    Stc,
    Br,
    Pc,
    Ok,
}

/// A node of a spin region.
#[derive(Clone)]
pub(crate) struct Node {
    pub(crate) kind: NK,
    pub(crate) base: SR,
    pub(crate) k: i64,
    pub(crate) ins: Vec<Insn>,
    pub(crate) pre: usize,
    pub(crate) uses: Option<HashSet<SR>>,
    pub(crate) defs: Option<HashSet<SR>>,
    pub(crate) succ: Vec<i64>,
}

/// A found region.
pub(crate) struct Region {
    pub(crate) t: i64,
    pub(crate) r: Vec<i64>,
    pub(crate) nodes: HashMap<i64, Node>,
    pub(crate) valid: BTreeSet<i64>,
}

pub(crate) enum Found {
    No,
    Vary(HashSet<i64>),
    Yes(Region),
}

const SPIN_LSE: [&str; 8] = [
    "ldadd", "ldclr", "ldeor", "ldset", "ldsmax", "ldsmin", "ldumax", "ldumin",
];
const SPIN_BAD: &[&str] = &[
    "bl", "blr", "br", "ret", "eret", "svc", "hvc", "smc", "brk", "hlt", "udf", "mrs", "msr",
    "sys", "sysl", "wfe", "wfi", "sev", "sevl", "clrex", "dc", "ic", "at", "tlbi", "drps", "dcps1",
    "dcps2", "dcps3", "blraa", "blrab", "blraaz", "blrabz", "braa", "brab", "braaz", "brabz",
    "retaa", "retab", "eretaa", "eretab",
];

const SPIN_STOP: [(u32, u32); 12] = [
    (0xfc00_0000, 0x9400_0000),
    (0xfe00_0000, 0xd600_0000),
    (0xff00_0000, 0xd400_0000),
    (0x3fc0_0000, 0x3900_0000),
    (0x3fc0_0000, 0x3d00_0000),
    (0x3fc0_0000, 0x3d80_0000),
    (0x3fe0_0000, 0x3800_0000),
    (0x3fe0_0c00, 0x3820_0800),
    (0x3fe0_0000, 0x3c00_0000),
    (0x3fe0_0000, 0x3c80_0000),
    (0x3a40_0000, 0x2800_0000),
    (0x3f20_0c00, 0x3820_0000),
];

/// The registers an instruction reads and writes for the spin analysis.
pub(crate) fn spin_rw(i: &Insn) -> (HashSet<SR>, HashSet<SR>) {
    let m = i.mn;
    let regs: Vec<Option<SR>> = i
        .ops
        .iter()
        .filter_map(|o| {
            if let Op::Reg(r, _) = o {
                Some(norm(r))
            } else {
                None
            }
        })
        .collect();
    let ops: HashSet<SR> = regs.iter().flatten().copied().collect();
    if matches!(m, "cbz" | "cbnz" | "tbz" | "tbnz") {
        return (ops, HashSet::new());
    }
    if m.starts_with("b.") {
        return (HashSet::from([NZCV]), HashSet::new());
    }
    if m == "b" {
        return (HashSet::new(), HashSet::new());
    }
    if matches!(m, "cmp" | "cmn" | "tst" | "fcmp" | "fcmpe") {
        return (ops, HashSet::from([NZCV]));
    }
    if matches!(m, "ccmp" | "ccmn" | "fccmp" | "fccmpe") {
        let mut u = ops;
        u.insert(NZCV);
        return (u, HashSet::from([NZCV]));
    }
    let r: HashSet<SR> = i.reads.iter().filter_map(norm).collect();
    let w: HashSet<SR> = i.writes.iter().filter_map(norm).collect();
    if m.starts_with("ld") || m.starts_with("prfm") || m.starts_with("prfum") {
        return (r, w.union(&ops).copied().collect());
    }
    let rest: HashSet<SR> = regs.iter().skip(1).flatten().copied().collect();
    let first: HashSet<SR> = regs.iter().take(1).flatten().copied().collect();
    (
        r.union(&rest).copied().collect(),
        w.union(&first).copied().collect(),
    )
}

/// `'br'` direct branch, `'pc'` adr / adrp, `'ok'` a load or register arithmetic, `None` anything else.
pub(crate) fn spin_kind(i: Option<&Insn>) -> Option<NK> {
    let i = i?;
    let m = i.mn;
    if matches!(m, "b" | "cbz" | "cbnz" | "tbz" | "tbnz") || m.starts_with("b.") {
        return Some(NK::Br);
    }
    if matches!(m, "adr" | "adrp") {
        return Some(NK::Pc);
    }
    if SPIN_BAD.contains(&m) {
        return None;
    }
    if m.starts_with("st")
        || m.starts_with("cas")
        || m.starts_with("swp")
        || SPIN_LSE.iter().any(|p| m.starts_with(p))
    {
        return None;
    }
    let mem = i.has_mem();
    if m.starts_with("ld") || m.starts_with("prfm") || m.starts_with("prfum") {
        return mem.then_some(NK::Ok);
    }
    if mem { None } else { Some(NK::Ok) }
}

fn plain_mem(i: &Insn, k: usize) -> bool {
    memop(i, k).is_some_and(|m| m.index.is_none() && m.disp == 0)
}

type Lock = (NK, SR, Vec<Insn>, usize, usize, HashSet<SR>, HashSet<SR>);

impl Rw {
    pub(crate) fn spin_ins(&self, a: i64) -> Option<Insn> {
        if a < 0 || a as usize + 4 > self.img.len() {
            return None;
        }
        decode(self.w32(a), a as u64)
    }

    pub(crate) fn spin_rmw(&self, a: i64) -> Option<(SR, i64, HashSet<SR>, Vec<Insn>)> {
        let l: Vec<Insn> = (0..4)
            .map(|i| self.spin_ins(a + 4 * i))
            .collect::<Option<Vec<_>>>()?;
        if !matches!(l[0].mn, "ldxr" | "ldaxr")
            || !matches!(l[1].mn, "add" | "sub")
            || !matches!(l[2].mn, "stxr" | "stlxr")
            || l[3].mn != "cbnz"
        {
            return None;
        }
        if !nops(&l[1], 3) || !matches!(l[1].ops[2], Op::Imm(_, s) if s.kind.is_empty()) {
            return None;
        }
        let m0 = memop(&l[0], 1)?;
        let m2 = memop(&l[2], 2)?;
        if m0.disp != 0 || m0.index.is_some() || m2.base != m0.base || m2.disp != 0 {
            return None;
        }
        let (r0, r1, r2, r3) = (l[0].reg(0)?, l[1].reg(1)?, l[1].reg(0)?, l[2].reg(1)?);
        let (a2, a3) = (l[2].reg(0)?, l[3].reg(0)?);
        if r1 != r0 || r3 != r2 || a3 != a2 || l[3].imm(1) != Some(a) {
            return None;
        }
        let k = l[1].imm(2)? * if l[1].mn == "add" { 1 } else { -1 };
        let set: HashSet<SR> = [
            norm(&l[0].reg(0)?),
            norm(&l[1].reg(0)?),
            norm(&l[2].reg(0)?),
        ]
        .into_iter()
        .flatten()
        .collect();
        Some((norm(&m0.base)?, k, set, l))
    }

    pub(crate) fn spin_lock(&self, a: i64) -> Option<Lock> {
        let l: Vec<Option<Insn>> = (0..6).map(|i| self.spin_ins(a + 4 * i)).collect();
        let l0 = l[0].as_ref()?;
        let m0 = l0.mn;
        let all: Option<Vec<Insn>> = l.iter().cloned().collect();
        if matches!(m0, "ldxr" | "ldaxr")
            && let Some(l) = all
            && matches!(l0.reg(0), Some(Reg::W(_)))
            && plain_mem(l0, 1)
        {
            let base = memop(l0, 1)?.base;
            let bn = norm(&base)?;
            let r0 = l0.reg(0);
            let acq = l[1].mn == "cbz"
                && l[1].reg(0) == r0
                && l[1].imm(1) == Some(a + 16)
                && l[2].mn == "clrex"
                && l[3].mn == "b"
                && l[3].imm(0) == Some(a)
                && matches!(l[4].mn, "stxr" | "stlxr")
                && plain_mem(&l[4], 2)
                && memop(&l[4], 2)?.base == base
                && !matches!(l[4].reg(1), Some(Reg::Wzr | Reg::Xzr))
                && l[5].mn == "cbnz"
                && l[5].reg(0) == l[4].reg(0)
                && l[5].imm(1) == Some(a);
            if acq {
                let u: HashSet<SR> = [Some(bn), l[4].reg(1).as_ref().and_then(norm)]
                    .into_iter()
                    .flatten()
                    .collect();
                let d: HashSet<SR> = [
                    r0.as_ref().and_then(norm),
                    l[4].reg(0).as_ref().and_then(norm),
                ]
                .into_iter()
                .flatten()
                .collect();
                return Some((NK::Acq, bn, l, 0, 6, u, d));
            }
            let p = self.spin_ins(a - 4);
            if let Some(p) = p
                && p.mn == "clrex"
                && l[1].mn == "cmp"
                && l[1].reg(0) == r0
                && matches!(l[1].ops.get(1), Some(Op::Imm(..)))
                && l[2].mn == "b.ne"
                && l[2].imm(0) == Some(a - 4)
                && matches!(l[3].mn, "stxr" | "stlxr")
                && l[3].reg(1) == Some(Reg::Wzr)
                && plain_mem(&l[3], 2)
                && memop(&l[3], 2)?.base == base
                && l[4].mn == "cbnz"
                && l[4].reg(0) == l[3].reg(0)
                && l[4].imm(1) == Some(a)
            {
                let d: HashSet<SR> = [
                    r0.as_ref().and_then(norm),
                    l[3].reg(0).as_ref().and_then(norm),
                    Some(NZCV),
                ]
                .into_iter()
                .flatten()
                .collect();
                let mut ins = vec![p];
                ins.extend(l[..5].iter().cloned());
                return Some((NK::Rel, bn, ins, 1, 5, HashSet::from([bn]), d));
            }
        }
        if m0 == "stlr"
            && nops(l0, 2)
            && matches!(l0.reg(0), Some(Reg::Wzr | Reg::Xzr))
            && plain_mem(l0, 1)
        {
            let bn = norm(&memop(l0, 1)?.base)?;
            return Some((
                NK::Rel,
                bn,
                vec![l0.clone()],
                0,
                1,
                HashSet::from([bn]),
                HashSet::new(),
            ));
        }
        if matches!(m0, "ldr" | "ldrb" | "ldrh")
            && let (Some(l1), Some(l2)) = (l[1].as_ref(), l[2].as_ref())
            && nops(l0, 2)
            && matches!(l0.reg(0), Some(Reg::W(_)))
            && memop(l0, 1).is_some_and(|m| m.index.is_none())
            && !l0.wb
            && l1.mn == "cmp"
            && l1.reg(0) == l0.reg(0)
            && matches!(l1.ops.get(1), Some(Op::Imm(..)))
            && matches!(l2.mn, "b.eq" | "b.ne")
            && l2.imm(0) == Some(a)
        {
            let bn = norm(&memop(l0, 1)?.base)?;
            let d: HashSet<SR> = [l0.reg(0).as_ref().and_then(norm), Some(NZCV)]
                .into_iter()
                .flatten()
                .collect();
            return Some((
                NK::Tst,
                bn,
                vec![l0.clone(), l1.clone(), l2.clone()],
                0,
                3,
                HashSet::from([bn]),
                d,
            ));
        }
        None
    }

    /// A store of a w register or zero to `[xN{,#imm}]`: `(value register or none, base register)`.
    pub(crate) fn spin_store(i: Option<&Insn>) -> Option<(Option<SR>, SR)> {
        let i = i?;
        if !matches!(i.mn, "str" | "strb" | "strh" | "stur" | "sturb" | "sturh")
            || i.wb
            || !nops(i, 2)
        {
            return None;
        }
        let v = i.reg(0)?;
        let m = memop(i, 1)?;
        if m.index.is_some() {
            return None;
        }
        if !matches!(v, Reg::W(_) | Reg::Wsp | Reg::Wzr | Reg::Xzr) {
            return None;
        }
        Some((norm(&v), norm(&m.base)?))
    }

    pub(crate) fn spin_quick(&self, t: i64, hi: i64) -> bool {
        let mut seen = HashSet::new();
        let mut todo = vec![t];
        while let Some(a) = todo.pop() {
            if seen.contains(&a) || !(t <= a && a <= hi) {
                continue;
            }
            seen.insert(a);
            let w = self.w32(a);
            let ty = w >> 24;
            let succ: Vec<i64> = if (0x14..=0x17).contains(&ty) {
                vec![a + ((i64::from((w & 0x3ff_ffff) ^ 0x200_0000)) - 0x200_0000) * 4]
            } else if ty == 0x54 || ty & 0x7e == 0x34 {
                vec![
                    a + ((i64::from(((w >> 5) & 0x7_ffff) ^ 0x4_0000)) - 0x4_0000) * 4,
                    a + 4,
                ]
            } else if ty & 0x7e == 0x36 {
                vec![
                    a + ((i64::from(((w >> 5) & 0x3fff) ^ 0x2000)) - 0x2000) * 4,
                    a + 4,
                ]
            } else if SPIN_STOP.iter().any(|(m, v)| w & m == *v)
                && !(matches!(w & 0xbfc0_0000, 0xb900_0000 | 0x3900_0000 | 0x7900_0000)
                    || matches!(w & 0xbfe0_0c00, 0xb800_0000 | 0x3800_0000 | 0x7800_0000))
            {
                continue;
            } else {
                vec![a + 4]
            };
            if succ.contains(&t) {
                return true;
            }
            todo.extend(succ);
        }
        false
    }
}
