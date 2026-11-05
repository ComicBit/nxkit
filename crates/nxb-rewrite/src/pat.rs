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

/// `'w' + reg_name(r)[1:]` for the registers `regs_access` lists, as a register number (`None` if not numeric).
pub(crate) fn wname(r: &Reg) -> Option<u32> {
    match r {
        Reg::X(29) | Reg::X(30) => None,
        Reg::X(n) | Reg::W(n) => Some(u32::from(*n)),
        Reg::F(_, n) => Some(u32::from(*n)),
        _ => None,
    }
}

/// Whether `regs_access` lists a write whose `reg_name` equals `name`.
pub(crate) fn writes_named(i: &Insn, name: &str) -> bool {
    i.writes.iter().any(|r| r.name() == name)
}

/// Whether the mnemonic is a direct branch (`b`, `bl`, `cbz`, `cbnz`, `tbz`, `tbnz`, `b.cond`).
pub(crate) fn is_direct_branch(m: &str) -> bool {
    matches!(m, "b" | "bl" | "cbz" | "cbnz" | "tbz" | "tbnz") || m.starts_with("b.")
}

/// A conditional or unconditional branch other than a call: `b`, `cbz`, `cbnz`, `tbz`, `tbnz`, `b.cond`.
pub(crate) fn is_branch(m: &str) -> bool {
    matches!(m, "b" | "cbz" | "cbnz" | "tbz" | "tbnz") || m.starts_with("b.")
}
