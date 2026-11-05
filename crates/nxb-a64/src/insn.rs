//! The decoded instruction: mnemonic, operands and the registers read and written, in the shape the
//! rewriter's rules need (the facts a Capstone 5 `CsInsn` with details gives them).

use std::fmt::Write;

/// Kinds of scalar FP and vector registers, the rewriter folds them into one vector register number.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Fk {
    /// `b0`: 8 bits.
    B,
    /// `h0`: 16 bits.
    H,
    /// `s0`: 32 bits.
    S,
    /// `d0`: 64 bits.
    D,
    /// `q0`: 128 bits.
    Q,
    /// `v0`: a vector.
    V,
}

/// A register as Capstone names it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Reg {
    /// `x0` to `x30`.
    X(u8),
    /// `w0` to `w30`.
    W(u8),
    /// `sp`.
    Sp,
    /// `wsp`.
    Wsp,
    /// `xzr`.
    Xzr,
    /// `wzr`.
    Wzr,
    /// A scalar FP or vector register.
    F(Fk, u8),
    /// The condition flags.
    Nzcv,
}

impl Reg {
    /// The number of a general register (`x` or `w` form), `None` for every other register.
    pub fn gpr(self) -> Option<u8> {
        match self {
            Reg::X(n) | Reg::W(n) => Some(n),
            _ => None,
        }
    }

    /// Capstone's `reg_name`: `x29` is `fp`, `x30` is `lr`.
    pub fn name(self) -> String {
        match self {
            Reg::X(29) => "fp".into(),
            Reg::X(30) => "lr".into(),
            r => r.text(),
        }
    }

    /// The register as the operand text prints it.
    pub fn text(self) -> String {
        match self {
            Reg::X(n) => format!("x{n}"),
            Reg::W(n) => format!("w{n}"),
            Reg::Sp => "sp".into(),
            Reg::Wsp => "wsp".into(),
            Reg::Xzr => "xzr".into(),
            Reg::Wzr => "wzr".into(),
            Reg::Nzcv => "nzcv".into(),
            Reg::F(k, n) => format!("{}{n}", ['b', 'h', 's', 'd', 'q', 'v'][k as usize]),
        }
    }
}

/// A shift or extend applied to a register operand.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Sh {
    /// `lsl`, `lsr`, `asr`, `ror`, `msl` or an extend name (`uxtw`, `sxtx`, ...), empty for none.
    pub kind: &'static str,
    /// The amount.
    pub amt: u8,
    /// Print `#amt` even when it is zero.
    pub show0: bool,
}

/// No shift.
pub const NOSH: Sh = Sh {
    kind: "",
    amt: 0,
    show0: false,
};

/// How a memory operand updates its base.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Wb {
    /// No writeback, or post-index (the increment follows as an immediate operand).
    None,
    /// Pre-index (`[x0, #8]!`).
    Pre,
}

/// One operand.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Op {
    /// A register with an optional shift.
    Reg(Reg, Sh),
    /// An immediate with an optional shift (`#1, lsl #16`).
    Imm(i64, Sh),
    /// An immediate that always prints in hexadecimal (a logical-immediate bit mask).
    Hex(i64),
    /// A post-index increment: negative values print as unsigned 64-bit hexadecimal.
    Post(i64),
    /// A PC-relative target, absolute.
    Addr(u64),
    /// A memory operand `[base, index {ext}, #disp]`.
    Mem {
        /// The base register.
        base: Reg,
        /// The index register and its extend.
        index: Option<(Reg, Sh)>,
        /// The displacement.
        disp: i64,
        /// Pre-index writeback.
        wb: Wb,
    },
    /// A condition code name.
    Cond(&'static str),
    /// A system register, barrier option or prefetch operation name.
    Name(String),
    /// Anything else (a floating-point immediate, a register list).
    Other,
}

/// A decoded instruction.
#[derive(Clone, Debug)]
pub struct Insn {
    /// Its address.
    pub addr: u64,
    /// The instruction word.
    pub word: u32,
    /// The mnemonic as Capstone prints it (an alias where it prints one).
    pub mn: &'static str,
    /// The operands.
    pub ops: Vec<Op>,
    /// The registers read, as Capstone's `regs_access` lists them (zero registers left out).
    pub reads: Vec<Reg>,
    /// The registers written.
    pub writes: Vec<Reg>,
    /// The base register is written back.
    pub wb: bool,
    /// The instruction is Advanced SIMD or floating point: only its general registers, memory operand and
    /// mnemonic class are modelled, not the operand text.
    pub simd: bool,
}

