//! Produced by tools/gen_xlate_decoders.py from InstDecoders.cs: do not edit.
#![allow(non_upper_case_globals, non_camel_case_types, dead_code, unused_parens, clippy::all)]

use crate::enums::*;

#[derive(Clone, Copy, Debug)]
pub struct InstVadd(pub u64);
impl InstVadd {
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
    pub fn imm16(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFF) as i32)
    }
    pub fn write_cc(&self) -> bool {
        (self.0 & 0x800000000000) != 0
    }
    pub fn avg_mode(&self) -> AvgMode {
        AvgMode(((self.0 >> 56) & 0x3) as i32)
    }
    pub fn d_format(&self) -> bool {
        (self.0 & 0x40000000000000) != 0
    }
    pub fn a_select(&self) -> VectorSelect {
        VectorSelect((((self.0 >> 45) & 0x8) as i32) | (((self.0 >> 36) & 0x7) as i32))
    }
    pub fn b_select(&self) -> VectorSelect {
        VectorSelect((((self.0 >> 46) & 0x8) as i32) | (((self.0 >> 28) & 0x7) as i32))
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x80000000000000) != 0
    }
    pub fn video_op(&self) -> VideoOp {
        VideoOp(((self.0 >> 51) & 0x7) as i32)
    }
    pub fn b_video(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstVmad(pub u64);
impl InstVmad {
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
    pub fn imm16(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFF) as i32)
    }
    pub fn write_cc(&self) -> bool {
        (self.0 & 0x800000000000) != 0
    }
    pub fn a_select(&self) -> VectorSelect {
        VectorSelect((((self.0 >> 45) & 0x8) as i32) | (((self.0 >> 36) & 0x7) as i32))
    }
    pub fn b_select(&self) -> VectorSelect {
        VectorSelect((((self.0 >> 46) & 0x8) as i32) | (((self.0 >> 28) & 0x7) as i32))
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x80000000000000) != 0
    }
    pub fn avg_mode(&self) -> AvgMode {
        AvgMode(((self.0 >> 53) & 0x3) as i32)
    }
    pub fn video_scale(&self) -> VideoScale {
        VideoScale(((self.0 >> 51) & 0x3) as i32)
    }
    pub fn b_video(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstVmnmx(pub u64);
impl InstVmnmx {
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
    pub fn imm16(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFF) as i32)
    }
    pub fn write_cc(&self) -> bool {
        (self.0 & 0x800000000000) != 0
    }
    pub fn d_format(&self) -> bool {
        (self.0 & 0x40000000000000) != 0
    }
    pub fn a_select(&self) -> VectorSelect {
        VectorSelect((((self.0 >> 45) & 0x8) as i32) | (((self.0 >> 36) & 0x7) as i32))
    }
    pub fn b_select(&self) -> VectorSelect {
        VectorSelect((((self.0 >> 46) & 0x8) as i32) | (((self.0 >> 28) & 0x7) as i32))
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x80000000000000) != 0
    }
    pub fn video_op(&self) -> VideoOp {
        VideoOp(((self.0 >> 51) & 0x7) as i32)
    }
    pub fn mn(&self) -> bool {
        (self.0 & 0x100000000000000) != 0
    }
    pub fn b_video(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstVote(pub u64);
impl InstVote {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
    pub fn vote_mode(&self) -> VoteMode {
        VoteMode(((self.0 >> 48) & 0x3) as i32)
    }
    pub fn vp_dest(&self) -> i32 {
        (((self.0 >> 45) & 0x7) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstVotevtg(pub u64);
impl InstVotevtg {
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn vote_mode(&self) -> VoteMode {
        VoteMode(((self.0 >> 48) & 0x3) as i32)
    }
    pub fn imm28(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFFFFF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstVset(pub u64);
impl InstVset {
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
    pub fn v_comp(&self) -> IComp {
        IComp(((self.0 >> 54) & 0x7) as i32)
    }
    pub fn a_select(&self) -> VectorSelect {
        VectorSelect((((self.0 >> 45) & 0x8) as i32) | (((self.0 >> 36) & 0x7) as i32))
    }
    pub fn b_select(&self) -> VectorSelect {
        VectorSelect((((self.0 >> 46) & 0x8) as i32) | (((self.0 >> 28) & 0x7) as i32))
    }
    pub fn video_op(&self) -> VideoOp {
        VideoOp(((self.0 >> 51) & 0x7) as i32)
    }
    pub fn b_video(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstVsetp(pub u64);
impl InstVsetp {
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
    pub fn imm16(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFF) as i32)
    }
    pub fn a_select(&self) -> VectorSelect {
        VectorSelect((((self.0 >> 45) & 0x8) as i32) | (((self.0 >> 36) & 0x7) as i32))
    }
    pub fn b_select(&self) -> VectorSelect {
        VectorSelect((((self.0 >> 46) & 0x8) as i32) | (((self.0 >> 28) & 0x7) as i32))
    }
    pub fn v_comp(&self) -> IComp {
        IComp((((self.0 >> 45) & 0x4) as i32) | (((self.0 >> 43) & 0x3) as i32))
    }
    pub fn bool_op(&self) -> BoolOp {
        BoolOp(((self.0 >> 45) & 0x3) as i32)
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
    pub fn b_video(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstVshl(pub u64);
impl InstVshl {
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
    pub fn mv(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn d_format(&self) -> bool {
        (self.0 & 0x40000000000000) != 0
    }
    pub fn a_select(&self) -> VectorSelect {
        VectorSelect((((self.0 >> 45) & 0x8) as i32) | (((self.0 >> 36) & 0x7) as i32))
    }
    pub fn b_select(&self) -> VectorSelect {
        VectorSelect(((self.0 >> 28) & 0x7) as i32)
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x80000000000000) != 0
    }
    pub fn video_op(&self) -> VideoOp {
        VideoOp(((self.0 >> 51) & 0x7) as i32)
    }
    pub fn b_video(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstVshr(pub u64);
impl InstVshr {
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
    pub fn mv(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn d_format(&self) -> bool {
        (self.0 & 0x40000000000000) != 0
    }
    pub fn a_select(&self) -> VectorSelect {
        VectorSelect((((self.0 >> 45) & 0x8) as i32) | (((self.0 >> 36) & 0x7) as i32))
    }
    pub fn b_select(&self) -> VectorSelect {
        VectorSelect(((self.0 >> 28) & 0x7) as i32)
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x80000000000000) != 0
    }
    pub fn video_op(&self) -> VideoOp {
        VideoOp(((self.0 >> 51) & 0x7) as i32)
    }
    pub fn b_video(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstXmadR(pub u64);
impl InstXmadR {
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
    pub fn hilo_a(&self) -> bool {
        (self.0 & 0x20000000000000) != 0
    }
    pub fn xmad_cop(&self) -> XmadCop {
        XmadCop(((self.0 >> 50) & 0x7) as i32)
    }
    pub fn b_signed(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn a_signed(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn x(&self) -> bool {
        (self.0 & 0x4000000000) != 0
    }
    pub fn mrg(&self) -> bool {
        (self.0 & 0x2000000000) != 0
    }
    pub fn psl(&self) -> bool {
        (self.0 & 0x1000000000) != 0
    }
    pub fn hilo_b(&self) -> bool {
        (self.0 & 0x800000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstXmadI(pub u64);
impl InstXmadI {
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
    pub fn write_cc(&self) -> bool {
        (self.0 & 0x800000000000) != 0
    }
    pub fn imm16(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFF) as i32)
    }
    pub fn hilo_a(&self) -> bool {
        (self.0 & 0x20000000000000) != 0
    }
    pub fn xmad_cop(&self) -> XmadCop {
        XmadCop(((self.0 >> 50) & 0x7) as i32)
    }
    pub fn b_signed(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn a_signed(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn x(&self) -> bool {
        (self.0 & 0x4000000000) != 0
    }
    pub fn mrg(&self) -> bool {
        (self.0 & 0x2000000000) != 0
    }
    pub fn psl(&self) -> bool {
        (self.0 & 0x1000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstXmadC(pub u64);
impl InstXmadC {
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
    pub fn mrg(&self) -> bool {
        (self.0 & 0x100000000000000) != 0
    }
    pub fn psl(&self) -> bool {
        (self.0 & 0x80000000000000) != 0
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

