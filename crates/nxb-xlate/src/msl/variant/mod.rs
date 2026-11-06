//! The variant libraries of a shader, written on top of the base library.
//!
//! The base library is built by `entry::build`. Its pieces are the hooks for the variants:
//! `entry::entry_text` takes the name of the entry function, so a variant writes another entry (`main0_z`,
//! `main0_gs`, `main0_discard`, `main0_l`, `main0_lz`, `main0_u`, `main0_m`) around the same helper functions. The
//! vertex pulling (`_bp`), vertex input (`_bv`), fetch (`_bf`) and side (`_bx`) libraries are separate sources that
//! the same emitter writes with other options. The vertex pad takes `Context::partners`.
//!
//! No variant is written yet.

pub mod pixel;
pub mod side;
