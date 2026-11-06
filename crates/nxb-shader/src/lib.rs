//! The shader domain: the translator jobs, the chain version, the guard tables, the header and the target conventions
//! of the shader step, and the cut of a pixel shader's side library (design-v2 sb-domain).
//!
//! No I/O. Everything here is output-relevant and hashed into the chain version V. The MSL itself comes from `nxb-xlate`.
//!
//! Status: skeleton. The modules below are placeholders that fix the crate's shape, see
//! `docs/workspace-layout.md` for the planned public surface.

/// NVN shader record decoding (`BTCDBTCD` records).
pub mod nvn {}

pub mod chain;

pub mod guards;
pub mod job;
pub(crate) mod mesh;

/// Shader keys and compile-input keys.
pub mod keys {}

pub mod pch;
pub mod side;
pub mod target;

/// The single shim that gives Python `re` semantics for 1:1 ported patterns.
pub mod re {}
