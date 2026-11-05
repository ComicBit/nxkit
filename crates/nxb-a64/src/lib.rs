//! AArch64 instruction decoding and the encoders the module rewriter needs (replaces Capstone).
//!
//! Status: skeleton. The modules below are placeholders that fix the crate's shape, see
//! `docs/workspace-layout.md` for the planned public surface.

/// `decode(u32) -> Insn` with operands and registers read and written.
pub mod decode {}

/// The `nso_asm` encoders.
pub mod encode {}
