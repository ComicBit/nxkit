//! Produced by tools/gen_xlate_decoders.py from InstDecoders.cs: do not edit.
#![allow(non_upper_case_globals, non_camel_case_types, dead_code, unused_parens, clippy::all)]

use crate::enums::*;

#[derive(Clone, Copy, Debug)]
pub struct InstOutC(pub u64);
impl InstOutC {
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
    pub fn out_type(&self) -> OutType {
        OutType(((self.0 >> 39) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstP2rR(pub u64);
impl InstP2rR {
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
    pub fn byte_sel(&self) -> ByteSel {
        ByteSel(((self.0 >> 41) & 0x3) as i32)
    }
    pub fn ccpr(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstP2rI(pub u64);
impl InstP2rI {
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
    pub fn byte_sel(&self) -> ByteSel {
        ByteSel(((self.0 >> 41) & 0x3) as i32)
    }
    pub fn ccpr(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstP2rC(pub u64);
impl InstP2rC {
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
    pub fn byte_sel(&self) -> ByteSel {
        ByteSel(((self.0 >> 41) & 0x3) as i32)
    }
    pub fn ccpr(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstPbk(pub u64);
impl InstPbk {
    pub fn imm24(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFFFF) as i32)
    }
    pub fn ca(&self) -> bool {
        (self.0 & 0x20) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstPcnt(pub u64);
impl InstPcnt {
    pub fn imm24(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFFFF) as i32)
    }
    pub fn ca(&self) -> bool {
        (self.0 & 0x20) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstPexit(pub u64);
impl InstPexit {
    pub fn imm24(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFFFF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstPixld(pub u64);
impl InstPixld {
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
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 45) & 0x7) as i32)
    }
    pub fn pix_mode(&self) -> PixMode {
        PixMode(((self.0 >> 31) & 0x7) as i32)
    }
    pub fn imm8(&self) -> i32 {
        (((self.0 >> 20) & 0xFF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstPlongjmp(pub u64);
impl InstPlongjmp {
    pub fn imm24(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFFFF) as i32)
    }
    pub fn ca(&self) -> bool {
        (self.0 & 0x20) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstPopcR(pub u64);
impl InstPopcR {
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
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstPopcI(pub u64);
impl InstPopcI {
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
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstPopcC(pub u64);
impl InstPopcC {
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
    pub fn neg_b(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstPret(pub u64);
impl InstPret {
    pub fn ca(&self) -> bool {
        (self.0 & 0x20) != 0
    }
    pub fn imm24(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFFFF) as i32)
    }
    pub fn inc(&self) -> bool {
        (self.0 & 0x40) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstPrmtR(pub u64);
impl InstPrmtR {
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
    pub fn p_mode(&self) -> PMode {
        PMode(((self.0 >> 48) & 0xF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstPrmtI(pub u64);
impl InstPrmtI {
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
    pub fn p_mode(&self) -> PMode {
        PMode(((self.0 >> 48) & 0xF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstPrmtC(pub u64);
impl InstPrmtC {
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
    pub fn p_mode(&self) -> PMode {
        PMode(((self.0 >> 48) & 0xF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstPrmtRc(pub u64);
impl InstPrmtRc {
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
    pub fn p_mode(&self) -> PMode {
        PMode(((self.0 >> 48) & 0xF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstPset(pub u64);
impl InstPset {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
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
    pub fn src2_pred(&self) -> i32 {
        (((self.0 >> 12) & 0x7) as i32)
    }
    pub fn src2_pred_inv(&self) -> bool {
        (self.0 & 0x8000) != 0
    }
    pub fn src1_pred(&self) -> i32 {
        (((self.0 >> 29) & 0x7) as i32)
    }
    pub fn src1_pred_inv(&self) -> bool {
        (self.0 & 0x100000000) != 0
    }
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
    pub fn bool_op_ab(&self) -> BoolOp {
        BoolOp(((self.0 >> 24) & 0x3) as i32)
    }
    pub fn bool_op_c(&self) -> BoolOp {
        BoolOp(((self.0 >> 45) & 0x3) as i32)
    }
    pub fn b_val(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstPsetp(pub u64);
impl InstPsetp {
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 3) & 0x7) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn dest_pred_inv(&self) -> i32 {
        ((self.0 & 0x7) as i32)
    }
    pub fn src2_pred(&self) -> i32 {
        (((self.0 >> 12) & 0x7) as i32)
    }
    pub fn src2_pred_inv(&self) -> bool {
        (self.0 & 0x8000) != 0
    }
    pub fn src1_pred(&self) -> i32 {
        (((self.0 >> 29) & 0x7) as i32)
    }
    pub fn src1_pred_inv(&self) -> bool {
        (self.0 & 0x100000000) != 0
    }
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
    pub fn bool_op_ab(&self) -> BoolOp {
        BoolOp(((self.0 >> 24) & 0x3) as i32)
    }
    pub fn bool_op_c(&self) -> BoolOp {
        BoolOp(((self.0 >> 45) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstR2b(pub u64);
impl InstR2b {
    pub fn src_b(&self) -> i32 {
        (((self.0 >> 20) & 0xFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn mode(&self) -> BarMode {
        BarMode(((self.0 >> 32) & 0x3) as i32)
    }
    pub fn name(&self) -> i32 {
        (((self.0 >> 28) & 0xF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstR2pR(pub u64);
impl InstR2pR {
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
    pub fn byte_sel(&self) -> ByteSel {
        ByteSel(((self.0 >> 41) & 0x3) as i32)
    }
    pub fn ccpr(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstR2pI(pub u64);
impl InstR2pI {
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
    pub fn byte_sel(&self) -> ByteSel {
        ByteSel(((self.0 >> 41) & 0x3) as i32)
    }
    pub fn ccpr(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
}

