//! Produced by tools/gen_xlate_decoders.py from InstDecoders.cs: do not edit.
#![allow(non_upper_case_globals, non_camel_case_types, dead_code, unused_parens, clippy::all)]

use crate::enums::*;

#[derive(Clone, Copy, Debug)]
pub struct InstFmnmxI(pub u64);
impl InstFmnmxI {
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
    pub fn ftz(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFmnmxC(pub u64);
impl InstFmnmxC {
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
    pub fn ftz(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFmulR(pub u64);
impl InstFmulR {
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
    pub fn round_mode(&self) -> RoundMode {
        RoundMode(((self.0 >> 39) & 0x3) as i32)
    }
    pub fn fmz(&self) -> Fmz {
        Fmz(((self.0 >> 44) & 0x3) as i32)
    }
    pub fn scale(&self) -> MultiplyScale {
        MultiplyScale(((self.0 >> 41) & 0x7) as i32)
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFmulI(pub u64);
impl InstFmulI {
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
    pub fn round_mode(&self) -> RoundMode {
        RoundMode(((self.0 >> 39) & 0x3) as i32)
    }
    pub fn fmz(&self) -> Fmz {
        Fmz(((self.0 >> 44) & 0x3) as i32)
    }
    pub fn scale(&self) -> MultiplyScale {
        MultiplyScale(((self.0 >> 41) & 0x7) as i32)
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFmulC(pub u64);
impl InstFmulC {
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
    pub fn round_mode(&self) -> RoundMode {
        RoundMode(((self.0 >> 39) & 0x3) as i32)
    }
    pub fn fmz(&self) -> Fmz {
        Fmz(((self.0 >> 44) & 0x3) as i32)
    }
    pub fn scale(&self) -> MultiplyScale {
        MultiplyScale(((self.0 >> 41) & 0x7) as i32)
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFmul32i(pub u64);
impl InstFmul32i {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn imm32(&self) -> i32 {
        ((self.0 >> 20) as i32)
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x80000000000000) != 0
    }
    pub fn fmz(&self) -> Fmz {
        Fmz(((self.0 >> 53) & 0x3) as i32)
    }
    pub fn write_cc(&self) -> bool {
        (self.0 & 0x10000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFsetR(pub u64);
impl InstFsetR {
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
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x20000000000000) != 0
    }
    pub fn abs_a(&self) -> bool {
        (self.0 & 0x40000000000000) != 0
    }
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn f_comp(&self) -> FComp {
        FComp(((self.0 >> 48) & 0xF) as i32)
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
    pub fn ftz(&self) -> bool {
        (self.0 & 0x80000000000000) != 0
    }
    pub fn b_val(&self) -> bool {
        (self.0 & 0x10000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFsetC(pub u64);
impl InstFsetC {
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
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x20000000000000) != 0
    }
    pub fn abs_a(&self) -> bool {
        (self.0 & 0x40000000000000) != 0
    }
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn f_comp(&self) -> FComp {
        FComp(((self.0 >> 48) & 0xF) as i32)
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
    pub fn ftz(&self) -> bool {
        (self.0 & 0x80000000000000) != 0
    }
    pub fn b_val(&self) -> bool {
        (self.0 & 0x10000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFsetI(pub u64);
impl InstFsetI {
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
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x20000000000000) != 0
    }
    pub fn abs_a(&self) -> bool {
        (self.0 & 0x40000000000000) != 0
    }
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn f_comp(&self) -> FComp {
        FComp(((self.0 >> 48) & 0xF) as i32)
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
    pub fn ftz(&self) -> bool {
        (self.0 & 0x80000000000000) != 0
    }
    pub fn b_val(&self) -> bool {
        (self.0 & 0x10000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFsetpR(pub u64);
impl InstFsetpR {
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 3) & 0x7) as i32)
    }
    pub fn dest_pred_inv(&self) -> i32 {
        ((self.0 & 0x7) as i32)
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
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x40) != 0
    }
    pub fn abs_a(&self) -> bool {
        (self.0 & 0x80) != 0
    }
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn f_comp(&self) -> FComp {
        FComp(((self.0 >> 48) & 0xF) as i32)
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
    pub fn ftz(&self) -> bool {
        (self.0 & 0x800000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFsetpI(pub u64);
impl InstFsetpI {
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 3) & 0x7) as i32)
    }
    pub fn dest_pred_inv(&self) -> i32 {
        ((self.0 & 0x7) as i32)
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
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x40) != 0
    }
    pub fn abs_a(&self) -> bool {
        (self.0 & 0x80) != 0
    }
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn f_comp(&self) -> FComp {
        FComp(((self.0 >> 48) & 0xF) as i32)
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
    pub fn ftz(&self) -> bool {
        (self.0 & 0x800000000000) != 0
    }
}

