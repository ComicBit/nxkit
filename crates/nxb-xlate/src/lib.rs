#![allow(missing_docs)]
//! Rust port of `dmsh_xlate`: the driver around the vendored shader translator.
//!
//! Phase 1 of the port (see `docs/translator-port.md`): the job file, the answers the translator queries (`accessor`), the
//! retry logic and the `res.json` writer (`driver`, `res`), and the visibility counter lowering (`lowering`). The translation
//! core (decoders, IR, emitters, SPIR-V) plugs in through the [`driver::Core`] trait.

pub mod accessor;
pub mod cfg;
pub mod codegen;
pub mod corpus;
pub mod driver;
pub mod emit;
pub mod iface;
pub mod ir;
pub mod job;
pub mod lowering;
pub mod opt;
pub mod res;
pub mod structured;
pub mod translation;
pub mod types;
pub mod xform;

/// The Maxwell decoder crate (kept under its old module path).
pub use nxb_maxwell as maxwell;
pub use nxb_maxwell::attr;
/// The SPIR-V module builder crate (kept under its old module path).
pub use nxb_spvgen as spv;
mod ampere_decode_test;
mod decoder_test;
