//! The shader domain: pure functions from package records to final MSL, reflection and output bytes (design-v2 sb-domain).
//!
//! No I/O. Everything here is output-relevant and hashed into the chain version V. Only this crate and `nxb-air` construct
//! `OutputFile` values.
//!
//! Status: skeleton. The modules below are placeholders that fix the crate's shape, see
//! `docs/workspace-layout.md` for the planned public surface.

/// NVN shader record decoding (`BTCDBTCD` records).
pub mod nvn {}

pub mod chain;
pub mod msl;

pub mod reflect;

pub mod variants;

pub mod guards;
pub mod job;

/// Shader keys and compile-input keys.
pub mod keys {}

pub mod pch;
pub mod side;
pub mod target;

/// The single shim that gives Python `re` semantics for 1:1 ported patterns.
pub mod re {}
