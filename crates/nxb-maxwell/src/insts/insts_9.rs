//! Produced by tools/gen_xlate_decoders.py from InstDecoders.cs: do not edit.
#![allow(non_upper_case_globals, non_camel_case_types, dead_code, unused_parens, clippy::all)]

use crate::enums::*;

#[derive(Clone, Copy, Debug)]
pub struct InstIcmpRc(pub u64);
impl InstIcmpRc {
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
    pub fn i_comp(&self) -> IComp {
        IComp(((self.0 >> 49) & 0x7) as i32)
    }
    pub fn signed(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstIde(pub u64);
impl InstIde {
    pub fn imm16(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFF) as i32)
    }
    pub fn di(&self) -> bool {
        (self.0 & 0x20) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstIdpR(pub u64);
impl InstIdpR {
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
    pub fn is_hi(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn src_a_sign(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn is_dp(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn src_b_sign(&self) -> bool {
        (self.0 & 0x800000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstIdpC(pub u64);
impl InstIdpC {
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
    pub fn is_hi(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn src_a_sign(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn is_dp(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn src_b_sign(&self) -> bool {
        (self.0 & 0x800000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstImadR(pub u64);
impl InstImadR {
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
    pub fn hilo(&self) -> bool {
        (self.0 & 0x40000000000000) != 0
    }
    pub fn b_signed(&self) -> bool {
        (self.0 & 0x20000000000000) != 0
    }
    pub fn avg_mode(&self) -> AvgMode {
        AvgMode(((self.0 >> 51) & 0x3) as i32)
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn x(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn a_signed(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstImadI(pub u64);
impl InstImadI {
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
    pub fn hilo(&self) -> bool {
        (self.0 & 0x40000000000000) != 0
    }
    pub fn b_signed(&self) -> bool {
        (self.0 & 0x20000000000000) != 0
    }
    pub fn avg_mode(&self) -> AvgMode {
        AvgMode(((self.0 >> 51) & 0x3) as i32)
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn x(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn a_signed(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstImadC(pub u64);
impl InstImadC {
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
    pub fn hilo(&self) -> bool {
        (self.0 & 0x40000000000000) != 0
    }
    pub fn b_signed(&self) -> bool {
        (self.0 & 0x20000000000000) != 0
    }
    pub fn avg_mode(&self) -> AvgMode {
        AvgMode(((self.0 >> 51) & 0x3) as i32)
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn x(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn a_signed(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstImadRc(pub u64);
impl InstImadRc {
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
    pub fn hilo(&self) -> bool {
        (self.0 & 0x40000000000000) != 0
    }
    pub fn b_signed(&self) -> bool {
        (self.0 & 0x20000000000000) != 0
    }
    pub fn avg_mode(&self) -> AvgMode {
        AvgMode(((self.0 >> 51) & 0x3) as i32)
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn x(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn a_signed(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstImad32i(pub u64);
impl InstImad32i {
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
    pub fn b_signed(&self) -> bool {
        (self.0 & 0x200000000000000) != 0
    }
    pub fn avg_mode(&self) -> AvgMode {
        AvgMode(((self.0 >> 55) & 0x3) as i32)
    }
    pub fn a_signed(&self) -> bool {
        (self.0 & 0x40000000000000) != 0
    }
    pub fn write_cc(&self) -> bool {
        (self.0 & 0x10000000000000) != 0
    }
    pub fn hilo(&self) -> bool {
        (self.0 & 0x20000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstImadspR(pub u64);
impl InstImadspR {
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
    pub fn a_select(&self) -> ImadspASelect {
        ImadspASelect(((self.0 >> 48) & 0x7) as i32)
    }
    pub fn b_select(&self) -> ImadspBSelect {
        ImadspBSelect(((self.0 >> 53) & 0x3) as i32)
    }
    pub fn c_select(&self) -> ImadspASelect {
        ImadspASelect((((self.0 >> 50) & 0x6) as i32) | (((self.0 >> 48) & 0x1) as i32))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstImadspI(pub u64);
impl InstImadspI {
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
    pub fn a_select(&self) -> ImadspASelect {
        ImadspASelect(((self.0 >> 48) & 0x7) as i32)
    }
    pub fn b_select(&self) -> ImadspBSelect {
        ImadspBSelect(((self.0 >> 53) & 0x3) as i32)
    }
    pub fn c_select(&self) -> ImadspASelect {
        ImadspASelect((((self.0 >> 50) & 0x6) as i32) | (((self.0 >> 48) & 0x1) as i32))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstImadspC(pub u64);
impl InstImadspC {
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
    pub fn a_select(&self) -> ImadspASelect {
        ImadspASelect(((self.0 >> 48) & 0x7) as i32)
    }
    pub fn b_select(&self) -> ImadspBSelect {
        ImadspBSelect(((self.0 >> 53) & 0x3) as i32)
    }
    pub fn c_select(&self) -> ImadspASelect {
        ImadspASelect((((self.0 >> 50) & 0x6) as i32) | (((self.0 >> 48) & 0x1) as i32))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstImadspRc(pub u64);
impl InstImadspRc {
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
    pub fn a_select(&self) -> ImadspASelect {
        ImadspASelect(((self.0 >> 48) & 0x7) as i32)
    }
    pub fn b_select(&self) -> ImadspBSelect {
        ImadspBSelect(((self.0 >> 53) & 0x3) as i32)
    }
    pub fn c_select(&self) -> ImadspASelect {
        ImadspASelect((((self.0 >> 50) & 0x6) as i32) | (((self.0 >> 48) & 0x1) as i32))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstImnmxR(pub u64);
impl InstImnmxR {
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

