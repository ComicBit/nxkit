//! SHA-1 and SHA-256, implemented here (no external crates).
//!
//! Both hashes have a portable implementation and, on AArch64, a path through the ARMv8
//! cryptography instructions (`FEAT_SHA1`, `FEAT_SHA256`, detected at run time). The
//! instruction path is the only `unsafe` code of the crate, it lives in the private `arm` module.
//!
//! Code signing needs SHA-1 only for the legacy `files` dictionary of `CodeResources` and for
//! old deployment targets, it is not used for any security decision here, it reproduces a format.

mod sha1;
mod sha256;

pub use sha1::Sha1;
pub use sha256::Sha256;

/// Length of a SHA-1 digest in bytes.
pub const SHA1_LEN: usize = 20;
/// Length of a SHA-256 digest in bytes.
pub const SHA256_LEN: usize = 32;

/// A digest algorithm used by code signatures (the `hashType` field of a CodeDirectory).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HashType {
    /// SHA-1, `hashType` 1.
    Sha1,
    /// SHA-256, `hashType` 2.
    Sha256,
}

impl HashType {
    /// The value of the CodeDirectory `hashType` field.
    pub fn code(self) -> u8 {
        match self {
            HashType::Sha1 => 1,
            HashType::Sha256 => 2,
        }
    }

    /// Digest length in bytes.
    pub fn digest_len(self) -> usize {
        match self {
            HashType::Sha1 => SHA1_LEN,
            HashType::Sha256 => SHA256_LEN,
        }
    }

    /// Hashes `data` in one call.
    pub fn digest(self, data: &[u8]) -> Vec<u8> {
        match self {
            HashType::Sha1 => sha1(data).to_vec(),
            HashType::Sha256 => sha256(data).to_vec(),
        }
    }
}

/// SHA-1 of `data` in one call.
pub fn sha1(data: &[u8]) -> [u8; SHA1_LEN] {
    let mut h = Sha1::new();
    h.update(data);
    h.finalize()
}

/// SHA-256 of `data` in one call.
pub fn sha256(data: &[u8]) -> [u8; SHA256_LEN] {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize()
}

/// Lower-case hexadecimal form of `bytes`.
pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(DIGITS[(b >> 4) as usize] as char);
        s.push(DIGITS[(b & 15) as usize] as char);
    }
    s
}

/// Parses lower- or upper-case hexadecimal into bytes.`None` on odd length or a bad digit.
pub fn unhex(s: &str) -> Option<Vec<u8>> {
    let b = s.as_bytes();
    if !b.len().is_multiple_of(2) {
        return None;
    }
    let nib = |c: u8| -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            b'A'..=b'F' => Some(c - b'A' + 10),
            _ => None,
        }
    };
    b.as_chunks::<2>()
        .0
        .iter()
        .map(|p| Some(nib(p[0])? << 4 | nib(p[1])?))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small deterministic generator for test data (xorshift64*).
    fn noise(n: usize, seed: u64) -> Vec<u8> {
        let mut x = seed | 1;
        (0..n)
            .map(|_| {
                x ^= x >> 12;
                x ^= x << 25;
                x ^= x >> 27;
                (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 56) as u8
            })
            .collect()
    }

    #[test]
    fn known_vectors() {
        assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(
            hex(&sha1(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(
            hex(&sha1(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
        assert_eq!(
            hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex(&sha256(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn million_a() {
        let data = vec![b'a'; 1_000_000];
        assert_eq!(
            hex(&sha1(&data)),
            "34aa973cd4c4daa4f61eeb2bdbad27316534016f"
        );
        assert_eq!(
            hex(&sha256(&data)),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    #[test]
    fn streaming_equals_one_shot_and_portable() {
        let data = noise(70_001, 7);
        for split in [0usize, 1, 55, 56, 63, 64, 65, 127, 128, 4097, 70_000] {
            let mut a = Sha1::new();
            a.update(&data[..split]);
            a.update(&data[split..]);
            assert_eq!(a.finalize(), sha1(&data));
            let mut b = Sha256::new();
            b.update(&data[..split]);
            b.update(&data[split..]);
            assert_eq!(b.finalize(), sha256(&data));
        }
        // The hardware path (when present) must agree with the portable one on every length.
        for n in 0..300 {
            let d = noise(n, n as u64 + 11);
            assert_eq!(sha1(&d), Sha1::portable_digest(&d), "sha1 len {n}");
            assert_eq!(sha256(&d), Sha256::portable_digest(&d), "sha256 len {n}");
        }
    }

    #[test]
    fn hex_round_trip() {
        let b = noise(33, 3);
        assert_eq!(unhex(&hex(&b)).unwrap(), b);
        assert!(unhex("abc").is_none());
        assert!(unhex("zz").is_none());
    }
}
