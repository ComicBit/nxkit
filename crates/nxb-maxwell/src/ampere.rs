//! `AmpereOp`: field access on one 128-bit Ampere (sm_86) instruction, and the opcodes the decoder knows by name.

use super::table::InstOp;

/// `AmpereOpcodes`.
pub mod opc {
    pub const BRA: usize = 0x947;
    pub const BRX: usize = 0x949;
    pub const BRXU: usize = 0x958;
    pub const EXIT: usize = 0x94d;
    pub const KILL: usize = 0x95b;
    pub const RET: usize = 0x950;
    pub const CALL: usize = 0x944;
    pub const IPA: usize = 0x326;
    pub const IPA_IDX: usize = 0x526;
    pub const ALD: usize = 0x321;
    pub const AST: usize = 0x322;
    pub const LDC: usize = 0xb82;
    pub const LDL: usize = 0x983;
}

/// One 128-bit instruction.
#[derive(Clone, Copy, Debug)]
pub struct AmpOp {
    pub lo: u64,
    pub hi: u64,
}

impl AmpOp {
    pub fn new(lo: u64, hi: u64) -> AmpOp {
        AmpOp { lo, hi }
    }

    pub fn of(op: &InstOp) -> AmpOp {
        AmpOp::new(op.raw, op.raw_hi)
    }

    /// Bits `[lo, hi)` as an unsigned value (at most 64 bits wide).
    pub fn field(&self, lo: u32, hi: u32) -> u64 {
        let v = (u128::from(self.hi) << 64) | u128::from(self.lo);
        let n = hi - lo;
        let mask = if n >= 64 {
            u128::from(u64::MAX)
        } else {
            (1u128 << n) - 1
        };
        ((v >> lo) & mask) as u64
    }

    pub fn bits(&self, lo: u32, hi: u32) -> i32 {
        self.field(lo, hi) as i32
    }

    pub fn bit(&self, n: u32) -> bool {
        self.field(n, n + 1) != 0
    }

    pub fn signed_field(&self, lo: u32, hi: u32) -> i64 {
        let n = hi - lo;
        ((self.field(lo, hi) << (64 - n)) as i64) >> (64 - n)
    }

    pub fn opcode(&self) -> usize {
        (self.lo & 0xfff) as usize
    }

    /// ALU operand form (bits 9..12).
    pub fn form(&self) -> i32 {
        self.bits(9, 12)
    }

    pub fn pred(&self) -> i32 {
        self.bits(12, 15)
    }

    pub fn pred_inv(&self) -> bool {
        self.bit(15)
    }

    pub fn rd(&self) -> i32 {
        self.bits(16, 24)
    }

    pub fn ra(&self) -> i32 {
        self.bits(24, 32)
    }

    pub fn rb(&self) -> i32 {
        self.bits(32, 40)
    }

    pub fn rc(&self) -> i32 {
        self.bits(64, 72)
    }

    pub fn imm32(&self) -> u32 {
        (self.lo >> 32) as u32
    }

    /// Branch-like instructions: a signed word offset at 34..82, relative to the next instruction.
    pub fn branch_offset(&self) -> i64 {
        self.signed_field(34, 82) * 4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields() {
        let a = AmpOp::new(0x0123_4567_89ab_cdef, 0xfedc_ba98_7654_3210);
        assert_eq!(a.opcode(), 0xdef);
        assert_eq!(a.form(), 6);
        assert_eq!(a.field(0, 64), a.lo);
        assert_eq!(a.field(64, 128), a.hi);
        assert_eq!(a.bits(64, 72), 0x10);
        assert_eq!(AmpOp::new(0x3 << 50, 0).signed_field(50, 52), -1);
        // branch offsets are signed words
        assert_eq!(AmpOp::new(0, 0).branch_offset(), 0);
        assert_eq!(AmpOp::new(u64::MAX << 34, u64::MAX).branch_offset(), -4);
    }
}
