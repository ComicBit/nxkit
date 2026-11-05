//! Produced by tools/gen_xlate_decoders.py from InstDecoders.cs: do not edit.
#![allow(non_upper_case_globals, non_camel_case_types, dead_code, unused_parens, clippy::all)]

use crate::enums::*;

#[derive(Clone, Copy, Debug)]
pub struct InstConditional(pub u64);
impl InstConditional {
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
pub struct InstAl2p(pub u64);
impl InstAl2p {
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
    pub fn al_size(&self) -> AlSize {
        AlSize(((self.0 >> 47) & 0x3) as i32)
    }
    pub fn aio(&self) -> bool {
        (self.0 & 0x100000000) != 0
    }
    pub fn imm11(&self) -> i32 {
        (((self.0 >> 20) & 0x7FF) as i32)
    }
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 44) & 0x7) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstAld(pub u64);
impl InstAld {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn src_b(&self) -> i32 {
        (((self.0 >> 39) & 0xFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn imm11(&self) -> i32 {
        (((self.0 >> 20) & 0x7FF) as i32)
    }
    pub fn p(&self) -> bool {
        (self.0 & 0x80000000) != 0
    }
    pub fn o(&self) -> bool {
        (self.0 & 0x100000000) != 0
    }
    pub fn al_size(&self) -> AlSize {
        AlSize(((self.0 >> 47) & 0x3) as i32)
    }
    pub fn phys(&self) -> bool {
        !self.p() && self.imm11() == 0 && self.src_a() != 255
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstAst(pub u64);
impl InstAst {
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn src_b(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
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
    pub fn imm11(&self) -> i32 {
        (((self.0 >> 20) & 0x7FF) as i32)
    }
    pub fn p(&self) -> bool {
        (self.0 & 0x80000000) != 0
    }
    pub fn al_size(&self) -> AlSize {
        AlSize(((self.0 >> 47) & 0x3) as i32)
    }
    pub fn phys(&self) -> bool {
        !self.p() && self.imm11() == 0 && self.src_a() != 255
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstAtom(pub u64);
impl InstAtom {
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
    pub fn imm20(&self) -> i32 {
        (((self.0 >> 28) & 0xFFFFF) as i32)
    }
    pub fn size(&self) -> AtomSize {
        AtomSize(((self.0 >> 49) & 0x7) as i32)
    }
    pub fn op(&self) -> AtomOp {
        AtomOp(((self.0 >> 52) & 0xF) as i32)
    }
    pub fn e(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstAtomCas(pub u64);
impl InstAtomCas {
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
    pub fn bc_rz(&self) -> i32 {
        (((self.0 >> 50) & 0x3) as i32)
    }
    pub fn e(&self) -> bool {
        (self.0 & 0x1000000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstAtoms(pub u64);
impl InstAtoms {
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
    pub fn imm22(&self) -> i32 {
        (((self.0 >> 30) & 0x3FFFFF) as i32)
    }
    pub fn atoms_size(&self) -> AtomsSize {
        AtomsSize(((self.0 >> 28) & 0x3) as i32)
    }
    pub fn atom_op(&self) -> AtomOp {
        AtomOp(((self.0 >> 52) & 0xF) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstAtomsCas(pub u64);
impl InstAtomsCas {
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
    pub fn atoms_bc_rz(&self) -> i32 {
        (((self.0 >> 28) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstB2r(pub u64);
impl InstB2r {
    pub fn dest(&self) -> i32 {
        ((self.0 & 0xFF) as i32)
    }
    pub fn dest_pred(&self) -> i32 {
        (((self.0 >> 45) & 0x7) as i32)
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
    pub fn mode(&self) -> BarMode {
        BarMode(((self.0 >> 32) & 0x3) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstBar(pub u64);
impl InstBar {
    pub fn src_a(&self) -> i32 {
        (((self.0 >> 8) & 0xFF) as i32)
    }
    pub fn imm12(&self) -> i32 {
        (((self.0 >> 20) & 0xFFF) as i32)
    }
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn src_pred_inv(&self) -> bool {
        (self.0 & 0x40000000000) != 0
    }
    pub fn src_pred(&self) -> i32 {
        (((self.0 >> 39) & 0x7) as i32)
    }
    pub fn bar_op(&self) -> BarOp {
        BarOp(((self.0 >> 32) & 0x7) as i32)
    }
    pub fn bar_red_op(&self) -> BarRedOp {
        BarRedOp(((self.0 >> 35) & 0x3) as i32)
    }
    pub fn a_fix_bar(&self) -> bool {
        (self.0 & 0x100000000000) != 0
    }
    pub fn b_fix_bar(&self) -> bool {
        (self.0 & 0x80000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstBfeR(pub u64);
impl InstBfeR {
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
    pub fn brev(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstBfeI(pub u64);
impl InstBfeI {
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
    pub fn brev(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstBfeC(pub u64);
impl InstBfeC {
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
    pub fn brev(&self) -> bool {
        (self.0 & 0x10000000000) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstBfiR(pub u64);
impl InstBfiR {
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
}

#[derive(Clone, Copy, Debug)]
pub struct InstBfiI(pub u64);
impl InstBfiI {
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
}

#[derive(Clone, Copy, Debug)]
pub struct InstBfiC(pub u64);
impl InstBfiC {
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
}

#[derive(Clone, Copy, Debug)]
pub struct InstBfiRc(pub u64);
impl InstBfiRc {
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
}

#[derive(Clone, Copy, Debug)]
pub struct InstBpt(pub u64);
impl InstBpt {
    pub fn imm20(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFFF) as i32)
    }
    pub fn bpt(&self) -> Bpt {
        Bpt(((self.0 >> 6) & 0x7) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstBra(pub u64);
impl InstBra {
    pub fn pred(&self) -> i32 {
        (((self.0 >> 16) & 0x7) as i32)
    }
    pub fn pred_inv(&self) -> bool {
        (self.0 & 0x80000) != 0
    }
    pub fn ccc(&self) -> Ccc {
        Ccc((self.0 & 0x1F) as i32)
    }
    pub fn imm24(&self) -> i32 {
        (((self.0 >> 20) & 0xFFFFFF) as i32)
    }
    pub fn ca(&self) -> bool {
        (self.0 & 0x20) != 0
    }
    pub fn lmt(&self) -> bool {
        (self.0 & 0x40) != 0
    }
    pub fn u(&self) -> bool {
        (self.0 & 0x80) != 0
    }
}

