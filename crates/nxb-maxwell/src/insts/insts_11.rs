//! Produced by tools/gen_xlate_decoders.py from InstDecoders.cs: do not edit.
#![allow(non_upper_case_globals, non_camel_case_types, dead_code, unused_parens, clippy::all)]

use crate::enums::*;

#[derive(Clone, Copy, Debug)]
pub struct InstIsetpR(pub u64);
impl InstIsetpR {
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
    pub fn i_comp(&self) -> IComp {
        IComp(((self.0 >> 49) & 0x7) as i32)
    }
    pub fn signed(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn bop(&self) -> BoolOp {
        BoolOp(((self.0 >> 45) & 0x3) as i32)
    }
    pub fn x(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 3) & 0x7) as i32)
    }
    pub fn dest_pred_inv(&self) -> i32 {
        ((self.0 & 0x7) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstIsetpI(pub u64);
impl InstIsetpI {
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
    pub fn i_comp(&self) -> IComp {
        IComp(((self.0 >> 49) & 0x7) as i32)
    }
    pub fn signed(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn bop(&self) -> BoolOp {
        BoolOp(((self.0 >> 45) & 0x3) as i32)
    }
    pub fn x(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 3) & 0x7) as i32)
    }
    pub fn dest_pred_inv(&self) -> i32 {
        ((self.0 & 0x7) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstIsetpC(pub u64);
impl InstIsetpC {
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
    pub fn i_comp(&self) -> IComp {
        IComp(((self.0 >> 49) & 0x7) as i32)
    }
    pub fn signed(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn bop(&self) -> BoolOp {
        BoolOp(((self.0 >> 45) & 0x3) as i32)
    }
    pub fn x(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 3) & 0x7) as i32)
    }
    pub fn dest_pred_inv(&self) -> i32 {
        ((self.0 & 0x7) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstJcal(pub u64);
impl InstJcal {
    pub fn imm32(&self) -> i32 {
        ((self.0 >> 20) as i32)
    }
    pub fn ca(&self) -> bool {
        (self.0 & 0x20) != 0
    }
    pub fn inc(&self) -> bool {
        (self.0 & 0x40) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstJmp(pub u64);
impl InstJmp {
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn ccc(&self) -> Ccc {
        Ccc((self.0 & 0x1F) as i32)
    }
    pub fn ca(&self) -> bool {
        (self.0 & 0x20) != 0
    }
    pub fn imm32(&self) -> i32 {
        ((self.0 >> 20) as i32)
    }
    pub fn lmt(&self) -> bool {
        (self.0 & 0x40) != 0
    }
    pub fn u(&self) -> bool {
        (self.0 & 0x80) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstJmx(pub u64);
impl InstJmx {
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn ccc(&self) -> Ccc {
        Ccc((self.0 & 0x1F) as i32)
    }
    pub fn ca(&self) -> bool {
        (self.0 & 0x20) != 0
    }
    pub fn imm32(&self) -> i32 {
        ((self.0 >> 20) as i32)
    }
    pub fn lmt(&self) -> bool {
        (self.0 & 0x40) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstKil(pub u64);
impl InstKil {
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn ccc(&self) -> Ccc {
        Ccc((self.0 & 0x1F) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstLd(pub u64);
impl InstLd {
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
    pub fn cache_op(&self) -> CacheOpLd {
        CacheOpLd(((self.0 >> 56) & 0x3) as i32)
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
pub struct InstLdc(pub u64);
impl InstLdc {
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
    pub fn address_mode(&self) -> AddressMode {
        AddressMode(((self.0 >> 44) & 0x3) as i32)
    }
    pub fn cbuf_slot(&self) -> i32 {
        (((self.0 >> 36) & 0x1F) as i32)
    }
    pub fn cbuf_offset(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstLdg(pub u64);
impl InstLdg {
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
    pub fn ls_size(&self) -> LsSize {
        LsSize(((self.0 >> 48) & 0x7) as i32)
    }
    pub fn cache_op(&self) -> CacheOpLd {
        CacheOpLd(((self.0 >> 46) & 0x3) as i32)
    }
    pub fn e(&self) -> bool {
        (self.0 & 0x200000000000) != 0
    }
    pub fn imm24(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFFFF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstLdl(pub u64);
impl InstLdl {
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
    pub fn cache_op(&self) -> CacheOp2 {
        CacheOp2(((self.0 >> 44) & 0x3) as i32)
    }
    pub fn imm24(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFFFF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstLds(pub u64);
impl InstLds {
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
    pub fn u(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn imm24(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFFFF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstLeaR(pub u64);
impl InstLeaR {
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
    pub fn x(&self) -> bool {
        (self.0 & 0x400000000000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x200000000000) != 0
    }
    pub fn imm_u5(&self) -> i32 {
        (((self.0 >> 39) & 0x1F) as i32)
    }
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 48) & 0x7) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstLeaI(pub u64);
impl InstLeaI {
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
    pub fn x(&self) -> bool {
        (self.0 & 0x400000000000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x200000000000) != 0
    }
    pub fn imm_u5(&self) -> i32 {
        (((self.0 >> 39) & 0x1F) as i32)
    }
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 48) & 0x7) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstLeaC(pub u64);
impl InstLeaC {
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
        (self.0 & 0x400000000000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x200000000000) != 0
    }
    pub fn imm_u5(&self) -> i32 {
        (((self.0 >> 39) & 0x1F) as i32)
    }
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 48) & 0x7) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstLeaHiR(pub u64);
impl InstLeaHiR {
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
    pub fn x(&self) -> bool {
        (self.0 & 0x4000000000) != 0
    }
    pub fn neg_a(&self) -> bool {
        (self.0 & 0x2000000000) != 0
    }
    pub fn imm_u5(&self) -> i32 {
        (((self.0 >> 28) & 0x1F) as i32)
    }
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 48) & 0x7) as i32)
    }
}

