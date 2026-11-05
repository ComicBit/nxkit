//! Produced by tools/gen_xlate_decoders.py from InstDecoders.cs: do not edit.
#![allow(non_upper_case_globals, non_camel_case_types, dead_code, unused_parens, clippy::all)]

use crate::enums::*;

#[derive(Clone, Copy, Debug)]
pub struct InstLeaHiC(pub u64);
impl InstLeaHiC {
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
    pub fn x(&self) -> bool {
        (self.0 & 0x200000000000000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x100000000000000) != 0
    }
    pub fn imm_u5(&self) -> i32 {
        (((self.0 >> 51) & 0x1F) as i32)
    }
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 48) & 0x7) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstLepc(pub u64);
impl InstLepc {
}

#[derive(Clone, Copy, Debug)]
pub struct InstLongjmp(pub u64);
impl InstLongjmp {
    pub fn ccc(&self) -> Ccc {
        Ccc((self.0 & 0x1F) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstLopR(pub u64);
impl InstLopR {
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
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 48) & 0x7) as i32)
    }
    pub fn predicate_op(&self) -> PredicateOp {
        PredicateOp(((self.0 >> 44) & 0x3) as i32)
    }
    pub fn x(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
    pub fn lop(&self) -> LogicOp {
        LogicOp(((self.0 >> 41) & 0x3) as i32)
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x8000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstLopI(pub u64);
impl InstLopI {
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
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 48) & 0x7) as i32)
    }
    pub fn predicate_op(&self) -> PredicateOp {
        PredicateOp(((self.0 >> 44) & 0x3) as i32)
    }
    pub fn x(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
    pub fn logic_op(&self) -> LogicOp {
        LogicOp(((self.0 >> 41) & 0x3) as i32)
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x8000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstLopC(pub u64);
impl InstLopC {
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
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 48) & 0x7) as i32)
    }
    pub fn predicate_op(&self) -> PredicateOp {
        PredicateOp(((self.0 >> 44) & 0x3) as i32)
    }
    pub fn x(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
    pub fn logic_op(&self) -> LogicOp {
        LogicOp(((self.0 >> 41) & 0x3) as i32)
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x8000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstLop3R(pub u64);
impl InstLop3R {
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
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 48) & 0x7) as i32)
    }
    pub fn predicate_op(&self) -> PredicateOp {
        PredicateOp(((self.0 >> 36) & 0x3) as i32)
    }
    pub fn x(&self) -> bool {
        (self.0 & 0x4000000000) != 0
    }
    pub fn imm(&self) -> i32 {
        (((self.0 >> 28) & 0xFF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstLop3I(pub u64);
impl InstLop3I {
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
    pub fn x(&self) -> bool {
        (self.0 & 0x200000000000000) != 0
    }
    pub fn imm(&self) -> i32 {
        (((self.0 >> 48) & 0xFF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstLop3C(pub u64);
impl InstLop3C {
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
    pub fn x(&self) -> bool {
        (self.0 & 0x100000000000000) != 0
    }
    pub fn imm(&self) -> i32 {
        (((self.0 >> 48) & 0xFF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstLop32i(pub u64);
impl InstLop32i {
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
    pub fn x(&self) -> bool {
        (self.0 & 0x200000000000000) != 0
    }
    pub fn logic_op(&self) -> LogicOp {
        LogicOp(((self.0 >> 53) & 0x3) as i32)
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x80000000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x100000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstMembar(pub u64);
impl InstMembar {
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn membar(&self) -> Membar {
        Membar(((self.0 >> 8) & 0x3) as i32)
    }
    pub fn ivall(&self) -> Ivall {
        Ivall((self.0 & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstMovR(pub u64);
impl InstMovR {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 20) & 0xFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn quad_mask(&self) -> i32 {
        (((self.0 >> 39) & 0xF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstMovI(pub u64);
impl InstMovI {
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
    pub fn quad_mask(&self) -> i32 {
        (((self.0 >> 39) & 0xF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstMovC(pub u64);
impl InstMovC {
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
    pub fn quad_mask(&self) -> i32 {
        (((self.0 >> 39) & 0xF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstMov32i(pub u64);
impl InstMov32i {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
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
    pub fn quad_mask(&self) -> i32 {
        (((self.0 >> 12) & 0xF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstMufu(pub u64);
impl InstMufu {
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
    pub fn mufu_op(&self) -> MufuOp {
        MufuOp(((self.0 >> 20) & 0xF) as i32)
    }
    pub fn abs_a(&self) -> bool {
        (self.0 & 0x400000000000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstNop(pub u64);
impl InstNop {
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn imm16(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFF) as i32)
    }
    pub fn trig(&self) -> bool {
        (self.0 & 0x2000) != 0
    }
    pub fn ccc(&self) -> Ccc {
        Ccc(((self.0 >> 8) & 0x1F) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstOutR(pub u64);
impl InstOutR {
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
    pub fn out_type(&self) -> OutType {
        OutType(((self.0 >> 39) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstOutI(pub u64);
impl InstOutI {
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
    pub fn out_type(&self) -> OutType {
        OutType(((self.0 >> 39) & 0x3) as i32)
    }
}

