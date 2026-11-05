//! Produced by tools/gen_xlate_decoders.py from InstDecoders.cs: do not edit.
#![allow(non_upper_case_globals, non_camel_case_types, dead_code, unused_parens, clippy::all)]

use crate::enums::*;

#[derive(Clone, Copy, Debug)]
pub struct InstBrk(pub u64);
impl InstBrk {
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
pub struct InstBrx(pub u64);
impl InstBrx {
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn ccc(&self) -> Ccc {
        Ccc((self.0 & 0x1F) as i32)
    }
    pub fn imm24(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFFFF) as i32)
    }
    pub fn ca(&self) -> bool {
        (self.0 & 0x20) != 0
    }
    pub fn lmt(&self) -> bool {
        (self.0 & 0x40) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstCal(pub u64);
impl InstCal {
    pub fn ca(&self) -> bool {
        (self.0 & 0x20) != 0
    }
    pub fn imm24(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFFFF) as i32)
    }
    pub fn inc(&self) -> bool {
        (self.0 & 0x40) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstCctl(pub u64);
impl InstCctl {
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn imm30(&self) -> i32 {
        (((self.0 >> 22) & 0x3FFFFFFF) as i32)
    }
    pub fn e(&self) -> bool {
        (self.0 & 0x10000000000000) != 0
    }
    pub fn cache(&self) -> CacheType {
        CacheType(((self.0 >> 4) & 0x7) as i32)
    }
    pub fn cctl_op(&self) -> CctlOp {
        CctlOp((self.0 & 0xF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstCctll(pub u64);
impl InstCctll {
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn imm22(&self) -> i32 {
        (((self.0 >> 22) & 0x3FFFFF) as i32)
    }
    pub fn cache(&self) -> i32 {
        (((self.0 >> 4) & 0x3) as i32)
    }
    pub fn cctl_op(&self) -> CctlOp {
        CctlOp((self.0 & 0xF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstCctlt(pub u64);
impl InstCctlt {
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn ts_idx13(&self) -> i32 {
        (((self.0 >> 36) & 0x1FFF) as i32)
    }
    pub fn cctlt_op(&self) -> CctltOp {
        CctltOp((self.0 & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstCctltR(pub u64);
impl InstCctltR {
    pub fn src_c(&self) -> i32 {
        (((self.0 >> 39) & 0xFF) as i32)
    }
    pub fn cctlt_op(&self) -> CctltOp {
        CctltOp((self.0 & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstCont(pub u64);
impl InstCont {
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
pub struct InstCset(pub u64);
impl InstCset {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
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
    pub fn ccc(&self) -> Ccc {
        Ccc(((self.0 >> 8) & 0x1F) as i32)
    }
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
    pub fn b_val(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn bop(&self) -> BoolOp {
        BoolOp(((self.0 >> 45) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstCsetp(pub u64);
impl InstCsetp {
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn write_cc(&self) -> bool {
        (self.0 & 0x800000000000) != 0
    }
    pub fn ccc(&self) -> Ccc {
        Ccc(((self.0 >> 8) & 0x1F) as i32)
    }
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 3) & 0x7) as i32)
    }
    pub fn dest_pred_inv(&self) -> i32 {
        ((self.0 & 0x7) as i32)
    }
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
    pub fn bop(&self) -> BoolOp {
        BoolOp(((self.0 >> 45) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstCs2r(pub u64);
impl InstCs2r {
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
pub struct InstDaddR(pub u64);
impl InstDaddR {
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
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn abs_a(&self) -> bool {
        (self.0 & 0x400000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x200000000000) != 0
    }
    pub fn round_mode(&self) -> RoundMode {
        RoundMode(((self.0 >> 39) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstDaddI(pub u64);
impl InstDaddI {
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
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn abs_a(&self) -> bool {
        (self.0 & 0x400000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x200000000000) != 0
    }
    pub fn round_mode(&self) -> RoundMode {
        RoundMode(((self.0 >> 39) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstDaddC(pub u64);
impl InstDaddC {
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
    pub fn write_cc(&self) -> bool {
        (self.0 & 0x800000000000) != 0
    }
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn abs_a(&self) -> bool {
        (self.0 & 0x400000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x200000000000) != 0
    }
    pub fn round_mode(&self) -> RoundMode {
        RoundMode(((self.0 >> 39) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstDepbar(pub u64);
impl InstDepbar {
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn le(&self) -> bool {
        (self.0 & 0x20000000) != 0
    }
    pub fn sbid(&self) -> i32 {
        (((self.0 >> 26) & 0x7) as i32)
    }
    pub fn pend_cnt(&self) -> i32 {
        (((self.0 >> 20) & 0x3F) as i32)
    }
    pub fn imm6(&self) -> i32 {
        ((self.0 & 0x3F) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstDfmaR(pub u64);
impl InstDfmaR {
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
    pub fn round_mode(&self) -> RoundMode {
        RoundMode(((self.0 >> 50) & 0x3) as i32)
    }
    pub fn neg_c(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstDfmaI(pub u64);
impl InstDfmaI {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn imm20(&self) -> i32 {
        (((self.0 >> 37) & 0x80000) as i32) | (((self.0 >> 20) & 0x7FFFF) as i32)
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
    pub fn round_mode(&self) -> RoundMode {
        RoundMode(((self.0 >> 50) & 0x3) as i32)
    }
    pub fn neg_c(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstDfmaC(pub u64);
impl InstDfmaC {
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
    pub fn round_mode(&self) -> RoundMode {
        RoundMode(((self.0 >> 50) & 0x3) as i32)
    }
    pub fn neg_c(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstDfmaRc(pub u64);
impl InstDfmaRc {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn src_c(&self) -> i32 {
        (((self.0 >> 39) & 0xFF) as i32)
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
    pub fn write_cc(&self) -> bool {
        (self.0 & 0x800000000000) != 0
    }
    pub fn round_mode(&self) -> RoundMode {
        RoundMode(((self.0 >> 50) & 0x3) as i32)
    }
    pub fn neg_c(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
}

