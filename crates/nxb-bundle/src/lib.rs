//! App layouts: `MacApp::plan` and `IosPayload::plan` produce an ordered bundle plan from typed inputs, without I/O.
//!
//! Status: skeleton. The modules below are placeholders that fix the crate's shape, see
//! `docs/workspace-layout.md` for the planned public surface.

/// The Mac app layout.
pub mod mac {}

/// The iOS payload layout.
pub mod ios {}

/// `BundlePlan`: ordered destination, source and mode.
pub mod plan {}
