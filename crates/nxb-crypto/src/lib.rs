//! Safe cryptography for the stages around the shader step: AES-CTR/XTS/ECB, SHA-1 and the one-pass SHA-1 + SHA-256 `DualHasher`.
//!
//! SHA-256 itself lives in `nxb-model`, system-accelerated paths come through `nxb-sys`.
//!
//! Status: skeleton. The modules below are placeholders that fix the crate's shape, see
//! `docs/workspace-layout.md` for the planned public surface.

/// AES-128 in CTR, XTS (Nintendo sector numbering) and ECB modes.
pub mod aes {}

/// SHA-1, needed only for `CodeResources` `files`.
pub mod sha1 {}

/// `DualHasher`: SHA-1 and SHA-256 over the same bytes in one pass.
pub mod dual {}
