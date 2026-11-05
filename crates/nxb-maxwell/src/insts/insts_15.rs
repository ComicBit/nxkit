//! Produced by tools/gen_xlate_decoders.py from InstDecoders.cs: do not edit.
#![allow(non_upper_case_globals, non_camel_case_types, dead_code, unused_parens, clippy::all)]

use crate::enums::*;

#[derive(Clone, Copy, Debug)]
pub struct InstShlC(pub u64);
impl InstShlC {
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
    pub fn x(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
    pub fn m(&self) -> bool {
        (self.0 & 0x8000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstShrR(pub u64);
impl InstShrR {
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
    pub fn brev(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
    pub fn m(&self) -> bool {
        (self.0 & 0x8000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstShrI(pub u64);
impl InstShrI {
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
    pub fn brev(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
    pub fn m(&self) -> bool {
        (self.0 & 0x8000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstShrC(pub u64);
impl InstShrC {
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
    pub fn brev(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
    pub fn m(&self) -> bool {
        (self.0 & 0x8000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstSsy(pub u64);
impl InstSsy {
    pub fn imm24(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFFFF) as i32)
    }
    pub fn ca(&self) -> bool {
        (self.0 & 0x20) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstSt(pub u64);
impl InstSt {
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
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 58) & 0x7) as i32)
    }
    pub fn cache_op(&self) -> CacheOpSt {
        CacheOpSt(((self.0 >> 56) & 0x3) as i32)
    }
    pub fn ls_size(&self) -> LsSize {
        LsSize(((self.0 >> 53) & 0x7) as i32)
    }
    pub fn e(&self) -> bool {
        (self.0 & 0x10000000000000) != 0
    }
    pub fn imm32(&self) -> i32 {
        ((self.0 >> 20) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstStg(pub u64);
impl InstStg {
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
    pub fn ls_size(&self) -> LsSize2 {
        LsSize2(((self.0 >> 48) & 0x7) as i32)
    }
    pub fn cache_op(&self) -> CacheOpSt {
        CacheOpSt(((self.0 >> 46) & 0x3) as i32)
    }
    pub fn e(&self) -> bool {
        (self.0 & 0x200000000000) != 0
    }
    pub fn imm24(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFFFF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstStl(pub u64);
impl InstStl {
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
    pub fn ls_size(&self) -> LsSize2 {
        LsSize2(((self.0 >> 48) & 0x7) as i32)
    }
    pub fn cache_op(&self) -> CacheOpSt {
        CacheOpSt(((self.0 >> 44) & 0x3) as i32)
    }
    pub fn imm24(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFFFF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstStp(pub u64);
impl InstStp {
    pub fn wait(&self) -> bool {
        (self.0 & 0x80000000) != 0
    }
    pub fn imm8(&self) -> i32 {
        (((self.0 >> 20) & 0xFF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstSts(pub u64);
impl InstSts {
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
    pub fn ls_size(&self) -> LsSize2 {
        LsSize2(((self.0 >> 48) & 0x7) as i32)
    }
    pub fn imm24(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFFFF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstSuatomB(pub u64);
impl InstSuatomB {
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
    pub fn clamp(&self) -> Clamp {
        Clamp(((self.0 >> 49) & 0x3) as i32)
    }
    pub fn size(&self) -> SuatomSize {
        SuatomSize(((self.0 >> 36) & 0x7) as i32)
    }
    pub fn dim(&self) -> SuDim {
        SuDim(((self.0 >> 33) & 0x7) as i32)
    }
    pub fn op(&self) -> SuatomOp {
        SuatomOp(((self.0 >> 29) & 0xF) as i32)
    }
    pub fn ba(&self) -> bool {
        (self.0 & 0x10000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstSuatom(pub u64);
impl InstSuatom {
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
    pub fn size(&self) -> SuatomSize {
        SuatomSize(((self.0 >> 51) & 0x7) as i32)
    }
    pub fn clamp(&self) -> Clamp {
        Clamp(((self.0 >> 49) & 0x3) as i32)
    }
    pub fn tid_b(&self) -> i32 {
        (((self.0 >> 36) & 0x1FFF) as i32)
    }
    pub fn dim(&self) -> SuDim {
        SuDim(((self.0 >> 33) & 0x7) as i32)
    }
    pub fn op(&self) -> SuatomOp {
        SuatomOp(((self.0 >> 29) & 0xF) as i32)
    }
    pub fn ba(&self) -> bool {
        (self.0 & 0x10000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstSuatomB2(pub u64);
impl InstSuatomB2 {
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
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 51) & 0x7) as i32)
    }
    pub fn clamp(&self) -> Clamp {
        Clamp(((self.0 >> 49) & 0x3) as i32)
    }
    pub fn size(&self) -> SuatomSize {
        SuatomSize(((self.0 >> 36) & 0x7) as i32)
    }
    pub fn dim(&self) -> SuDim {
        SuDim(((self.0 >> 33) & 0x7) as i32)
    }
    pub fn op(&self) -> SuatomOp {
        SuatomOp(((self.0 >> 29) & 0xF) as i32)
    }
    pub fn ba(&self) -> bool {
        (self.0 & 0x10000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstSuatomCasB(pub u64);
impl InstSuatomCasB {
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
    pub fn clamp(&self) -> Clamp {
        Clamp(((self.0 >> 49) & 0x3) as i32)
    }
    pub fn size(&self) -> SuatomSize {
        SuatomSize(((self.0 >> 36) & 0x7) as i32)
    }
    pub fn dim(&self) -> SuDim {
        SuDim(((self.0 >> 33) & 0x7) as i32)
    }
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 30) & 0x7) as i32)
    }
    pub fn ba(&self) -> bool {
        (self.0 & 0x10000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstSuatomCas(pub u64);
impl InstSuatomCas {
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
    pub fn size(&self) -> SuatomSize {
        SuatomSize(((self.0 >> 51) & 0x7) as i32)
    }
    pub fn clamp(&self) -> Clamp {
        Clamp(((self.0 >> 49) & 0x3) as i32)
    }
    pub fn tid_b(&self) -> i32 {
        (((self.0 >> 36) & 0x1FFF) as i32)
    }
    pub fn dim(&self) -> SuDim {
        SuDim(((self.0 >> 33) & 0x7) as i32)
    }
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 30) & 0x7) as i32)
    }
    pub fn ba(&self) -> bool {
        (self.0 & 0x10000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstSuldDB(pub u64);
impl InstSuldDB {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
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
    pub fn clamp(&self) -> Clamp {
        Clamp(((self.0 >> 49) & 0x3) as i32)
    }
    pub fn dim(&self) -> SuDim {
        SuDim(((self.0 >> 33) & 0x7) as i32)
    }
    pub fn dest_pred2(&self) -> i32 {
        (((self.0 >> 30) & 0x7) as i32)
    }
    pub fn cache_op(&self) -> CacheOpLd {
        CacheOpLd(((self.0 >> 24) & 0x3) as i32)
    }
    pub fn ba(&self) -> bool {
        (self.0 & 0x800000) != 0
    }
    pub fn size(&self) -> SuSize {
        SuSize(((self.0 >> 20) & 0x7) as i32)
    }
}

