#![allow(missing_docs)]
//! The shader translator: Maxwell and Ampere machine code to Metal Shading Language.
//!
//! The front end decodes the machine code (`cfg`, `ir`, `opt`, `xform`, `structured`, `translation`) and answers the
//! queries of the job through `accessor`. The retry loop of `driver` repeats a translation with the guessed image and
//! texture types and with 64 bit floats lowered to 32 bit. The back end `msl` turns the structured program into MSL
//! libraries and their reflection.

pub mod accessor;
pub mod cfg;
pub mod corpus;
pub mod driver;
pub mod emit;
pub mod ir;
pub mod job;
pub mod msl;
pub mod opt;
pub mod props;
pub mod structured;
pub mod translation;
pub mod types;
pub mod xform;

/// The Maxwell decoder crate (kept under its old module path).
pub use nxb_maxwell as maxwell;
pub use nxb_maxwell::attr;
mod ampere_decode_test;
mod decoder_test;
