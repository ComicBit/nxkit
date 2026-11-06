//! Pattern helpers over decoded instructions: the regular expressions of `patch_nso.py` on `op_str`, as
//! structural matches on the operands.

use nxb_a64::{Insn, Op, Reg, Sh, Wb};

/// Operand `i` as a plain register (no shift or extend).
pub(crate) fn preg(i: &Insn, k: usize) -> Option<Reg> {
    match i.ops.get(k)? {
        Op::Reg(r, s) if s.kind.is_empty() => Some(*r),
        _ => None,
    }
}

/// Operand `k` as a plain non-negative immediate (`#5`, `#0x10`, no shift).
pub(crate) fn pimm(i: &Insn, k: usize) -> Option<u64> {
    match i.ops.get(k)? {
        Op::Imm(v, s) if s.kind.is_empty() && *v >= 0 => Some(*v as u64),
        Op::Hex(v) => Some(*v as u64),
        _ => None,
    }
}

/// The text of a register as the operand text prints it.
pub(crate) fn tx(r: Reg) -> String {
    r.text()
}

/// `[wx]N` register number if the register is an x or w register.
pub(crate) fn gn(r: Reg) -> Option<u8> {
    r.gpr()
}

/// Whether the instruction has exactly `n` operands.
pub(crate) fn nops(i: &Insn, n: usize) -> bool {
    i.ops.len() == n
}

/// A memory operand `[base, index, lsl #amt]` (index register as written) of operand `k`.
pub(crate) struct MemOp {
    pub base: Reg,
    pub index: Option<(Reg, Sh)>,
    pub disp: i64,
    pub wb: Wb,
}

/// Operand `k` as a memory operand.
pub(crate) fn memop(i: &Insn, k: usize) -> Option<MemOp> {
    match i.ops.get(k)? {
        Op::Mem {
            base,
            index,
            disp,
            wb,
        } => Some(MemOp {
            base: *base,
            index: *index,
            disp: *disp,
            wb: *wb,
        }),
        _ => None,
    }
}

/// `ldrsw xT, [xB, xI, lsl #2]` operand shapes: returns `(T, B, I)`.
pub(crate) fn ldrsw_idx(i: &Insn) -> Option<(Reg, Reg, u8)> {
    if i.mn != "ldrsw" || !nops(i, 2) {
        return None;
    }
    let t = preg(i, 0)?;
    let m = memop(i, 1)?;
    let (ir, sh) = m.index?;
    if m.disp != 0 || m.wb != Wb::None || sh.kind != "lsl" || sh.amt != 2 {
        return None;
    }
    match ir {
        Reg::X(n) => Some((t, m.base, n)),
        _ => None,
    }
}

/// How Capstone's `reg_name` and the `canon` table of `copied_base` spell a register: `fp`/`lr` as `x29`/`x30`
/// and the first `w` as `x`.
pub(crate) fn canon(r: &Reg) -> String {
    match r {
        Reg::X(n) | Reg::W(n) => format!("x{n}"),
        Reg::Wsp => "xsp".into(),
        r => r.name(),
    }
}

/// `written_w(p)`: the general registers `p` writes as numbers (a write of `wN` or `xN` is the same register).
/// `x29` and `x30` count (Capstone calls them `fp` and `lr`), FP and SIMD registers do not.
pub(crate) fn written_w(i: &Insn) -> std::collections::HashSet<u32> {
    i.writes
        .iter()
        .filter_map(|r| match r {
            Reg::X(n) | Reg::W(n) => Some(u32::from(*n)),
            _ => None,
        })
        .collect()
}

/// `writes_reg(p, reg)`: whether `p` changes the general register `reg` (`xN` or `wN`), a compare only sets flags.
pub(crate) fn writes_reg(i: &Insn, reg: &str) -> bool {
    let n: Option<u32> = reg.get(1..).and_then(|d| d.parse().ok());
    !matches!(i.mn, "cmp" | "cmn" | "tst") && n.is_some_and(|n| written_w(i).contains(&n))
}

/// `writes_reg_any(p, regs)` for a set of register numbers.
pub(crate) fn writes_reg_any<T: Copy + Into<u32>>(
    i: &Insn,
    regs: &std::collections::HashSet<T>,
) -> bool {
    !matches!(i.mn, "cmp" | "cmn" | "tst") && {
        let w = written_w(i);
        regs.iter().any(|r| w.contains(&(*r).into()))
    }
}

/// Whether the mnemonic is a direct branch (`b`, `bl`, `cbz`, `cbnz`, `tbz`, `tbnz`, `b.cond`).
pub(crate) fn is_direct_branch(m: &str) -> bool {
    matches!(m, "b" | "bl" | "cbz" | "cbnz" | "tbz" | "tbnz") || m.starts_with("b.")
}

/// A conditional or unconditional branch other than a call: `b`, `cbz`, `cbnz`, `tbz`, `tbnz`, `b.cond`.
pub(crate) fn is_branch(m: &str) -> bool {
    matches!(m, "b" | "cbz" | "cbnz" | "tbz" | "tbnz") || m.starts_with("b.")
}
