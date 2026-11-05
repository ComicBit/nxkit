//! RomFS tables, the host's `DMROMFS1` image layout planned before any data is read, and `romfs.id`.
//!
//! Status: skeleton. The modules below are placeholders that fix the crate's shape, see
//! `docs/workspace-layout.md` for the planned public surface.

/// Nintendo RomFS directory and file tables.
pub mod tables {}

/// `ImageLayout::plan`: offsets, index and header of a `DMROMFS1` image.
pub mod layout {}

/// `romfs.id` of a layout.
pub mod identity {}
