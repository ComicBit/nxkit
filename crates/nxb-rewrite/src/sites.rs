//! The rewrite blocks of each site kind (`site_*` of `patch_nso.py`).

use nxb_a64::{Insn, Op, Reg, Wb, encode};

use crate::error::fail;
use crate::rw::{Gate, K18, KGATE, KTLS, R, Rw, dis, pick, reach, regs_in, subst};

/// `ldp xt, xt2, [xn, #off]` for an offset that may not fit (Python integers do not overflow).
fn ldp_wide(t: u32, t2: u32, n: u32, off: i128) -> u32 {
    0xa940_0000 | (((off.div_euclid(8) & 0x7f) as u32) << 15) | (t2 << 10) | (n << 5) | t
}

/// The amount an instruction writes back to sp (pre-index or post-index), 0 if sp is not written back.
fn sp_delta(ins: &Insn) -> R<i128> {
    for (i, o) in ins.ops.iter().enumerate() {
        if let Op::Mem {
            base: Reg::Sp,
            disp,
            wb: Wb::Pre,
            ..
        } = o
        {
            return Ok(i128::from(*disp));
        }
        if let (
            Op::Mem {
                base: Reg::Sp,
                index: None,
                disp: 0,
                wb: Wb::None,
            },
            Some(n),
        ) = (o, ins.ops.get(i + 1))
        {
            match n {
                Op::Post(v) if i + 2 == ins.ops.len() => {
                    return Ok(if *v < 0 {
                        i128::from(*v as u64)
                    } else {
                        i128::from(*v)
                    });
                }
                Op::Imm(v, _) if i + 2 == ins.ops.len() => return Ok(i128::from(*v)),
                _ => {}
            }
        }
    }
    if matches!(ins.reg(0), Some(Reg::Sp | Reg::Wsp)) {
        return fail(format!(
            "assertion failed: writes sp: {:x} {}",
            ins.addr,
            ins.op_str()
        ));
    }
    Ok(0)
}

impl Rw {
    fn prologue(&mut self, s: u32, t: u32, save: i64) -> R<()> {
        self.emit(encode::stp(s, t, 31, save));
        self.emit(encode::mrs_tpidrro(t));
        self.emit_slot_addr(t, s, K18)?;
        self.emit(encode::ldrx(s, t, 0));
        Ok(())
    }

    pub(crate) fn site_plain(&mut self, ins: &Insn) -> R<i64> {
        let st = pick(&regs_in(ins), 2)?;
        let (s, t) = (st[0], st[1]);
        let dlt = sp_delta(ins)?;
        let save: i64 = if dlt == 0 { -16 } else { -112 };
        if !(dlt == 0 || dlt >= -96) {
            return fail(format!(
                "assertion failed: sp delta {dlt} at {:x}",
                ins.addr
            ));
        }
        let start = self.here();
        self.prologue(s, t, save)?;
        let w = subst(ins, s)?;
        self.emit(w);
        self.emit(encode::strx(s, t, 0));
        self.emit(ldp_wide(s, t, 31, i128::from(save) - dlt));
        self.emit_b(ins.addr as i64 + 4)?;
        Ok(start)
    }

    pub(crate) fn site_sp(&mut self, ins: &Insn) -> R<i64> {
        let st = pick(&regs_in(ins), 2)?;
        let (s, t) = (st[0], st[1]);
        let start = self.here();
        self.prologue(s, t, -16)?;
        self.emit(encode::addi(t, 31, 0, 0));
        let w = subst(ins, s)?;
        self.emit(w);
        self.emit(encode::ldp(s, t, t, -16));
        self.emit_b(ins.addr as i64 + 4)?;
        Ok(start)
    }

    pub(crate) fn site_sp_align(&mut self, ins: &Insn, n: u32) -> R<i64> {
        let start = self.here();
        self.emit(encode::addi(n, n, 15, 0));
        self.emit(0x927c_ec00 | n << 5 | n);
        self.emit(ins.word);
        self.emit_b(ins.addr as i64 + 4)?;
        Ok(start)
    }

