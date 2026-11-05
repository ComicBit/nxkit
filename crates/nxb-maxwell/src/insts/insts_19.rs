//! Produced by tools/gen_xlate_decoders.py from InstDecoders.cs: do not edit.
#![allow(non_upper_case_globals, non_camel_case_types, dead_code, unused_parens, clippy::all)]

use crate::enums::*;

#[derive(Clone, Copy, Debug)]
pub struct InstXmadRc(pub u64);
impl InstXmadRc {
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
    pub fn x(&self) -> bool {
        (self.0 & 0x40000000000000) != 0
    }
    pub fn hilo_a(&self) -> bool {
        (self.0 & 0x20000000000000) != 0
    }
    pub fn hilo_b(&self) -> bool {
        (self.0 & 0x10000000000000) != 0
    }
    pub fn xmad_cop(&self) -> XmadCop2 {
        XmadCop2(((self.0 >> 50) & 0x3) as i32)
    }
    pub fn b_signed(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn a_signed(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
}

