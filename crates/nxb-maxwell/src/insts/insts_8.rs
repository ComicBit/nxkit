//! Produced by tools/gen_xlate_decoders.py from InstDecoders.cs: do not edit.
#![allow(non_upper_case_globals, non_camel_case_types, dead_code, unused_parens, clippy::all)]

use crate::enums::*;

#[derive(Clone, Copy, Debug)]
pub struct InstI2fC(pub u64);
impl InstI2fC {
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
    pub fn byte_sel(&self) -> ByteSel {
        ByteSel(((self.0 >> 41) & 0x3) as i32)
    }
    pub fn round_mode(&self) -> RoundMode {
        RoundMode(((self.0 >> 39) & 0x3) as i32)
    }
    pub fn i_src_fmt(&self) -> ISrcFmt {
        ISrcFmt((((self.0 >> 11) & 0x4) as i32) | (((self.0 >> 10) & 0x3) as i32))
    }
    pub fn dst_fmt(&self) -> DstFmt {
        DstFmt(((self.0 >> 8) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstI2iR(pub u64);
impl InstI2iR {
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
    pub fn sat(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x200000000000) != 0
    }
    pub fn byte_sel(&self) -> ByteSel {
        ByteSel(((self.0 >> 41) & 0x3) as i32)
    }
    pub fn i_dst_fmt(&self) -> ISrcDstFmt {
        ISrcDstFmt((((self.0 >> 10) & 0x4) as i32) | (((self.0 >> 8) & 0x3) as i32))
    }
    pub fn i_src_fmt(&self) -> ISrcDstFmt {
        ISrcDstFmt((((self.0 >> 11) & 0x4) as i32) | (((self.0 >> 10) & 0x3) as i32))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstI2iI(pub u64);
impl InstI2iI {
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
    pub fn sat(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x200000000000) != 0
    }
    pub fn byte_sel(&self) -> ByteSel {
        ByteSel(((self.0 >> 41) & 0x3) as i32)
    }
    pub fn i_dst_fmt(&self) -> ISrcDstFmt {
        ISrcDstFmt((((self.0 >> 10) & 0x4) as i32) | (((self.0 >> 8) & 0x3) as i32))
    }
    pub fn i_src_fmt(&self) -> ISrcDstFmt {
        ISrcDstFmt((((self.0 >> 11) & 0x4) as i32) | (((self.0 >> 10) & 0x3) as i32))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstI2iC(pub u64);
impl InstI2iC {
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
    pub fn sat(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn abs_b(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x200000000000) != 0
    }
    pub fn byte_sel(&self) -> ByteSel {
        ByteSel(((self.0 >> 41) & 0x3) as i32)
    }
    pub fn i_dst_fmt(&self) -> ISrcDstFmt {
        ISrcDstFmt((((self.0 >> 10) & 0x4) as i32) | (((self.0 >> 8) & 0x3) as i32))
    }
    pub fn i_src_fmt(&self) -> ISrcDstFmt {
        ISrcDstFmt((((self.0 >> 11) & 0x4) as i32) | (((self.0 >> 10) & 0x3) as i32))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstIaddR(pub u64);
impl InstIaddR {
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
    pub fn avg_mode(&self) -> AvgMode {
        AvgMode(((self.0 >> 48) & 0x3) as i32)
    }
    pub fn x(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstIaddI(pub u64);
impl InstIaddI {
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
    pub fn avg_mode(&self) -> AvgMode {
        AvgMode(((self.0 >> 48) & 0x3) as i32)
    }
    pub fn x(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstIaddC(pub u64);
impl InstIaddC {
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
    pub fn avg_mode(&self) -> AvgMode {
        AvgMode(((self.0 >> 48) & 0x3) as i32)
    }
    pub fn x(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstIadd32i(pub u64);
impl InstIadd32i {
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
    pub fn avg_mode(&self) -> AvgMode {
        AvgMode(((self.0 >> 55) & 0x3) as i32)
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x40000000000000) != 0
    }
    pub fn write_cc(&self) -> bool {
        (self.0 & 0x10000000000000) != 0
    }
    pub fn x(&self) -> bool {
        (self.0 & 0x20000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstIadd3R(pub u64);
impl InstIadd3R {
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
        (self.0 & 0x8000000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn neg_c(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn x(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn lrs(&self) -> Lrs {
        Lrs(((self.0 >> 37) & 0x3) as i32)
    }
    pub fn apart(&self) -> HalfSelect {
        HalfSelect(((self.0 >> 35) & 0x3) as i32)
    }
    pub fn bpart(&self) -> HalfSelect {
        HalfSelect(((self.0 >> 33) & 0x3) as i32)
    }
    pub fn cpart(&self) -> HalfSelect {
        HalfSelect(((self.0 >> 31) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstIadd3I(pub u64);
impl InstIadd3I {
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
        (self.0 & 0x8000000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn neg_c(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn x(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstIadd3C(pub u64);
impl InstIadd3C {
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
        (self.0 & 0x8000000000000) != 0
    }
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn neg_c(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn x(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstIcmpR(pub u64);
impl InstIcmpR {
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
    pub fn i_comp(&self) -> IComp {
        IComp(((self.0 >> 49) & 0x7) as i32)
    }
    pub fn signed(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstIcmpI(pub u64);
impl InstIcmpI {
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
    pub fn i_comp(&self) -> IComp {
        IComp(((self.0 >> 49) & 0x7) as i32)
    }
    pub fn signed(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstIcmpC(pub u64);
impl InstIcmpC {
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
    pub fn i_comp(&self) -> IComp {
        IComp(((self.0 >> 49) & 0x7) as i32)
    }
    pub fn signed(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
}