    pub(crate) fn site_cond(&mut self, ins: &Insn) -> R<i64> {
        let w = ins.word;
        let st = pick(&regs_in(ins), 2)?;
        let (s, t) = (st[0], st[1]);
        let tgt = ins.target().unwrap_or(0) as i64;
        let start = self.here();
        self.prologue(s, t, -16)?;
        let m = ins.mn;
        let taken = self.here() + 4 * 3;
        let off = (taken - self.here()) >> 2;
        let nw = if matches!(m, "cbz" | "cbnz") {
            (w & 0xff00_0000) | (((off as u32) & 0x7_ffff) << 5) | s
        } else {
            (w & 0xfff8_0000) | (((off as u32) & 0x3fff) << 5) | s
        };
        let pc = self.here();
        self.emit(nw);
        let dd = dis(nw, pc)?;
        if dd.mn != m || dd.target() != Some(taken as u64) {
            return fail("assertion failed: conditional branch copy");
        }
        self.emit(encode::ldp(s, t, 31, -16));
        self.emit_b(ins.addr as i64 + 4)?;
        if self.here() != taken {
            return fail("assertion failed: taken");
        }
        self.emit(encode::ldp(s, t, 31, -16));
        self.emit_b(tgt)?;
        Ok(start)
    }

    pub(crate) fn site_pcrel(&mut self, ins: &Insn) -> R<i64> {
        let tgt = ins.imm(1).unwrap_or(0);
        let (s, t) = (0, 1);
        let start = self.here();
        self.emit(encode::stp(s, t, 31, -16));
        self.emit(encode::mrs_tpidrro(t));
        self.emit_slot_addr(t, s, K18)?;
        let pc = self.here();
        let w = reach(encode::adrp(s, pc, tgt))?;
        self.emit(w);
        if ins.mn == "adr" {
            self.emit(encode::addi(s, s, (tgt & 0xfff) as u32, 0));
        }
        self.emit(encode::strx(s, t, 0));
        self.emit(encode::ldp(s, t, 31, -16));
        self.emit_b(ins.addr as i64 + 4)?;
        Ok(start)
    }

    pub(crate) fn site_br(&mut self, base: i64, count: usize) -> R<i64> {
        let start = self.here();
        self.emit(encode::stur(16, 31, -16));
        self.emit(encode::stur(17, 31, -24));
        self.emit(encode::mrs_tpidrro(16));
        self.emit_slot_addr(16, 17, K18)?;
        self.emit(encode::ldrx(16, 16, 0));
        self.emit(encode::ldur(17, 31, -24));
        let padbase = start + 4 * 24;
        let delta = padbase - base;
        let dl = delta.unsigned_abs();
        let (mut hi, lo) = (dl >> 12, (dl & 0xfff) as u32);
        let mut ops = Vec::new();
        let op = |imm: u32, sh: u32| {
            if delta >= 0 {
                encode::addi(16, 16, imm, sh)
            } else {
                encode::subi(16, 16, imm, sh)
            }
        };
        while hi != 0 {
            let h = hi.min(0xfff);
            ops.push(op(h as u32, 1));
            hi -= h;
        }
        if lo != 0 {
            ops.push(op(lo, 0));
        }
        if ops.len() > 12 {
            return fail("assertion failed: br offset ops");
        }
        for o in ops {
            self.emit(o);
        }
        self.emit(encode::br(16));
        while self.here() < padbase {
            self.emit(encode::NOP);
        }
        let tails = padbase + 4 * count as i64;
        for i in 0..count {
            self.emit_b(tails + 8 * i as i64)?;
        }
        for i in 0..count {
            self.emit(0xf85f_0000 | (31 << 5) | 16);
            self.emit_b(base + 4 * i as i64)?;
        }
        Ok(start)
    }

    pub(crate) fn site_br_tab(&mut self, targets: &[i64]) -> R<i64> {
        let start = self.here();
        self.emit(encode::stur(16, 31, -16));
        self.emit(encode::stur(17, 31, -24));
        self.emit(encode::mrs_tpidrro(16));
        self.emit_slot_addr(16, 17, K18)?;
        self.emit(encode::ldrx(16, 16, 0));
        let mut uniq: Vec<i64> = targets.to_vec();
        uniq.sort_unstable();
        uniq.dedup();
        let pads = self.here() + 4 * (4 * uniq.len() as i64 + 1);
        for (k, t) in uniq.iter().enumerate() {
            let w = reach(encode::adrp(17, self.here(), *t))?;
            self.emit(w);
            self.emit(encode::addi(17, 17, (*t & 0xfff) as u32, 0));
            self.emit(encode::cmpr(16, 17));
            let w = reach(encode::beq(self.here(), pads + 12 * k as i64))?;
            self.emit(w);
        }
        self.emit(encode::brk(0x18));
        for t in &uniq {
            self.emit(encode::ldur(16, 31, -16));
            self.emit(encode::ldur(17, 31, -24));
            self.emit_b(*t)?;
        }
        Ok(start)
    }

