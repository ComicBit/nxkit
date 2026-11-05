//! Produced by tools/gen_xlate_decoders.py from InstDecoders.cs: do not edit.
#![allow(non_upper_case_globals, non_camel_case_types, dead_code, unused_parens, clippy::all)]

use crate::enums::*;

#[derive(Clone, Copy, Debug)]
pub struct InstExit(pub u64);
impl InstExit {
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn ccc(&self) -> Ccc {
        Ccc((self.0 & 0x1F) as i32)
    }
    pub fn keep_ref_cnt(&self) -> bool {
        (self.0 & 0x20) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstF2fR(pub u64);
impl InstF2fR {
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
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x200000000000) != 0
    }
    pub fn ftz(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn dst_fmt(&self) -> DstFmt {
        DstFmt(((self.0 >> 8) & 0x3) as i32)
    }
    pub fn src_fmt(&self) -> DstFmt {
        DstFmt(((self.0 >> 10) & 0x3) as i32)
    }
    pub fn round_mode(&self) -> IntegerRound {
        IntegerRound((((self.0 >> 40) & 0x4) as i32) | (((self.0 >> 39) & 0x3) as i32))
    }
    pub fn sh(&self) -> bool {
        (self.0 & 0x20000000000) != 0
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstF2fI(pub u64);
impl InstF2fI {
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
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x200000000000) != 0
    }
    pub fn ftz(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn dst_fmt(&self) -> DstFmt {
        DstFmt(((self.0 >> 8) & 0x3) as i32)
    }
    pub fn src_fmt(&self) -> DstFmt {
        DstFmt(((self.0 >> 10) & 0x3) as i32)
    }
    pub fn round_mode(&self) -> IntegerRound {
        IntegerRound((((self.0 >> 40) & 0x4) as i32) | (((self.0 >> 39) & 0x3) as i32))
    }
    pub fn sh(&self) -> bool {
        (self.0 & 0x20000000000) != 0
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstF2fC(pub u64);
impl InstF2fC {
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
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x200000000000) != 0
    }
    pub fn ftz(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn dst_fmt(&self) -> DstFmt {
        DstFmt(((self.0 >> 8) & 0x3) as i32)
    }
    pub fn src_fmt(&self) -> DstFmt {
        DstFmt(((self.0 >> 10) & 0x3) as i32)
    }
    pub fn round_mode(&self) -> IntegerRound {
        IntegerRound((((self.0 >> 40) & 0x4) as i32) | (((self.0 >> 39) & 0x3) as i32))
    }
    pub fn sh(&self) -> bool {
        (self.0 & 0x20000000000) != 0
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstF2iR(pub u64);
impl InstF2iR {
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
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x200000000000) != 0
    }
    pub fn ftz(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn sh(&self) -> bool {
        (self.0 & 0x20000000000) != 0
    }
    pub fn i_dst_fmt(&self) -> IDstFmt {
        IDstFmt((((self.0 >> 10) & 0x4) as i32) | (((self.0 >> 8) & 0x3) as i32))
    }
    pub fn src_fmt(&self) -> DstFmt {
        DstFmt(((self.0 >> 10) & 0x3) as i32)
    }
    pub fn round_mode(&self) -> RoundMode2 {
        RoundMode2(((self.0 >> 39) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstF2iI(pub u64);
impl InstF2iI {
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
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x200000000000) != 0
    }
    pub fn ftz(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn sh(&self) -> bool {
        (self.0 & 0x20000000000) != 0
    }
    pub fn i_dst_fmt(&self) -> IDstFmt {
        IDstFmt((((self.0 >> 10) & 0x4) as i32) | (((self.0 >> 8) & 0x3) as i32))
    }
    pub fn src_fmt(&self) -> DstFmt {
        DstFmt(((self.0 >> 10) & 0x3) as i32)
    }
    pub fn round_mode(&self) -> RoundMode2 {
        RoundMode2(((self.0 >> 39) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstF2iC(pub u64);
impl InstF2iC {
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
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x200000000000) != 0
    }
    pub fn ftz(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn sh(&self) -> bool {
        (self.0 & 0x20000000000) != 0
    }
    pub fn i_dst_fmt(&self) -> IDstFmt {
        IDstFmt((((self.0 >> 10) & 0x4) as i32) | (((self.0 >> 8) & 0x3) as i32))
    }
    pub fn src_fmt(&self) -> DstFmt {
        DstFmt(((self.0 >> 10) & 0x3) as i32)
    }
    pub fn round_mode(&self) -> RoundMode2 {
        RoundMode2(((self.0 >> 39) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFaddR(pub u64);
impl InstFaddR {
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
    pub fn sat(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
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
    pub fn round_mode(&self) -> RoundMode {
        RoundMode(((self.0 >> 39) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFaddI(pub u64);
impl InstFaddI {
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
    pub fn sat(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
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
    pub fn round_mode(&self) -> RoundMode {
        RoundMode(((self.0 >> 39) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFaddC(pub u64);
impl InstFaddC {
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
    pub fn sat(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
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
    pub fn round_mode(&self) -> RoundMode {
        RoundMode(((self.0 >> 39) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFadd32i(pub u64);
impl InstFadd32i {
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
    pub fn write_cc(&self) -> bool {
        (self.0 & 0x10000000000000) != 0
    }
    pub fn imm32(&self) -> i32 {
        ((self.0 >> 20) as i32)
    }
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x200000000000000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x100000000000000) != 0
    }
    pub fn ftz(&self) -> bool {
        (self.0 & 0x80000000000000) != 0
    }
    pub fn abs_a(&self) -> bool {
        (self.0 & 0x40000000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x20000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstFchkR(pub u64);
impl InstFchkR {
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 3) & 0x7) as i32)
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
pub struct InstFchkI(pub u64);
impl InstFchkI {
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 3) & 0x7) as i32)
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

