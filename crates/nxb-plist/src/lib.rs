//! Property list writers: CoreFoundation XML (for `CodeResources`) and Python `plistlib` XML (for `Info.plist`), both byte-exact.
//!
//! Status: skeleton. The modules below are placeholders that fix the crate's shape, see
//! `docs/workspace-layout.md` for the planned public surface.

/// The plist value model.
pub mod value {}

/// Writer byte-equal to `CFPropertyListCreateData` XML.
pub mod cf_xml {}

/// Writer byte-equal to Python `plistlib.dumps`.
pub mod plistlib_xml {}
