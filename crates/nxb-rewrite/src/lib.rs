//! Executable module parsing (NSO, NRO) and the AArch64 rewriter with its rules and TSV outputs (replaces `patch_nso.py`).
//!
//! Status: skeleton. The modules below are placeholders that fix the crate's shape, see
//! `docs/workspace-layout.md` for the planned public surface.

/// NSO and NRO parsing.
pub mod nso {}

/// One file per rewrite rule, collected by `registry()`.
pub mod rules {}

/// The five TSV outputs per module.
pub mod tables {}
