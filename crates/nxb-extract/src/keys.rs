//! `prod.keys` as LibHac's `ExternalKeyReader` reads it: `name = hex` lines. Values are never formatted: no `Debug` with
//! contents, errors name keys only, the store is zeroed on drop.

use std::collections::BTreeMap;
use std::fmt;

/// Parsed key file plus title keys imported from common tickets.
#[derive(Default)]
pub struct KeySet {
    keys: BTreeMap<String, Vec<u8>>,
    title_keys: BTreeMap<[u8; 16], [u8; 16]>,
}

impl fmt::Debug for KeySet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "KeySet({} keys, {} title keys, values hidden)",
            self.keys.len(),
            self.title_keys.len()
        )
    }
}

impl Drop for KeySet {
    fn drop(&mut self) {
        for v in self.keys.values_mut() {
            v.iter_mut().for_each(|b| *b = 0);
        }
        for v in self.title_keys.values_mut() {
            *v = [0; 16];
        }
    }
}

/// The key area names by key area index (application, ocean, system).
pub const KEY_AREAS: [&str; 3] = ["application", "ocean", "system"];

impl KeySet {
    /// Parses the text of a key file. Lines that are not `name = hex` are skipped (as LibHac does with unknown lines).
    pub fn parse(text: &str) -> KeySet {
        let mut keys = BTreeMap::new();
        for line in text.lines() {
            let Some((name, value)) = line.split_once('=') else {
                continue;
            };
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim();
            if value.len() % 2 != 0 || name.is_empty() {
                continue;
            }
            let bytes: Option<Vec<u8>> = (0..value.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(value.get(i..i + 2)?, 16).ok())
                .collect();
            if let Some(b) = bytes {
                keys.insert(name, b);
            }
        }
        KeySet {
            keys,
            title_keys: BTreeMap::new(),
        }
    }

    fn get<const N: usize>(&self, name: &str) -> Option<[u8; N]> {
        let v = self.keys.get(name)?;
        v.as_slice().try_into().ok()
    }

    /// `header_key` (32 bytes, XTS key pair).
    pub fn header_key(&self) -> Option<[u8; 32]> {
        self.get("header_key")
    }

    /// `titlekek_<rev>`.
    pub fn titlekek(&self, rev: u8) -> Option<[u8; 16]> {
        self.get(&format!("titlekek_{rev:02x}"))
    }

    /// `key_area_key_<area>_<rev>`.
    pub fn key_area_key(&self, area: u8, rev: u8) -> Option<[u8; 16]> {
        let name = KEY_AREAS.get(area as usize)?;
        self.get(&format!("key_area_key_{name}_{rev:02x}"))
    }

    /// Adds an encrypted title key from a common ticket.
    pub fn add_title_key(&mut self, rights_id: [u8; 16], encrypted: [u8; 16]) {
        self.title_keys.insert(rights_id, encrypted);
    }

    /// The encrypted title key of a rights ID.
    pub fn title_key(&self, rights_id: &[u8; 16]) -> Option<[u8; 16]> {
        self.title_keys.get(rights_id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_hide() {
        let k = KeySet::parse(
            "header_key = 00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff\n# x\ntitlekek_0a=000102030405060708090a0b0c0d0e0f\nbad = 0g\n",
        );
        assert!(k.header_key().is_some());
        assert_eq!(k.titlekek(10).map(|v| v[15]), Some(15));
        assert!(k.titlekek(11).is_none());
        assert!(!format!("{k:?}").contains("0011"));
    }
}
