//! The instruction encoders of the rewriter (`dm/tools/nso_asm.py`). Each returns the instruction word.
//! a branch checks its reach.

/// Error of an encoder whose operand does not fit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reach(pub &'static str);

/// `b` from `frm` to `to`.
pub fn b(frm: i64, to: i64) -> Result<u32, Reach> {
    let off = to - frm;
    if !(-(1 << 27)..(1 << 27)).contains(&off) || off % 4 != 0 {
        return Err(Reach("b out of reach"));
    }
    Ok(0x1400_0000 | ((off >> 2) as u32 & 0x03ff_ffff))
}

/// `stp xt, xt2, [xn, #off]`.
pub fn stp(t: u32, t2: u32, n: u32, off: i64) -> u32 {
    0xa900_0000 | (((off / 8) as u32 & 0x7f) << 15) | (t2 << 10) | (n << 5) | t
}

/// `ldp xt, xt2, [xn, #off]`.
pub fn ldp(t: u32, t2: u32, n: u32, off: i64) -> u32 {
    0xa940_0000 | (((off / 8) as u32 & 0x7f) << 15) | (t2 << 10) | (n << 5) | t
}

/// `stp xt, xt2, [xn, #off]!`.
pub fn stp_pre(t: u32, t2: u32, n: u32, off: i64) -> u32 {
    0xa980_0000 | (((off / 8) as u32 & 0x7f) << 15) | (t2 << 10) | (n << 5) | t
}

/// `ldp xt, xt2, [xn], #off`.
pub fn ldp_post(t: u32, t2: u32, n: u32, off: i64) -> u32 {
    0xa8c0_0000 | (((off / 8) as u32 & 0x7f) << 15) | (t2 << 10) | (n << 5) | t
}

/// `stur xt, [xn, #off]`.
pub fn stur(t: u32, n: u32, off: i64) -> u32 {
    0xf800_0000 | ((off as u32 & 0x1ff) << 12) | (n << 5) | t
}

/// `ldur xt, [xn, #off]`.
pub fn ldur(t: u32, n: u32, off: i64) -> u32 {
    0xf840_0000 | ((off as u32 & 0x1ff) << 12) | (n << 5) | t
}

/// `ldr xt, [xn, #imm12 * 8]`.
pub fn ldrx(t: u32, n: u32, imm12: u32) -> u32 {
    0xf940_0000 | (imm12 << 10) | (n << 5) | t
}

/// `str xt, [xn, #imm12 * 8]`.
pub fn strx(t: u32, n: u32, imm12: u32) -> u32 {
    0xf900_0000 | (imm12 << 10) | (n << 5) | t
}

/// `mrs xt, tpidrro_el0`.
pub fn mrs_tpidrro(t: u32) -> u32 {
    0xd53b_d060 | t
}

/// `adrp xd` from `pc` to the page of `target`.
pub fn adrp(dd: u32, pc: i64, target: i64) -> Result<u32, Reach> {
    let imm = ((target & !0xfff) - (pc & !0xfff)) >> 12;
    if !(-(1 << 20)..(1 << 20)).contains(&imm) {
        return Err(Reach("adrp out of reach"));
    }
    Ok(0x9000_0000 | ((imm as u32 & 3) << 29) | (((imm >> 2) as u32 & 0x7ffff) << 5) | dd)
}

/// `add xd, xn, #imm12 {, lsl #12}`.
pub fn addi(dd: u32, n: u32, imm12: u32, sh: u32) -> u32 {
    0x9100_0000 | (sh << 22) | (imm12 << 10) | (n << 5) | dd
}

/// `add xd, xn, xm`.
pub fn addr(dd: u32, n: u32, m: u32) -> u32 {
    0x8b00_0000 | (m << 16) | (n << 5) | dd
}

/// `sub xd, xn, #imm12 {, lsl #12}`.
pub fn subi(dd: u32, n: u32, imm12: u32, sh: u32) -> u32 {
    0xd100_0000 | (sh << 22) | (imm12 << 10) | (n << 5) | dd
}

/// `br xn`.
pub fn br(n: u32) -> u32 {
    0xd61f_0000 | (n << 5)
}

/// `cmp xn, xm`.
pub fn cmpr(n: u32, m: u32) -> u32 {
    0xeb00_001f | (m << 16) | (n << 5)
}

/// `b.eq` from `frm` to `to`.
pub fn beq(frm: i64, to: i64) -> Result<u32, Reach> {
    let off = to - frm;
    if !(-(1 << 20)..(1 << 20)).contains(&off) || off % 4 != 0 {
        return Err(Reach("b.eq out of reach"));
    }
    Ok(0x5400_0000 | (((off >> 2) as u32 & 0x7ffff) << 5))
}

/// `brk #imm`.
pub fn brk(imm: u32) -> u32 {
    0xd420_0000 | ((imm & 0xffff) << 5)
}

/// `blr xn`.
pub fn blr(n: u32) -> u32 {
    0xd63f_0000 | (n << 5)
}

/// `nop`.
pub const NOP: u32 = 0xd503_201f;
