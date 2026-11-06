//! A fast, deterministic hasher for the maps of the compiler (the FxHash of rustc). Keys are compiler-made names, so
//! flooding attacks do not apply, and SipHash cost a measurable share of every shader.

use std::hash::{BuildHasherDefault, Hasher};

const K: u64 = 0x517c_c1b7_2722_0a95;

/// The Fx word hasher.
#[derive(Clone, Copy, Default)]
pub struct FxHasher(u64);

impl FxHasher {
    #[inline]
    fn add(&mut self, w: u64) {
        self.0 = (self.0.rotate_left(5) ^ w).wrapping_mul(K);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for c in &mut chunks {
            let mut w = [0u8; 8];
            w.copy_from_slice(c);
            self.add(u64::from_le_bytes(w));
        }
        let rest = chunks.remainder();
        if !rest.is_empty() {
            let mut w = [0u8; 8];
            w[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(w) ^ ((rest.len() as u64) << 59));
        }
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add(u64::from(i));
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.add(u64::from(i));
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add(u64::from(i));
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add(i);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add(i as u64);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

/// A `HashMap` with the Fx hasher.
pub type HashMap<K, V> = std::collections::HashMap<K, V, BuildHasherDefault<FxHasher>>;

/// A `HashSet` with the Fx hasher.
pub type HashSet<K> = std::collections::HashSet<K, BuildHasherDefault<FxHasher>>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::{BuildHasher, BuildHasherDefault};

    #[test]
    fn equal_keys_hash_alike_and_lengths_are_told_apart() {
        let b = BuildHasherDefault::<FxHasher>::default();
        assert_eq!(b.hash_one("m2r.1"), b.hash_one(String::from("m2r.1")));
        assert_ne!(
            b.hash_one([0u8; 3].as_slice()),
            b.hash_one([0u8; 4].as_slice())
        );
        let mut m: HashMap<String, u32> = HashMap::default();
        m.insert("a".into(), 1);
        assert_eq!(m.get("a"), Some(&1));
    }
}
