//! `ContentKey`: canonical, versioned, domain-separated keys for the store and for single-flight.

use crate::digest::{Digest, ParseError};
use crate::sha256::Sha256;
use std::fmt;

/// The identity of a piece of work or a store entry: SHA-256 over a domain tag and labelled, length-prefixed fields.
///
/// Two keys are equal only when the domain and every (label, value) pair are equal in order. Length prefixes make the
/// encoding unambiguous (no concatenation collisions). A change of key layout changes the domain tag (it carries a version).
///
/// ```
/// use nxb_model::ContentKey;
/// let a = ContentKey::builder("nxb.metal.v1").field("msl", b"kernel void f(){}").field_str("target", "mac").finish();
/// let b = ContentKey::builder("nxb.metal.v1").field("msl", b"kernel void f(){}").field_str("target", "ios").finish();
/// assert_ne!(a, b);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentKey(Digest);

impl ContentKey {
    /// Start a key in a domain such as `nxb.store.metallib.v1`.
    pub fn builder(domain: &str) -> ContentKeyBuilder {
        let mut h = Sha256::new();
        h.update(b"nxb-content-key/1\0");
        put(&mut h, domain.as_bytes());
        ContentKeyBuilder {
            hasher: h,
            description: domain.to_string(),
        }
    }

    /// Wrap a digest that is already a content key (for example read back from a manifest).
    pub fn from_digest(d: Digest) -> ContentKey {
        ContentKey(d)
    }

    /// Parse 64 hex characters.
    pub fn from_hex(s: &str) -> Result<ContentKey, ParseError> {
        Digest::from_hex(s).map(ContentKey)
    }

    /// The digest.
    pub fn digest(&self) -> &Digest {
        &self.0
    }

    /// 64 lower-case hex characters.
    pub fn to_hex(&self) -> String {
        self.0.to_hex()
    }
}

impl fmt::Display for ContentKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl fmt::Debug for ContentKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContentKey({})", self.0.to_hex())
    }
}

fn put(h: &mut Sha256, bytes: &[u8]) {
    h.update(&(bytes.len() as u64).to_be_bytes());
    h.update(bytes);
}

/// Builder for `ContentKey`. Also keeps a short human description for `explain` output (labels and value sizes only,
/// never the values, which may be shader text).
#[derive(Debug, Clone)]
pub struct ContentKeyBuilder {
    hasher: Sha256,
    description: String,
}

impl ContentKeyBuilder {
    /// Add a labelled byte field.
    pub fn field(mut self, label: &str, value: &[u8]) -> Self {
        put(&mut self.hasher, label.as_bytes());
        put(&mut self.hasher, value);
        self.description
            .push_str(&format!(" {label}=<{} bytes>", value.len()));
        self
    }

    /// Add a labelled text field.
    pub fn field_str(self, label: &str, value: &str) -> Self {
        self.field(label, value.as_bytes())
    }

    /// Add a labelled digest field (a dependency's identity).
    pub fn field_digest(self, label: &str, value: &Digest) -> Self {
        self.field(label, &value.0)
    }

    /// Add a labelled integer field.
    pub fn field_u64(self, label: &str, value: u64) -> Self {
        self.field(label, &value.to_be_bytes())
    }

    /// The description so far: domain, labels and value sizes.
    pub fn describe(&self) -> &str {
        &self.description
    }

    /// The key.
    pub fn finish(self) -> ContentKey {
        ContentKey(self.hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nxb_testkit::check;

    #[test]
    fn deterministic() {
        let k = || {
            ContentKey::builder("d")
                .field("a", b"1")
                .field_u64("n", 5)
                .finish()
        };
        assert_eq!(k(), k());
    }

    #[test]
    fn no_concatenation_collisions() {
        let a = ContentKey::builder("d")
            .field("a", b"xy")
            .field("b", b"z")
            .finish();
        let b = ContentKey::builder("d")
            .field("a", b"x")
            .field("b", b"yz")
            .finish();
        let c = ContentKey::builder("da").field("", b"xyz").finish();
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_ne!(
            ContentKey::builder("d").finish(),
            ContentKey::builder("e").finish()
        );
    }

    /// Property: different field values (same layout) give different keys, equal values give equal keys.
    #[test]
    fn injective_on_values() {
        check(21, 300, |r| {
            let n = r.below(40) as usize;
            let x = r.bytes(n);
            let mut y = x.clone();
            if r.chance(1, 2) {
                y.push(r.next_u64() as u8);
            } else if !y.is_empty() {
                let i = r.below(y.len() as u64) as usize;
                y[i] ^= 1 + (r.below(255) as u8);
            }
            let kx = ContentKey::builder("p").field("v", &x).finish();
            let ky = ContentKey::builder("p").field("v", &y).finish();
            assert_eq!(x == y, kx == ky);
        });
    }

    #[test]
    fn describe_hides_values() {
        let b = ContentKey::builder("nxb.t.v1").field_str("msl", "secret text");
        assert_eq!(b.describe(), "nxb.t.v1 msl=<11 bytes>");
    }
}
