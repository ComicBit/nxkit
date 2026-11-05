//! Switch package formats: PFS0, XCI/HFS0, NCA, BKTR, IVFC, CNMT, NACP, tickets and the `KeySet`.
//!
//! Keys never appear in `Debug`, `Display`, logs, events or errors.
//!
//! Status: skeleton. The modules below are placeholders that fix the crate's shape, see
//! `docs/workspace-layout.md` for the planned public surface.

/// PFS0 and HFS0 containers.
pub mod pfs0 {}

/// NCA headers and sections.
pub mod nca {}

/// BKTR relocation for updates.
pub mod bktr {}

/// IVFC hash tree verification.
pub mod ivfc {}

/// Content meta.
pub mod cnmt {}

/// `KeySet` parsing and key derivation.
pub mod keys {}
