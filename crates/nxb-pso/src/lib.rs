//! Pipeline archive planning: bases, the `metal-tt` script, batch cut and stamp, byte-identical to `pso_offline.py`.
//!
//! Status: skeleton. The modules below are placeholders that fix the crate's shape, see
//! `docs/workspace-layout.md` for the planned public surface.

/// Bases from libraries and pairs.
pub mod plan {}

/// The script text for `metal-tt`.
pub mod script {}

/// Contiguous batches and their keys.
pub mod batches {}
