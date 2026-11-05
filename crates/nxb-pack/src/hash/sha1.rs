//! SHA-1: the workspace's implementation (`nxb_model::Sha1`, ARMv8 path with run-time detection) behind the vendored
//! signer's interface. (The standalone `nxb-codesign` had its own SHA code and the crate's only `unsafe`, vendoring
//! replaced both.)

use super::SHA1_LEN;

/// Streaming SHA-1.
#[derive(Clone)]
pub struct Sha1(nxb_model::Sha1);

impl Sha1 {
    /// A new hasher.
    pub fn new() -> Self {
        Sha1(nxb_model::Sha1::new())
    }

    /// Feeds `data`.
    pub fn update(&mut self, data: &[u8]) {
        self.0.update(data);
    }

    /// The digest.
    pub fn finalize(self) -> [u8; SHA1_LEN] {
        self.0.finalize().0
    }

    /// The digest by the scalar path (tests compare the two paths).
    pub fn portable_digest(data: &[u8]) -> [u8; SHA1_LEN] {
        let mut h = nxb_model::Sha1::with_backend(nxb_model::Backend::Scalar);
        h.update(data);
        h.finalize().0
    }
}

impl Default for Sha1 {
    fn default() -> Self {
        Self::new()
    }
}
