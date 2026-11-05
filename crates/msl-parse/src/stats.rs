//! Construct counters (the prototype's `stats` and `reasons` Counters) and a small fast hasher.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// The FxHash function (as in rustc): fast for short keys, not DoS resistant. Inputs here are
/// identifiers and counter keys of one file.
#[derive(Default, Clone, Copy)]
pub struct FxHasher {
    hash: u64,
}

const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

impl FxHasher {
    #[inline]
    fn add(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let (chunks, rest) = bytes.as_chunks::<8>();
        for c in chunks {
            self.add(u64::from_le_bytes(*c));
        }
        if !rest.is_empty() {
            let mut buf = [0u8; 8];
            buf[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(buf));
        }
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add(i as u64);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add(i as u64);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}

pub type FxBuild = BuildHasherDefault<FxHasher>;
pub type FxHashMap<K, V> = HashMap<K, V, FxBuild>;

/// A string-keyed counter that keeps first-insertion order, like Python's `Counter`.
#[derive(Clone, Debug, Default)]
pub struct Counter {
    index: FxHashMap<Box<str>, usize>,
    entries: Vec<(Box<str>, u64)>,
}

impl Counter {
    pub fn new() -> Counter {
        Counter::default()
    }

    /// Adds one. Returns true when the key was new (where Python calls `Counter.__missing__`).
    #[inline]
    pub fn inc(&mut self, key: &str) -> bool {
        if let Some(&i) = self.index.get(key) {
            self.entries[i].1 += 1;
            false
        } else {
            self.index.insert(key.into(), self.entries.len());
            self.entries.push((key.into(), 1));
            true
        }
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn get(&self, key: &str) -> u64 {
        self.index.get(key).map_or(0, |&i| self.entries[i].1)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Entries in first-insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, u64)> {
        self.entries.iter().map(|(k, v)| (&**k, *v))
    }

    pub fn set(&mut self, key: &str, value: u64) {
        if let Some(&i) = self.index.get(key) {
            self.entries[i].1 = value;
        } else {
            self.index.insert(key.into(), self.entries.len());
            self.entries.push((key.into(), value));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_keeps_order() {
        let mut c = Counter::new();
        assert!(c.inc("b"));
        assert!(c.inc("a"));
        assert!(!c.inc("b"));
        assert_eq!(c.iter().collect::<Vec<_>>(), vec![("b", 2), ("a", 1)]);
        assert_eq!(c.get("z"), 0);
    }
}
