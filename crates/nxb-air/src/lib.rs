//! The own shader front end: lowering of the MSL AST to LLVM IR for the AIR target (design-v2 section 16).
//!
//! Not byte-identical to Apple's `metal -c` by design, validated by the GPU differential harness, coverage and whole-scene
//! comparison, with a per-shader fallback to Apple's compiler.
//!
//! Status: skeleton. The modules below are placeholders that fix the crate's shape, see
//! `docs/workspace-layout.md` for the planned public surface.

/// One module per construct family, enabled by data.
pub mod families {}

/// `covers(&Tree) -> Coverage`: does every construct belong to an enabled family.
pub mod coverage {}

/// The LLVM IR text model and printer.
pub mod ir {}
