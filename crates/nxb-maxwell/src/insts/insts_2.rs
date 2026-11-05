//! Produced by tools/gen_xlate_decoders.py from InstDecoders.cs: do not edit.
#![allow(non_upper_case_globals, non_camel_case_types, dead_code, unused_parens, clippy::all)]

use crate::enums::*;

#[derive(Clone, Copy, Debug)]
pub struct InstDmnmxR(pub u64);
impl InstDmnmxR {
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
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstDmnmxI(pub u64);
impl InstDmnmxI {
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
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstDmnmxC(pub u64);
impl InstDmnmxC {
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
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstDmulR(pub u64);
impl InstDmulR {
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
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstDmulI(pub u64);
impl InstDmulI {
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
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstDmulC(pub u64);
impl InstDmulC {
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
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstDsetR(pub u64);
impl InstDsetR {
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
    pub fn abs_a(&self) -> bool {
        (self.0 & 0x40000000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x20000000000000) != 0
    }
    pub fn b_val(&self) -> bool {
        (self.0 & 0x10000000000000) != 0
    }
    pub fn f_comp(&self) -> FComp {
        FComp(((self.0 >> 48) & 0xF) as i32)
    }
    pub fn bop(&self) -> BoolOp {
        BoolOp(((self.0 >> 45) & 0x3) as i32)
    }
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstDsetI(pub u64);
impl InstDsetI {
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
    pub fn abs_a(&self) -> bool {
        (self.0 & 0x40000000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x20000000000000) != 0
    }
    pub fn b_val(&self) -> bool {
        (self.0 & 0x10000000000000) != 0
    }
    pub fn f_comp(&self) -> FComp {
        FComp(((self.0 >> 48) & 0xF) as i32)
    }
    pub fn bop(&self) -> BoolOp {
        BoolOp(((self.0 >> 45) & 0x3) as i32)
    }
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstDsetC(pub u64);
impl InstDsetC {
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
    pub fn abs_a(&self) -> bool {
        (self.0 & 0x40000000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x20000000000000) != 0
    }
    pub fn b_val(&self) -> bool {
        (self.0 & 0x10000000000000) != 0
    }
    pub fn f_comp(&self) -> FComp {
        FComp(((self.0 >> 48) & 0xF) as i32)
    }
    pub fn bop(&self) -> BoolOp {
        BoolOp(((self.0 >> 45) & 0x3) as i32)
    }
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstDsetpR(pub u64);
impl InstDsetpR {
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
    pub fn f_comp(&self) -> FComp {
        FComp(((self.0 >> 48) & 0xF) as i32)
    }
    pub fn bop(&self) -> BoolOp {
        BoolOp(((self.0 >> 45) & 0x3) as i32)
    }
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn abs_a(&self) -> bool {
        (self.0 & 0x80) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x40) != 0
    }
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 3) & 0x7) as i32)
    }
    pub fn dest_pred_inv(&self) -> i32 {
        ((self.0 & 0x7) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstDsetpI(pub u64);
impl InstDsetpI {
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
    pub fn f_comp(&self) -> FComp {
        FComp(((self.0 >> 48) & 0xF) as i32)
    }
    pub fn bop(&self) -> BoolOp {
        BoolOp(((self.0 >> 45) & 0x3) as i32)
    }
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn abs_a(&self) -> bool {
        (self.0 & 0x80) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x40) != 0
    }
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 3) & 0x7) as i32)
    }
    pub fn dest_pred_inv(&self) -> i32 {
        ((self.0 & 0x7) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstDsetpC(pub u64);
impl InstDsetpC {
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
    pub fn f_comp(&self) -> FComp {
        FComp(((self.0 >> 48) & 0xF) as i32)
    }
    pub fn bop(&self) -> BoolOp {
        BoolOp(((self.0 >> 45) & 0x3) as i32)
    }
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn abs_a(&self) -> bool {
        (self.0 & 0x80) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x40) != 0
    }
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 3) & 0x7) as i32)
    }
    pub fn dest_pred_inv(&self) -> i32 {
        ((self.0 & 0x7) as i32)
    }
}

