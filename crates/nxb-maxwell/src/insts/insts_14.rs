//! Produced by tools/gen_xlate_decoders.py from InstDecoders.cs: do not edit.
#![allow(non_upper_case_globals, non_camel_case_types, dead_code, unused_parens, clippy::all)]

use crate::enums::*;

#[derive(Clone, Copy, Debug)]
pub struct InstR2pC(pub u64);
impl InstR2pC {
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn cbuf_slot(&self) -> i32 {
        (((self.0 >> 34) & 0x1F) as i32)
    }
    pub fn cbuf_offset(&self) -> i32 {
        (((self.0 >> 20) & 0x3FFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn byte_sel(&self) -> ByteSel {
        ByteSel(((self.0 >> 41) & 0x3) as i32)
    }
    pub fn ccpr(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstRam(pub u64);
impl InstRam {
}

#[derive(Clone, Copy, Debug)]
pub struct InstRed(pub u64);
impl InstRed {
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn src_b(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn imm20(&self) -> i32 {
        (((self.0 >> 28) & 0xFFFFF) as i32)
    }
    pub fn red_size(&self) -> AtomSize {
        AtomSize(((self.0 >> 20) & 0x7) as i32)
    }
    pub fn red_op(&self) -> RedOp {
        RedOp(((self.0 >> 23) & 0x7) as i32)
    }
    pub fn e(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstRet(pub u64);
impl InstRet {
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn ccc(&self) -> Ccc {
        Ccc((self.0 & 0x1F) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstRroR(pub u64);
impl InstRroR {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn src_b(&self) -> i32 {
        (((self.0 >> 20) & 0xFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x200000000000) != 0
    }
    pub fn rro_op(&self) -> bool {
        (self.0 & 0x8000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstRroI(pub u64);
impl InstRroI {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn imm20(&self) -> i32 {
        (((self.0 >> 37) & 0x80000) as i32) | (((self.0 >> 20) & 0x7FFFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x200000000000) != 0
    }
    pub fn rro_op(&self) -> bool {
        (self.0 & 0x8000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstRroC(pub u64);
impl InstRroC {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn cbuf_slot(&self) -> i32 {
        (((self.0 >> 34) & 0x1F) as i32)
    }
    pub fn cbuf_offset(&self) -> i32 {
        (((self.0 >> 20) & 0x3FFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x200000000000) != 0
    }
    pub fn rro_op(&self) -> bool {
        (self.0 & 0x8000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstRtt(pub u64);
impl InstRtt {
}

#[derive(Clone, Copy, Debug)]
pub struct InstS2r(pub u64);
impl InstS2r {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn s_reg(&self) -> SReg {
        SReg(((self.0 >> 20) & 0xFF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstSam(pub u64);
impl InstSam {
}

#[derive(Clone, Copy, Debug)]
pub struct InstSelR(pub u64);
impl InstSelR {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn src_b(&self) -> i32 {
        (((self.0 >> 20) & 0xFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstSelI(pub u64);
impl InstSelI {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn imm20(&self) -> i32 {
        (((self.0 >> 37) & 0x80000) as i32) | (((self.0 >> 20) & 0x7FFFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstSelC(pub u64);
impl InstSelC {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn cbuf_slot(&self) -> i32 {
        (((self.0 >> 34) & 0x1F) as i32)
    }
    pub fn cbuf_offset(&self) -> i32 {
        (((self.0 >> 20) & 0x3FFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstSetcrsptr(pub u64);
impl InstSetcrsptr {
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstSetlmembase(pub u64);
impl InstSetlmembase {
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstShfLR(pub u64);
impl InstShfLR {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn src_b(&self) -> i32 {
        (((self.0 >> 20) & 0xFF) as i32)
    }
    pub fn src_c(&self) -> i32 {
        (((self.0 >> 39) & 0xFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn write_cc(&self) -> bool {
        (self.0 & 0x800000000000) != 0
    }
    pub fn m(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn x_mode_shf(&self) -> XModeShf {
        XModeShf(((self.0 >> 48) & 0x3) as i32)
    }
    pub fn max_shift(&self) -> MaxShift {
        MaxShift(((self.0 >> 37) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstShfRR(pub u64);
impl InstShfRR {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn src_b(&self) -> i32 {
        (((self.0 >> 20) & 0xFF) as i32)
    }
    pub fn src_c(&self) -> i32 {
        (((self.0 >> 39) & 0xFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn write_cc(&self) -> bool {
        (self.0 & 0x800000000000) != 0
    }
    pub fn m(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn x_mode_shf(&self) -> XModeShf {
        XModeShf(((self.0 >> 48) & 0x3) as i32)
    }
    pub fn max_shift(&self) -> MaxShift {
        MaxShift(((self.0 >> 37) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstShfLI(pub u64);
impl InstShfLI {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn src_c(&self) -> i32 {
        (((self.0 >> 39) & 0xFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn write_cc(&self) -> bool {
        (self.0 & 0x800000000000) != 0
    }
    pub fn m(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn x_mode_shf(&self) -> XModeShf {
        XModeShf(((self.0 >> 48) & 0x3) as i32)
    }
    pub fn max_shift(&self) -> MaxShift {
        MaxShift(((self.0 >> 37) & 0x3) as i32)
    }
    pub fn imm6(&self) -> i32 {
        (((self.0 >> 20) & 0x3F) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstShfRI(pub u64);
impl InstShfRI {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn src_c(&self) -> i32 {
        (((self.0 >> 39) & 0xFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn write_cc(&self) -> bool {
        (self.0 & 0x800000000000) != 0
    }
    pub fn m(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn x_mode_shf(&self) -> XModeShf {
        XModeShf(((self.0 >> 48) & 0x3) as i32)
    }
    pub fn max_shift(&self) -> MaxShift {
        MaxShift(((self.0 >> 37) & 0x3) as i32)
    }
    pub fn imm6(&self) -> i32 {
        (((self.0 >> 20) & 0x3F) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstShfl(pub u64);
impl InstShfl {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn src_b(&self) -> i32 {
        (((self.0 >> 20) & 0xFF) as i32)
    }
    pub fn src_c(&self) -> i32 {
        (((self.0 >> 39) & 0xFF) as i32)
    }
    pub fn src_b_imm(&self) -> i32 {
        (((self.0 >> 20) & 0x1F) as i32)
    }
    pub fn src_c_imm(&self) -> i32 {
        (((self.0 >> 34) & 0x1FFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn shfl_mode(&self) -> ShflMode {
        ShflMode(((self.0 >> 30) & 0x3) as i32)
    }
    pub fn c_fix_shfl(&self) -> bool {
        (self.0 & 0x20000000) != 0
    }
    pub fn b_fix_shfl(&self) -> bool {
        (self.0 & 0x10000000) != 0
    }
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 48) & 0x7) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstShlR(pub u64);
impl InstShlR {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn src_b(&self) -> i32 {
        (((self.0 >> 20) & 0xFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn write_cc(&self) -> bool {
        (self.0 & 0x800000000000) != 0
    }
    pub fn x(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
    pub fn m(&self) -> bool {
        (self.0 & 0x8000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstShlI(pub u64);
impl InstShlI {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn imm20(&self) -> i32 {
        (((self.0 >> 37) & 0x80000) as i32) | (((self.0 >> 20) & 0x7FFFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn write_cc(&self) -> bool {
        (self.0 & 0x800000000000) != 0
    }
    pub fn x(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
    pub fn m(&self) -> bool {
        (self.0 & 0x8000000000) != 0
    }
}

