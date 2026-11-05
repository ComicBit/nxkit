//! Produced by tools/gen_xlate_decoders.py from InstDecoders.cs: do not edit.
#![allow(non_upper_case_globals, non_camel_case_types, dead_code, unused_parens, clippy::all)]

use crate::enums::*;

#[derive(Clone, Copy, Debug)]
pub struct InstFchkC(pub u64);
impl InstFchkC {
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 3) & 0x7) as i32)
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
    pub fn chk_mode_f(&self) -> ChkModeF {
        ChkModeF(((self.0 >> 39) & 0x3F) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFcmpR(pub u64);
impl InstFcmpR {
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
    pub fn f_comp(&self) -> FComp {
        FComp(((self.0 >> 48) & 0xF) as i32)
    }
    pub fn ftz(&self) -> bool {
        (self.0 & 0x800000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFcmpI(pub u64);
impl InstFcmpI {
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
    pub fn f_comp(&self) -> FComp {
        FComp(((self.0 >> 48) & 0xF) as i32)
    }
    pub fn ftz(&self) -> bool {
        (self.0 & 0x800000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFcmpC(pub u64);
impl InstFcmpC {
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
    pub fn f_comp(&self) -> FComp {
        FComp(((self.0 >> 48) & 0xF) as i32)
    }
    pub fn ftz(&self) -> bool {
        (self.0 & 0x800000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFcmpRc(pub u64);
impl InstFcmpRc {
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
    pub fn f_comp(&self) -> FComp {
        FComp(((self.0 >> 48) & 0xF) as i32)
    }
    pub fn ftz(&self) -> bool {
        (self.0 & 0x800000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFfmaR(pub u64);
impl InstFfmaR {
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
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn neg_c(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn round_mode(&self) -> RoundMode {
        RoundMode(((self.0 >> 51) & 0x3) as i32)
    }
    pub fn fmz(&self) -> Fmz {
        Fmz(((self.0 >> 53) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFfmaI(pub u64);
impl InstFfmaI {
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
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn neg_c(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn round_mode(&self) -> RoundMode {
        RoundMode(((self.0 >> 51) & 0x3) as i32)
    }
    pub fn fmz(&self) -> Fmz {
        Fmz(((self.0 >> 53) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFfmaC(pub u64);
impl InstFfmaC {
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
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn neg_c(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn round_mode(&self) -> RoundMode {
        RoundMode(((self.0 >> 51) & 0x3) as i32)
    }
    pub fn fmz(&self) -> Fmz {
        Fmz(((self.0 >> 53) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFfmaRc(pub u64);
impl InstFfmaRc {
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
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn neg_c(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn round_mode(&self) -> RoundMode {
        RoundMode(((self.0 >> 51) & 0x3) as i32)
    }
    pub fn fmz(&self) -> Fmz {
        Fmz(((self.0 >> 53) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFfma32i(pub u64);
impl InstFfma32i {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn imm32(&self) -> i32 {
        ((self.0 >> 20) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn neg_c(&self) -> bool {
        (self.0 & 0x200000000000000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x100000000000000) != 0
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x80000000000000) != 0
    }
    pub fn write_cc(&self) -> bool {
        (self.0 & 0x10000000000000) != 0
    }
    pub fn fmz(&self) -> Fmz {
        Fmz(((self.0 >> 53) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFloR(pub u64);
impl InstFloR {
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
    pub fn write_cc(&self) -> bool {
        (self.0 & 0x800000000000) != 0
    }
    pub fn signed(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn sh(&self) -> bool {
        (self.0 & 0x20000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFloI(pub u64);
impl InstFloI {
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
    pub fn write_cc(&self) -> bool {
        (self.0 & 0x800000000000) != 0
    }
    pub fn signed(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn sh(&self) -> bool {
        (self.0 & 0x20000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFloC(pub u64);
impl InstFloC {
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
    pub fn write_cc(&self) -> bool {
        (self.0 & 0x800000000000) != 0
    }
    pub fn signed(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn sh(&self) -> bool {
        (self.0 & 0x20000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFmnmxR(pub u64);
impl InstFmnmxR {
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

