//! Zip writer with exactly `pack_ipa.py`'s entry order, headers, zip64 rules, times and modes, over an abstract sink.
//!
//! Status: skeleton. The modules below are placeholders that fix the crate's shape, see
//! `docs/workspace-layout.md` for the planned public surface.

/// Local and central headers, zip64 rules.
pub mod headers {}

/// The writer over an abstract sink.
pub mod writer {}
