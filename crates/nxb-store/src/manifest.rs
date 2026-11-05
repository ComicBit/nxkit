//! Entry manifests: a deterministic, self-checksummed text format.
//!
//! ```text
//! nxb-store-manifest 1
//! key <64 hex>
//! file <sha256 hex> <len> <name>      one line per file, sorted by name
//! end <sha256 hex of every byte before this line>
//! ```

use crate::error::StoreError;
use nxb_model::{ContentKey, Digest, FileDigest, sha256};
use std::collections::BTreeMap;

const HEADER: &str = "nxb-store-manifest 1\n";

/// One file of an entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManifestFile {
    /// Relative name (`/`-separated components, validated by [`validate_name`]).
    pub name: String,
    /// Size and digest, the blob is `blobs/<digest>`.
    pub digest: FileDigest,
}

/// An entry: a key and its files, sorted by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    /// The entry's key.
    pub key: ContentKey,
    /// Files sorted by name, names unique.
    pub files: Vec<ManifestFile>,
}

/// Check a relative file name: non-empty `/`-separated components, none empty, `.` or `..`, no NUL, CR, LF or
/// backslash, at most 1024 bytes in all and 255 per component.
pub fn validate_name(name: &str) -> Result<(), StoreError> {
    let bad = || Err(StoreError::BadName(name.chars().take(80).collect()));
    if name.is_empty() || name.len() > 1024 {
        return bad();
    }
    if name.contains(['\0', '\n', '\r', '\\']) {
        return bad();
    }
    for comp in name.split('/') {
        if comp.is_empty() || comp == "." || comp == ".." || comp.len() > 255 {
            return bad();
        }
    }
    Ok(())
}

impl Manifest {
    /// Build from files, names are validated and must be unique.
    pub fn new(
        key: ContentKey,
        files: BTreeMap<String, FileDigest>,
    ) -> Result<Manifest, StoreError> {
        for name in files.keys() {
            validate_name(name)?;
        }
        Ok(Manifest {
            key,
            files: files
                .into_iter()
                .map(|(name, digest)| ManifestFile { name, digest })
                .collect(),
        })
    }

    /// The canonical bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut s = String::with_capacity(64 + self.files.len() * 100);
        s.push_str(HEADER);
        s.push_str("key ");
        s.push_str(&self.key.to_hex());
        s.push('\n');
        for f in &self.files {
            s.push_str(&format!(
                "file {} {} {}\n",
                f.digest.sha256, f.digest.len, f.name
            ));
        }
        let check = sha256(s.as_bytes());
        s.push_str(&format!("end {check}\n"));
        s.into_bytes()
    }

    /// Parse canonical bytes. `None` for anything damaged: wrong header, bad line, unsorted or duplicate names,
    /// checksum mismatch, trailing bytes.
    pub fn parse(bytes: &[u8]) -> Option<Manifest> {
        let text = std::str::from_utf8(bytes).ok()?;
        let body_end = text.rfind("end ")?;
        let (body, tail) = text.split_at(body_end);
        let check = tail.strip_prefix("end ")?.strip_suffix('\n')?;
        if Digest::from_hex(check).ok()? != sha256(body.as_bytes()) {
            return None;
        }
        let mut lines = body.strip_prefix(HEADER)?.lines();
        let key = ContentKey::from_hex(lines.next()?.strip_prefix("key ")?).ok()?;
        let mut files: Vec<ManifestFile> = Vec::new();
        for line in lines {
            let rest = line.strip_prefix("file ")?;
            let (hex, rest) = rest.split_once(' ')?;
            let (len, name) = rest.split_once(' ')?;
            validate_name(name).ok()?;
            if files.last().is_some_and(|p| p.name.as_str() >= name) {
                return None;
            }
            files.push(ManifestFile {
                name: name.to_string(),
                digest: FileDigest {
                    len: len.parse().ok()?,
                    sha256: Digest::from_hex(hex).ok()?,
                },
            });
        }
        Some(Manifest { key, files })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nxb_testkit::check;

    fn key() -> ContentKey {
        ContentKey::builder("t").finish()
    }

    #[test]
    fn names() {
        for ok in ["a", "a.metallib", "ExeFS/main", "html/x y.html", "é"] {
            assert!(validate_name(ok).is_ok(), "{ok}");
        }
        for bad in [
            "", ".", "..", "/abs", "a/", "a//b", "a/../b", "a\nb", "a\\b", "a\0",
        ] {
            assert!(validate_name(bad).is_err(), "{bad:?}");
        }
    }

    /// Property: round trip, and every single-byte change is detected.
    #[test]
    fn round_trip_and_damage_detection() {
        check(61, 100, |r| {
            let mut files = BTreeMap::new();
            for i in 0..r.below(6) {
                let len = r.below(1000) as usize;
                let data = r.bytes(len);
                files.insert(format!("f{i}.bin"), FileDigest::of(&data));
            }
            let m = Manifest::new(key(), files).unwrap();
            let bytes = m.to_bytes();
            assert_eq!(Manifest::parse(&bytes), Some(m));
            let mut bad = bytes.clone();
            let i = r.below(bad.len() as u64) as usize;
            bad[i] ^= 1 + r.below(255) as u8;
            assert_eq!(Manifest::parse(&bad), None, "flip at {i} not detected");
            assert_eq!(Manifest::parse(&bytes[..bytes.len() - 1]), None);
        });
    }

    #[test]
    fn order_independent_bytes() {
        let d = FileDigest::of(b"x");
        let mut a = BTreeMap::new();
        a.insert("b".to_string(), d);
        a.insert("a".to_string(), d);
        let m = Manifest::new(key(), a).unwrap();
        let text = String::from_utf8(m.to_bytes()).unwrap();
        let a_pos = text.find(" a\n").unwrap();
        let b_pos = text.find(" b\n").unwrap();
        assert!(a_pos < b_pos);
    }
}
