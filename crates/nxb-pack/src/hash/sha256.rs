//! SHA-256: the workspace's implementation (`nxb_model::Sha256`) behind the vendored signer's interface.

use super::SHA256_LEN;

/// Streaming SHA-256.
#[derive(Clone)]
pub struct Sha256(nxb_model::Sha256);

impl Sha256 {
    /// A new hasher.
    pub fn new() -> Self {
        Sha256(nxb_model::Sha256::new())
    }

    /// Feeds `data`.
    pub fn update(&mut self, data: &[u8]) {
        self.0.update(data);
    }

    /// The digest.
    pub fn finalize(self) -> [u8; SHA256_LEN] {
        self.0.finalize().0
    }

    /// The digest by the scalar path.
    pub fn portable_digest(data: &[u8]) -> [u8; SHA256_LEN] {
        let mut h = nxb_model::Sha256::with_backend(nxb_model::Backend::Scalar);
        h.update(data);
        h.finalize().0
    }
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}
