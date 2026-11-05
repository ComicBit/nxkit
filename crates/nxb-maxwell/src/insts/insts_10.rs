//! Produced by tools/gen_xlate_decoders.py from InstDecoders.cs: do not edit.
#![allow(non_upper_case_globals, non_camel_case_types, dead_code, unused_parens, clippy::all)]

use crate::enums::*;

#[derive(Clone, Copy, Debug)]
pub struct InstImnmxI(pub u64);
impl InstImnmxI {
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
    pub fn signed(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn x_mode(&self) -> XMode {
        XMode(((self.0 >> 43) & 0x3) as i32)
    }
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstImnmxC(pub u64);
impl InstImnmxC {
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
    pub fn signed(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn x_mode(&self) -> XMode {
        XMode(((self.0 >> 43) & 0x3) as i32)
    }
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstImulR(pub u64);
impl InstImulR {
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
    pub fn a_signed(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
    pub fn b_signed(&self) -> bool {
        (self.0 & 0x20000000000) != 0
    }
    pub fn hilo(&self) -> bool {
        (self.0 & 0x8000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstImulI(pub u64);
impl InstImulI {
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
    pub fn a_signed(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
    pub fn b_signed(&self) -> bool {
        (self.0 & 0x20000000000) != 0
    }
    pub fn hilo(&self) -> bool {
        (self.0 & 0x8000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstImulC(pub u64);
impl InstImulC {
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
    pub fn a_signed(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
    pub fn b_signed(&self) -> bool {
        (self.0 & 0x20000000000) != 0
    }
    pub fn hilo(&self) -> bool {
        (self.0 & 0x8000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstImul32i(pub u64);
impl InstImul32i {
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
    pub fn a_signed(&self) -> bool {
        (self.0 & 0x40000000000000) != 0
    }
    pub fn b_signed(&self) -> bool {
        (self.0 & 0x80000000000000) != 0
    }
    pub fn hilo(&self) -> bool {
        (self.0 & 0x20000000000000) != 0
    }
    pub fn write_cc(&self) -> bool {
        (self.0 & 0x10000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstIpa(pub u64);
impl InstIpa {
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
    pub fn ipa_op(&self) -> IpaOp {
        IpaOp(((self.0 >> 54) & 0x3) as i32)
    }
    pub fn msi(&self) -> i32 {
        (((self.0 >> 52) & 0x3) as i32)
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x8000000000000) != 0
    }
    pub fn idx(&self) -> bool {
        (self.0 & 0x4000000000) != 0
    }
    pub fn imm10(&self) -> i32 {
        (((self.0 >> 28) & 0x3FF) as i32)
    }
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 47) & 0x7) as i32)
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstIsberd(pub u64);
impl InstIsberd {
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
    pub fn al_size(&self) -> AlSize {
        AlSize(((self.0 >> 47) & 0x3) as i32)
    }
    pub fn i_base(&self) -> IBase {
        IBase(((self.0 >> 33) & 0x3) as i32)
    }
    pub fn o(&self) -> bool {
        (self.0 & 0x100000000) != 0
    }
    pub fn p(&self) -> bool {
        (self.0 & 0x80000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstIscaddR(pub u64);
impl InstIscaddR {
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
    pub fn imm5(&self) -> i32 {
        (((self.0 >> 39) & 0x1F) as i32)
    }
    pub fn avg_mode(&self) -> AvgMode {
        AvgMode(((self.0 >> 48) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstIscaddI(pub u64);
impl InstIscaddI {
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
    pub fn imm5(&self) -> i32 {
        (((self.0 >> 39) & 0x1F) as i32)
    }
    pub fn avg_mode(&self) -> AvgMode {
        AvgMode(((self.0 >> 48) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstIscaddC(pub u64);
impl InstIscaddC {
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
    pub fn imm5(&self) -> i32 {
        (((self.0 >> 39) & 0x1F) as i32)
    }
    pub fn avg_mode(&self) -> AvgMode {
        AvgMode(((self.0 >> 48) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstIscadd32i(pub u64);
impl InstIscadd32i {
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
    pub fn write_cc(&self) -> bool {
        (self.0 & 0x10000000000000) != 0
    }
    pub fn imm5(&self) -> i32 {
        (((self.0 >> 53) & 0x1F) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstIsetR(pub u64);
impl InstIsetR {
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
    pub fn i_comp(&self) -> IComp {
        IComp(((self.0 >> 49) & 0x7) as i32)
    }
    pub fn signed(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
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
    pub fn x(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstIsetI(pub u64);
impl InstIsetI {
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
    pub fn i_comp(&self) -> IComp {
        IComp(((self.0 >> 49) & 0x7) as i32)
    }
    pub fn signed(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
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
    pub fn x(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstIsetC(pub u64);
impl InstIsetC {
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
    pub fn i_comp(&self) -> IComp {
        IComp(((self.0 >> 49) & 0x7) as i32)
    }
    pub fn signed(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
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
    pub fn x(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
}

