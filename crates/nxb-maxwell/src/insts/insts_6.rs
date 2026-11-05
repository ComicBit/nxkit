//! Produced by tools/gen_xlate_decoders.py from InstDecoders.cs: do not edit.
#![allow(non_upper_case_globals, non_camel_case_types, dead_code, unused_parens, clippy::all)]

use crate::enums::*;

#[derive(Clone, Copy, Debug)]
pub struct InstFsetpC(pub u64);
impl InstFsetpC {
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 3) & 0x7) as i32)
    }
    pub fn dest_pred_inv(&self) -> i32 {
        ((self.0 & 0x7) as i32)
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
pub struct InstFswzadd(pub u64);
impl InstFswzadd {
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
    pub fn ftz(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn round_mode(&self) -> RoundMode {
        RoundMode(((self.0 >> 39) & 0x3) as i32)
    }
    pub fn ndv(&self) -> bool {
        (self.0 & 0x4000000000) != 0
    }
    pub fn pn_word(&self) -> i32 {
        (((self.0 >> 28) & 0xFF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstGetcrsptr(pub u64);
impl InstGetcrsptr {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstGetlmembase(pub u64);
impl InstGetlmembase {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstHadd2R(pub u64);
impl InstHadd2R {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn src_b(&self) -> i32 {
        (((self.0 >> 20) & 0xFF) as i32)
    }
    pub fn o_fmt(&self) -> OFmt {
        OFmt(((self.0 >> 49) & 0x3) as i32)
    }
    pub fn a_swizzle(&self) -> HalfSwizzle {
        HalfSwizzle(((self.0 >> 47) & 0x3) as i32)
    }
    pub fn b_swizzle(&self) -> HalfSwizzle {
        HalfSwizzle(((self.0 >> 28) & 0x3) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x80000000) != 0
    }
    pub fn abs_a(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x40000000) != 0
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x100000000) != 0
    }
    pub fn ftz(&self) -> bool {
        (self.0 & 0x8000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstHadd2I(pub u64);
impl InstHadd2I {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn bimm_h0(&self) -> i32 {
        (((self.0 >> 20) & 0x3FF) as i32)
    }
    pub fn bimm_h1(&self) -> i32 {
        (((self.0 >> 47) & 0x200) as i32) | (((self.0 >> 30) & 0x1FF) as i32)
    }
    pub fn o_fmt(&self) -> OFmt {
        OFmt(((self.0 >> 49) & 0x3) as i32)
    }
    pub fn a_swizzle(&self) -> HalfSwizzle {
        HalfSwizzle(((self.0 >> 47) & 0x3) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
    pub fn abs_a(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x10000000000000) != 0
    }
    pub fn ftz(&self) -> bool {
        (self.0 & 0x8000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstHadd2C(pub u64);
impl InstHadd2C {
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
    pub fn o_fmt(&self) -> OFmt {
        OFmt(((self.0 >> 49) & 0x3) as i32)
    }
    pub fn a_swizzle(&self) -> HalfSwizzle {
        HalfSwizzle(((self.0 >> 47) & 0x3) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x100000000000000) != 0
    }
    pub fn abs_a(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x40000000000000) != 0
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x10000000000000) != 0
    }
    pub fn ftz(&self) -> bool {
        (self.0 & 0x8000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstHadd232i(pub u64);
impl InstHadd232i {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn imm(&self) -> i32 {
        ((self.0 >> 20) as i32)
    }
    pub fn a_swizzle(&self) -> HalfSwizzle {
        HalfSwizzle(((self.0 >> 53) & 0x3) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x100000000000000) != 0
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x10000000000000) != 0
    }
    pub fn ftz(&self) -> bool {
        (self.0 & 0x80000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstHfma2R(pub u64);
impl InstHfma2R {
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
    pub fn o_fmt(&self) -> OFmt {
        OFmt(((self.0 >> 49) & 0x3) as i32)
    }
    pub fn a_swizzle(&self) -> HalfSwizzle {
        HalfSwizzle(((self.0 >> 47) & 0x3) as i32)
    }
    pub fn b_swizzle(&self) -> HalfSwizzle {
        HalfSwizzle(((self.0 >> 28) & 0x3) as i32)
    }
    pub fn c_swizzle(&self) -> HalfSwizzle {
        HalfSwizzle(((self.0 >> 35) & 0x3) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x80000000) != 0
    }
    pub fn neg_c(&self) -> bool {
        (self.0 & 0x40000000) != 0
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x100000000) != 0
    }
    pub fn fmz(&self) -> Fmz {
        Fmz(((self.0 >> 37) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstHfma2I(pub u64);
impl InstHfma2I {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn bimm_h0(&self) -> i32 {
        (((self.0 >> 20) & 0x3FF) as i32)
    }
    pub fn bimm_h1(&self) -> i32 {
        (((self.0 >> 47) & 0x200) as i32) | (((self.0 >> 30) & 0x1FF) as i32)
    }
    pub fn src_c(&self) -> i32 {
        (((self.0 >> 39) & 0xFF) as i32)
    }
    pub fn o_fmt(&self) -> OFmt {
        OFmt(((self.0 >> 49) & 0x3) as i32)
    }
    pub fn a_swizzle(&self) -> HalfSwizzle {
        HalfSwizzle(((self.0 >> 47) & 0x3) as i32)
    }
    pub fn c_swizzle(&self) -> HalfSwizzle {
        HalfSwizzle(((self.0 >> 53) & 0x3) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn neg_c(&self) -> bool {
        (self.0 & 0x8000000000000) != 0
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x10000000000000) != 0
    }
    pub fn fmz(&self) -> Fmz {
        Fmz(((self.0 >> 57) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstHfma2C(pub u64);
impl InstHfma2C {
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
    pub fn o_fmt(&self) -> OFmt {
        OFmt(((self.0 >> 49) & 0x3) as i32)
    }
    pub fn a_swizzle(&self) -> HalfSwizzle {
        HalfSwizzle(((self.0 >> 47) & 0x3) as i32)
    }
    pub fn c_swizzle(&self) -> HalfSwizzle {
        HalfSwizzle(((self.0 >> 53) & 0x3) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x100000000000000) != 0
    }
    pub fn neg_c(&self) -> bool {
        (self.0 & 0x8000000000000) != 0
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x10000000000000) != 0
    }
    pub fn fmz(&self) -> Fmz {
        Fmz(((self.0 >> 57) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstHfma2Rc(pub u64);
impl InstHfma2Rc {
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
    pub fn o_fmt(&self) -> OFmt {
        OFmt(((self.0 >> 49) & 0x3) as i32)
    }
    pub fn a_swizzle(&self) -> HalfSwizzle {
        HalfSwizzle(((self.0 >> 47) & 0x3) as i32)
    }
    pub fn c_swizzle(&self) -> HalfSwizzle {
        HalfSwizzle(((self.0 >> 53) & 0x3) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x100000000000000) != 0
    }
    pub fn neg_c(&self) -> bool {
        (self.0 & 0x8000000000000) != 0
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x10000000000000) != 0
    }
    pub fn fmz(&self) -> Fmz {
        Fmz(((self.0 >> 57) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstHfma232i(pub u64);
impl InstHfma232i {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn imm(&self) -> i32 {
        ((self.0 >> 20) as i32)
    }
    pub fn a_swizzle(&self) -> HalfSwizzle {
        HalfSwizzle(((self.0 >> 47) & 0x3) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn neg_c(&self) -> bool {
        (self.0 & 0x8000000000000) != 0
    }
    pub fn fmz(&self) -> Fmz {
        Fmz(((self.0 >> 57) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstHmul2R(pub u64);
impl InstHmul2R {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn src_b(&self) -> i32 {
        (((self.0 >> 20) & 0xFF) as i32)
    }
    pub fn o_fmt(&self) -> OFmt {
        OFmt(((self.0 >> 49) & 0x3) as i32)
    }
    pub fn a_swizzle(&self) -> HalfSwizzle {
        HalfSwizzle(((self.0 >> 47) & 0x3) as i32)
    }
    pub fn b_swizzle(&self) -> HalfSwizzle {
        HalfSwizzle(((self.0 >> 28) & 0x3) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x80000000) != 0
    }
    pub fn abs_a(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x40000000) != 0
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x100000000) != 0
    }
    pub fn fmz(&self) -> Fmz {
        Fmz(((self.0 >> 39) & 0x3) as i32)
    }
}