/// An immediate as Capstone prints it: decimal up to 9, hexadecimal above, with a sign for negatives.
pub fn fmt_imm(v: i64) -> String {
    let (s, m) = if v < 0 {
        ("-", v.unsigned_abs())
    } else {
        ("", v as u64)
    };
    if m <= 9 {
        format!("{s}{m}")
    } else {
        format!("{s}{m:#x}")
    }
}

fn sh_text(s: Sh) -> String {
    if s.kind.is_empty() || (s.kind == "lsl" && s.amt == 0 && !s.show0) {
        String::new()
    } else if s.amt == 0 && !s.show0 {
        format!(", {}", s.kind)
    } else {
        format!(", {} #{}", s.kind, s.amt)
    }
}

impl Insn {
    /// The general register numbers the operand text names (`x5`, `w5`, also inside memory operands).
    pub fn text_gprs(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for o in &self.ops {
            match o {
                Op::Reg(r, _) => out.extend(r.gpr()),
                Op::Mem { base, index, .. } => {
                    out.extend(base.gpr());
                    if let Some((r, _)) = index {
                        out.extend(r.gpr());
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// Whether the operand text names `x18` or `w18`.
    pub fn has18(&self) -> bool {
        self.text_gprs().contains(&18)
    }

    /// The operand text (`op_str`), empty for Advanced SIMD and floating-point instructions.
    pub fn op_str(&self) -> String {
        if self.simd {
            return String::new();
        }
        let mut s = String::new();
        for (i, o) in self.ops.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            match o {
                Op::Reg(r, sh) => {
                    s.push_str(&r.text());
                    s.push_str(&sh_text(*sh));
                }
                Op::Imm(v, sh) => {
                    let _ = write!(s, "#{}", fmt_imm(*v));
                    s.push_str(&sh_text(*sh));
                }
                Op::Hex(v) => {
                    let u = *v as u64;
                    if u <= 9 {
                        let _ = write!(s, "#{u}");
                    } else {
                        let _ = write!(s, "#{u:#x}");
                    }
                }
                Op::Post(v) => {
                    if *v < 0 {
                        let _ = write!(s, "#{:#x}", *v as u64);
                    } else {
                        let _ = write!(s, "#{}", fmt_imm(*v));
                    }
                }
                Op::Addr(a) => {
                    if *a <= 9 {
                        let _ = write!(s, "#{a}");
                    } else {
                        let _ = write!(s, "#{a:#x}");
                    }
                }
                Op::Mem {
                    base,
                    index,
                    disp,
                    wb,
                } => {
                    s.push('[');
                    s.push_str(&base.text());
                    if let Some((r, sh)) = index {
                        s.push_str(", ");
                        s.push_str(&r.text());
                        s.push_str(&sh_text(*sh));
                    } else if *disp != 0 || *wb == Wb::Pre {
                        let _ = write!(s, ", #{}", fmt_imm(*disp));
                    }
                    s.push(']');
                    if *wb == Wb::Pre {
                        s.push('!');
                    }
                }
                Op::Cond(c) => s.push_str(c),
                Op::Name(n) => s.push_str(n),
                Op::Other => s.push('?'),
            }
        }
        s
    }

    /// The immediate of operand `i` (an address for a branch), if it is one.
    pub fn imm(&self, i: usize) -> Option<i64> {
        match self.ops.get(i)? {
            Op::Imm(v, _) | Op::Hex(v) | Op::Post(v) => Some(*v),
            Op::Addr(a) => Some(*a as i64),
            _ => None,
        }
    }

    /// The immediate of the last operand: a direct branch's target.
    pub fn target(&self) -> Option<u64> {
        match self.ops.last()? {
            Op::Addr(a) => Some(*a),
            Op::Imm(v, _) => Some(*v as u64),
            _ => None,
        }
    }

    /// The register of operand `i`.
    pub fn reg(&self, i: usize) -> Option<Reg> {
        match self.ops.get(i)? {
            Op::Reg(r, _) => Some(*r),
            _ => None,
        }
    }

    /// Whether any operand is a memory operand.
    pub fn has_mem(&self) -> bool {
        self.ops.iter().any(|o| matches!(o, Op::Mem { .. }))
    }

    /// The names of the registers written (Capstone's `reg_name`).
    pub fn write_names(&self) -> Vec<String> {
        self.writes.iter().map(|r| r.name()).collect()
    }

    /// Whether `regs_access` lists a write named `name`.
    pub fn writes_name(&self, name: &str) -> bool {
        self.writes.iter().any(|r| r.name() == name)
    }
}
