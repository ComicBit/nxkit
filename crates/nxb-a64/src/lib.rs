//! AArch64 instruction decoding and the encoders the module rewriter needs (replaces Capstone).
//!
//! The decoder gives, for one instruction word, what a Capstone 5 `CsInsn` with details gives the rewriter:
//! the mnemonic (with its aliases), the operands, the registers read and written (with Capstone's quirks), the
//! write-back flag and whether the word decodes at all. Advanced SIMD and floating-point instructions are
//! modelled only as far as the rewriter looks at them (see `simd`).
//!
//! Oracle: `tests/oracle.rs` compares every distinct instruction word of real modules with a dump made by
//! Capstone (environment variable `NXB_A64_ORACLE`).

mod br;
mod dpi;
mod dpr;
mod ldst;
mod simd;
mod simdls;
mod util;

/// The `nso_asm` encoders.
pub mod encode;
/// The decoded instruction.
pub mod insn;

pub use insn::{Fk, Insn, Op, Reg, Sh, Wb};

/// Decodes one instruction word at `addr`.`None` where Capstone does not decode it.
pub fn decode(w: u32, addr: u64) -> Option<Insn> {
    match (w >> 25) & 0xf {
        0 => {
            if w >> 16 == 0 {
                Some(util::mk(
                    w,
                    addr,
                    "udf",
                    vec![util::iop(i64::from(w & 0xffff))],
                    ".",
                    false,
                    false,
                ))
            } else {
                None
            }
        }
        0b1000 | 0b1001 => dpi::decode(w, addr),
        0b1010 | 0b1011 => br::decode(w, addr),
        0b0100 | 0b0110 | 0b1100 | 0b1110 => ldst::decode(w, addr),
        0b0101 | 0b1101 => dpr::decode(w, addr),
        0b0111 | 0b1111 => simd::decode(w, addr),
        _ => None,
    }
}