    pub(crate) fn site_blr(&mut self, ins: &Insn) -> R<i64> {
        let start = self.here();
        self.emit(encode::mrs_tpidrro(16));
        self.emit_slot_addr(16, 17, K18)?;
        self.emit(encode::ldrx(16, 16, 0));
        let ret = ins.addr as i64 + 4;
        let w = reach(encode::adrp(30, self.here(), ret))?;
        self.emit(w);
        self.emit(encode::addi(30, 30, (ret & 0xfff) as u32, 0));
        self.emit(encode::br(16));
        Ok(start)
    }

    pub(crate) fn site_br_call(&mut self) -> R<i64> {
        let start = self.here();
        self.emit(encode::mrs_tpidrro(16));
        self.emit_slot_addr(16, 17, K18)?;
        self.emit(encode::ldrx(16, 16, 0));
        self.emit(encode::br(16));
        Ok(start)
    }

    pub(crate) fn site_sysreg_read(&mut self, ins: &Insn, kind: u32) -> R<i64> {
        let n = ins.word & 31;
        let tmp = if n != 16 { 16 } else { 17 };
        let start = self.here();
        self.emit(encode::stur(tmp, 31, -16));
        self.emit(encode::mrs_tpidrro(n));
        self.emit_slot_addr(n, tmp, kind)?;
        self.emit(encode::ldrx(n, n, 0));
        self.emit(encode::ldur(tmp, 31, -16));
        self.emit_b(ins.addr as i64 + 4)?;
        Ok(start)
    }

    pub(crate) fn site_tpidr_write(&mut self, ins: &Insn) -> R<i64> {
        let n = ins.word & 31;
        let t = pick(&[n], 1)?[0];
        let start = self.here();
        self.emit(0xf81f_0000 | (31 << 5) | t);
        let tmp = if ![n, t].contains(&16) { 16 } else { 17 };
        self.emit(encode::stur(tmp, 31, -24));
        self.emit(encode::mrs_tpidrro(t));
        self.emit_slot_addr(t, tmp, KTLS)?;
        self.emit(encode::strx(n, t, 0));
        self.emit(encode::ldur(tmp, 31, -24));
        self.emit(0xf85f_0000 | (31 << 5) | t);
        self.emit_b(ins.addr as i64 + 4)?;
        Ok(start)
    }

    /// The common section of a gate block: `x30` = the gate address in the thread slot.
    pub(crate) fn emit_gate_call(&mut self) -> R<()> {
        self.emit(encode::mrs_tpidrro(30));
        self.emit_slot_addr(30, 29, KGATE)?;
        self.emit(encode::ldrx(30, 30, 0));
        self.emit(encode::ldrx(29, 31, 0));
        Ok(())
    }

    pub(crate) fn site_gate(&mut self, ins: &Insn, kind: &str, arg: i64) -> R<i64> {
        let start = self.here();
        self.emit(encode::stp_pre(29, 30, 31, -16));
        self.emit_gate_call()?;
        self.emit(encode::blr(30));
        let ret = self.here();
        self.emit(encode::ldp_post(29, 30, 31, 16));
        self.emit_b(ins.addr as i64 + 4)?;
        self.gates.push(Gate {
            ret,
            kind: kind.into(),
            arg,
            orig: ins.addr as i64,
        });
        Ok(start)
    }

    pub(crate) fn site_obs(&mut self, off: i64, oid: u64, word: u32) -> R<i64> {
        let ins = dis(word, off)?;
        let m = ins.mn;
        if matches!(
            m,
            "adr" | "adrp" | "b" | "bl" | "br" | "blr" | "ret" | "cbz" | "cbnz" | "tbz" | "tbnz"
        ) || m.starts_with("b.")
            || (m.starts_with("ldr") && !ins.has_mem())
            || ins.has18()
        {
            return fail(format!(
                "assertion failed: observation gate original instruction cannot be relocated {off:#x} {m} {}",
                ins.op_str()
            ));
        }
        let start = self.here();
        self.emit(encode::stp_pre(29, 30, 31, -16));
        self.emit_gate_call()?;
        self.emit(encode::blr(30));
        let ret = self.here();
        self.emit(encode::ldp_post(29, 30, 31, 16));
        self.emit(word);
        self.emit_b(off + 4)?;
        self.gates.push(Gate {
            ret,
            kind: "obs".into(),
            arg: oid as i64,
            orig: off,
        });
        Ok(start)
    }
}
