//! Test support for the nxkit workspace: a seeded pseudo-random generator for property-style tests.
//!
//! Dev-only: used as a `dev-dependency`, never shipped. Std only, no external property-testing crate.
//! A failing property test prints its seed so the case can be replayed.
//! Later: synthetic fixture generators, the parity differ and the Python oracle runner.

/// SplitMix64: small, fast, good enough for test input generation. Deterministic per seed.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// A generator from a seed.
    pub fn new(seed: u64) -> Self {
        Rng { state: seed }
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A value in `0..n` (0 when `n` is 0).
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        self.next_u64() % n
    }

    /// A value in `lo..=hi`.
    pub fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.below(hi.saturating_sub(lo).saturating_add(1))
    }

    /// True with probability `num / den`.
    pub fn chance(&mut self, num: u64, den: u64) -> bool {
        self.below(den) < num
    }

    /// `len` random bytes.
    pub fn bytes(&mut self, len: usize) -> Vec<u8> {
        (0..len).map(|_| self.next_u64() as u8).collect()
    }

    /// A random string of `len` chars drawn from `alphabet`.
    pub fn string_from(&mut self, alphabet: &[char], len: usize) -> String {
        if alphabet.is_empty() {
            return String::new();
        }
        (0..len)
            .map(|_| alphabet[self.below(alphabet.len() as u64) as usize])
            .collect()
    }
}

/// Run `cases` property cases, each with its own derived seed, the seed is in the panic message of a failing case.
pub fn check<F: FnMut(&mut Rng)>(base_seed: u64, cases: u32, mut f: F) {
    for i in 0..cases {
        let seed = base_seed ^ (u64::from(i)).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        let mut rng = Rng::new(seed);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&mut rng)));
        if let Err(e) = outcome {
            eprintln!("property case {i} failed, replay with Rng::new({seed:#x})");
            std::panic::resume_unwind(e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_per_seed() {
        let a: Vec<u64> = (0..5)
            .map({
                let mut r = Rng::new(7);
                move |_| r.next_u64()
            })
            .collect();
        let b: Vec<u64> = (0..5)
            .map({
                let mut r = Rng::new(7);
                move |_| r.next_u64()
            })
            .collect();
        assert_eq!(a, b);
        assert_ne!(Rng::new(7).next_u64(), Rng::new(8).next_u64());
    }

    #[test]
    fn range_bounds() {
        check(1, 100, |r| {
            let v = r.range(3, 9);
            assert!((3..=9).contains(&v));
        });
    }
}
