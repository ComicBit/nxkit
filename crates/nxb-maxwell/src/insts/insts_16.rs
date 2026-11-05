//! Produced by tools/gen_xlate_decoders.py from InstDecoders.cs: do not edit.
#![allow(non_upper_case_globals, non_camel_case_types, dead_code, unused_parens, clippy::all)]

use crate::enums::*;

#[derive(Clone, Copy, Debug)]
pub struct InstSuldD(pub u64);
impl InstSuldD {
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
    pub fn clamp(&self) -> Clamp {
        Clamp(((self.0 >> 49) & 0x3) as i32)
    }
    pub fn tid_b(&self) -> i32 {
        (((self.0 >> 36) & 0x1FFF) as i32)
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

#[derive(Clone, Copy, Debug)]
pub struct InstSuldB(pub u64);
impl InstSuldB {
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
    pub fn rgba(&self) -> SuRgba {
        SuRgba(((self.0 >> 20) & 0xF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstSuld(pub u64);
impl InstSuld {
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
    pub fn clamp(&self) -> Clamp {
        Clamp(((self.0 >> 49) & 0x3) as i32)
    }
    pub fn tid_b(&self) -> i32 {
        (((self.0 >> 36) & 0x1FFF) as i32)
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
    pub fn rgba(&self) -> SuRgba {
        SuRgba(((self.0 >> 20) & 0xF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstSuredB(pub u64);
impl InstSuredB {
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
    pub fn op(&self) -> RedOp {
        RedOp(((self.0 >> 24) & 0x7) as i32)
    }
    pub fn ba(&self) -> bool {
        (self.0 & 0x800000) != 0
    }
    pub fn size(&self) -> SuatomSize {
        SuatomSize(((self.0 >> 20) & 0x7) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstSured(pub u64);
impl InstSured {
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
    pub fn clamp(&self) -> Clamp {
        Clamp(((self.0 >> 49) & 0x3) as i32)
    }
    pub fn tid_b(&self) -> i32 {
        (((self.0 >> 36) & 0x1FFF) as i32)
    }
    pub fn dim(&self) -> SuDim {
        SuDim(((self.0 >> 33) & 0x7) as i32)
    }
    pub fn op(&self) -> RedOp {
        RedOp(((self.0 >> 24) & 0x7) as i32)
    }
    pub fn ba(&self) -> bool {
        (self.0 & 0x800000) != 0
    }
    pub fn size(&self) -> SuatomSize {
        SuatomSize(((self.0 >> 20) & 0x7) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstSustDB(pub u64);
impl InstSustDB {
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
    pub fn cache_op(&self) -> CacheOpSt {
        CacheOpSt(((self.0 >> 24) & 0x3) as i32)
    }
    pub fn ba(&self) -> bool {
        (self.0 & 0x800000) != 0
    }
    pub fn size(&self) -> SuSize {
        SuSize(((self.0 >> 20) & 0x7) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstSustD(pub u64);
impl InstSustD {
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
    pub fn clamp(&self) -> Clamp {
        Clamp(((self.0 >> 49) & 0x3) as i32)
    }
    pub fn tid_b(&self) -> i32 {
        (((self.0 >> 36) & 0x1FFF) as i32)
    }
    pub fn dim(&self) -> SuDim {
        SuDim(((self.0 >> 33) & 0x7) as i32)
    }
    pub fn cache_op(&self) -> CacheOpSt {
        CacheOpSt(((self.0 >> 24) & 0x3) as i32)
    }
    pub fn ba(&self) -> bool {
        (self.0 & 0x800000) != 0
    }
    pub fn size(&self) -> SuSize {
        SuSize(((self.0 >> 20) & 0x7) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstSustB(pub u64);
impl InstSustB {
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
    pub fn cache_op(&self) -> CacheOpSt {
        CacheOpSt(((self.0 >> 24) & 0x3) as i32)
    }
    pub fn rgba(&self) -> SuRgba {
        SuRgba(((self.0 >> 20) & 0xF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstSust(pub u64);
impl InstSust {
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
    pub fn clamp(&self) -> Clamp {
        Clamp(((self.0 >> 49) & 0x3) as i32)
    }
    pub fn tid_b(&self) -> i32 {
        (((self.0 >> 36) & 0x1FFF) as i32)
    }
    pub fn dim(&self) -> SuDim {
        SuDim(((self.0 >> 33) & 0x7) as i32)
    }
    pub fn cache_op(&self) -> CacheOpSt {
        CacheOpSt(((self.0 >> 24) & 0x3) as i32)
    }
    pub fn rgba(&self) -> SuRgba {
        SuRgba(((self.0 >> 20) & 0xF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstSync(pub u64);
impl InstSync {
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
pub struct InstTex(pub u64);
impl InstTex {
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
    pub fn lod(&self) -> Lod {
        Lod(((self.0 >> 55) & 0x7) as i32)
    }
    pub fn aoffi(&self) -> bool {
        (self.0 & 0x40000000000000) != 0
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
pub struct InstTexB(pub u64);
impl InstTexB {
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
    pub fn lcb(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 51) & 0x7) as i32)
    }
    pub fn lodb(&self) -> Lod {
        Lod(((self.0 >> 37) & 0x7) as i32)
    }
    pub fn aoffib(&self) -> bool {
        (self.0 & 0x1000000000) != 0
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
pub struct InstTexs(pub u64);
impl InstTexs {
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
    pub fn target(&self) -> TexsTarget {
        TexsTarget(((self.0 >> 53) & 0xF) as i32)
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
pub struct InstTld(pub u64);
impl InstTld {
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

