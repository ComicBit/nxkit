//! Produced by tools/gen_xlate_decoders.py from InstDecoders.cs: do not edit.
#![allow(non_upper_case_globals, non_camel_case_types, dead_code, unused_parens, clippy::all)]

use crate::enums::*;

#[derive(Clone, Copy, Debug)]
pub struct InstTldB(pub u64);
impl InstTldB {
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
    pub fn w_mask(&self) -> i32 {
        (((self.0 >> 31) & 0xF) as i32)
    }
    pub fn lod(&self) -> bool {
        (self.0 & 0x80000000000000) != 0
    }
    pub fn toff(&self) -> bool {
        (self.0 & 0x800000000) != 0
    }
    pub fn ms(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn cl(&self) -> bool {
        (self.0 & 0x40000000000000) != 0
    }
    pub fn nodep(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 51) & 0x7) as i32)
    }
    pub fn dim(&self) -> TexDim {
        TexDim(((self.0 >> 28) & 0x7) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstTlds(pub u64);
impl InstTlds {
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
    pub fn tid_b(&self) -> i32 {
        (((self.0 >> 36) & 0x1FFF) as i32)
    }
    pub fn target(&self) -> TldsTarget {
        TldsTarget(((self.0 >> 53) & 0xF) as i32)
    }
    pub fn w_mask(&self) -> i32 {
        (((self.0 >> 50) & 0x7) as i32)
    }
    pub fn nodep(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn dest2(&self) -> i32 {
        (((self.0 >> 28) & 0xFF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstTld4(pub u64);
impl InstTld4 {
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
    pub fn lc(&self) -> bool {
        (self.0 & 0x400000000000000) != 0
    }
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 51) & 0x7) as i32)
    }
    pub fn tid_b(&self) -> i32 {
        (((self.0 >> 36) & 0x1FFF) as i32)
    }
    pub fn tex_comp(&self) -> TexComp {
        TexComp(((self.0 >> 56) & 0x3) as i32)
    }
    pub fn toff(&self) -> TexOffset {
        TexOffset(((self.0 >> 54) & 0x3) as i32)
    }
    pub fn dc(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn ndv(&self) -> bool {
        (self.0 & 0x800000000) != 0
    }
    pub fn dim(&self) -> TexDim {
        TexDim(((self.0 >> 28) & 0x7) as i32)
    }
    pub fn w_mask(&self) -> i32 {
        (((self.0 >> 31) & 0xF) as i32)
    }
    pub fn nodep(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstTld4B(pub u64);
impl InstTld4B {
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
    pub fn lc(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 51) & 0x7) as i32)
    }
    pub fn tex_comp(&self) -> TexComp {
        TexComp(((self.0 >> 38) & 0x3) as i32)
    }
    pub fn toff(&self) -> TexOffset {
        TexOffset(((self.0 >> 36) & 0x3) as i32)
    }
    pub fn dc(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn ndv(&self) -> bool {
        (self.0 & 0x800000000) != 0
    }
    pub fn dim(&self) -> TexDim {
        TexDim(((self.0 >> 28) & 0x7) as i32)
    }
    pub fn w_mask(&self) -> i32 {
        (((self.0 >> 31) & 0xF) as i32)
    }
    pub fn nodep(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstTld4s(pub u64);
impl InstTld4s {
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
    pub fn tid_b(&self) -> i32 {
        (((self.0 >> 36) & 0x1FFF) as i32)
    }
    pub fn tex_comp(&self) -> TexComp {
        TexComp(((self.0 >> 52) & 0x3) as i32)
    }
    pub fn aoffi(&self) -> bool {
        (self.0 & 0x8000000000000) != 0
    }
    pub fn dc(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn nodep(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn dest2(&self) -> i32 {
        (((self.0 >> 28) & 0xFF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstTmml(pub u64);
impl InstTmml {
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
    pub fn nodep(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn tid_b(&self) -> i32 {
        (((self.0 >> 36) & 0x1FFF) as i32)
    }
    pub fn ndv(&self) -> bool {
        (self.0 & 0x800000000) != 0
    }
    pub fn w_mask(&self) -> i32 {
        (((self.0 >> 31) & 0xF) as i32)
    }
    pub fn dim(&self) -> TexDim {
        TexDim(((self.0 >> 28) & 0x7) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstTmmlB(pub u64);
impl InstTmmlB {
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
    pub fn nodep(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn ndv(&self) -> bool {
        (self.0 & 0x800000000) != 0
    }
    pub fn w_mask(&self) -> i32 {
        (((self.0 >> 31) & 0xF) as i32)
    }
    pub fn dim(&self) -> TexDim {
        TexDim(((self.0 >> 28) & 0x7) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstTxa(pub u64);
impl InstTxa {
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
    pub fn nodep(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn tid_b(&self) -> i32 {
        (((self.0 >> 36) & 0x1FFF) as i32)
    }
    pub fn ndv(&self) -> bool {
        (self.0 & 0x800000000) != 0
    }
    pub fn w_mask(&self) -> i32 {
        (((self.0 >> 31) & 0xF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstTxd(pub u64);
impl InstTxd {
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
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 51) & 0x7) as i32)
    }
    pub fn lc(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn nodep(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn tid_b(&self) -> i32 {
        (((self.0 >> 36) & 0x1FFF) as i32)
    }
    pub fn toff(&self) -> bool {
        (self.0 & 0x800000000) != 0
    }
    pub fn w_mask(&self) -> i32 {
        (((self.0 >> 31) & 0xF) as i32)
    }
    pub fn dim(&self) -> TexDim {
        TexDim(((self.0 >> 28) & 0x7) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstTxdB(pub u64);
impl InstTxdB {
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
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 51) & 0x7) as i32)
    }
    pub fn lc(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn nodep(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn toff(&self) -> bool {
        (self.0 & 0x800000000) != 0
    }
    pub fn w_mask(&self) -> i32 {
        (((self.0 >> 31) & 0xF) as i32)
    }
    pub fn dim(&self) -> TexDim {
        TexDim(((self.0 >> 28) & 0x7) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstTxq(pub u64);
impl InstTxq {
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
    pub fn nodep(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn tid_b(&self) -> i32 {
        (((self.0 >> 36) & 0x1FFF) as i32)
    }
    pub fn w_mask(&self) -> i32 {
        (((self.0 >> 31) & 0xF) as i32)
    }
    pub fn tex_query(&self) -> TexQuery {
        TexQuery(((self.0 >> 22) & 0x3F) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstTxqB(pub u64);
impl InstTxqB {
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
    pub fn nodep(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn w_mask(&self) -> i32 {
        (((self.0 >> 31) & 0xF) as i32)
    }
    pub fn tex_query(&self) -> TexQuery {
        TexQuery(((self.0 >> 22) & 0x3F) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstVabsdiff(pub u64);
impl InstVabsdiff {
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
pub struct InstVabsdiff4(pub u64);
impl InstVabsdiff4 {
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
    pub fn v_red(&self) -> VideoRed {
        VideoRed(((self.0 >> 53) & 0x3) as i32)
    }
    pub fn lane_mask4(&self) -> LaneMask4 {
        LaneMask4((((self.0 >> 49) & 0xC) as i32) | (((self.0 >> 36) & 0x3) as i32))
    }
    pub fn sat(&self) -> bool {
        (self.0 & 0x4000000000000) != 0
    }
    pub fn src_b_fmt(&self) -> bool {
        (self.0 & 0x2000000000000) != 0
    }
    pub fn src_a_fmt(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
    pub fn d_format(&self) -> bool {
        (self.0 & 0x4000000000) != 0
    }
    pub fn asel4(&self) -> ASelect4 {
        ASelect4(((self.0 >> 32) & 0xF) as i32)
    }
    pub fn bsel4(&self) -> BSelect4 {
        BSelect4(((self.0 >> 28) & 0xF) as i32)
    }
}

