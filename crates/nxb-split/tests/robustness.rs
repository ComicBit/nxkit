//! Robustness of the decoders against damaged input.
//!
//! No game data and no external fuzzer. Small synthetic inputs are mutated in a deterministic way (single byte changes,
//! 32-bit words set to extreme values, cut-off tails, random garbage from a fixed seed). A decoder may return an error.
//! It must not panic, and it must not run for a long time.

use std::panic::{AssertUnwindSafe, catch_unwind};

use nxb_split::{lz4, yaz0, zstd};

/// Small deterministic generator (xorshift64*). The seed is fixed, so every run sees the same inputs.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

/// Every mutation of `seed` that the test uses.
fn mutations(seed: &[u8]) -> Vec<Vec<u8>> {
    let mut out = vec![seed.to_vec()];
    let interesting = [0u8, 1, 2, 0x7f, 0x80, 0xfe, 0xff];
    for i in 0..seed.len() {
        for &v in &interesting {
            let mut m = seed.to_vec();
            m[i] = v;
            out.push(m);
        }
        let mut m = seed.to_vec();
        m[i] ^= 1 << (i % 8);
        out.push(m);
    }
    // 32-bit words set to extreme values
    for i in (0..seed.len().saturating_sub(3)).step_by(2) {
        for &w in &[0u32, 0xffff_ffff, 0x8000_0000, 0x7fff_ffff, 0x0100_0000] {
            let mut m = seed.to_vec();
            m[i..i + 4].copy_from_slice(&w.to_le_bytes());
            out.push(m);
        }
    }
    // cut-off tails
    for n in 0..seed.len() {
        out.push(seed[..n].to_vec());
    }
    // random garbage behind a valid start
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..300 {
        let mut m = seed.to_vec();
        for _ in 0..1 + rng.next() % 4 {
            let at = (rng.next() as usize) % m.len().max(1);
            if let Some(b) = m.get_mut(at) {
                *b = rng.next() as u8;
            }
        }
        out.push(m);
    }
    for _ in 0..200 {
        let n = (rng.next() % 96) as usize;
        out.push((0..n).map(|_| rng.next() as u8).collect());
    }
    out
}

/// Runs `f` on every mutation and fails when one panics.
fn no_panic(name: &str, seed: &[u8], f: impl Fn(&[u8])) {
    for (k, m) in mutations(seed).into_iter().enumerate() {
        let r = catch_unwind(AssertUnwindSafe(|| f(&m)));
        assert!(r.is_ok(), "{name}: panic on mutation {k}: {m:02x?}");
    }
}

/// A zstd frame with one block. `kind` is 0 raw, 1 RLE.
fn zstd_frame(kind: u8, payload: &[u8], size: usize) -> Vec<u8> {
    let mut f = vec![0x28, 0xB5, 0x2F, 0xFD, 0x20, size as u8];
    let header = ((size as u32) << 3) | (u32::from(kind) << 1) | 1;
    f.extend_from_slice(&header.to_le_bytes()[..3]);
    f.extend_from_slice(payload);
    f
}

/// A zstd frame whose one compressed block has raw literals and no sequences.
fn zstd_compressed_literals(lit: &[u8]) -> Vec<u8> {
    let mut block = vec![(lit.len() as u8) << 3];
    block.extend_from_slice(lit);
    block.push(0);
    let mut f = vec![0x28, 0xB5, 0x2F, 0xFD, 0x20, lit.len() as u8];
    let header = ((block.len() as u32) << 3) | (2 << 1) | 1;
    f.extend_from_slice(&header.to_le_bytes()[..3]);
    f.extend_from_slice(&block);
    f
}

#[test]
fn zstd_seeds_decode() {
    assert_eq!(
        zstd::decompress(&zstd_frame(0, b"hello", 5))
            .ok()
            .as_deref(),
        Some(&b"hello"[..])
    );
    assert_eq!(
        zstd::decompress(&zstd_frame(1, b"x", 9)).ok().as_deref(),
        Some(&b"xxxxxxxxx"[..])
    );
    assert_eq!(
        zstd::decompress(&zstd_compressed_literals(b"abcdef"))
            .ok()
            .as_deref(),
        Some(&b"abcdef"[..])
    );
}

#[test]
fn zstd_mutations_do_not_panic() {
    for (name, seed) in [
        ("raw", zstd_frame(0, b"hello world", 11)),
        ("rle", zstd_frame(1, b"z", 30)),
        ("literals", zstd_compressed_literals(b"0123456789abcdef")),
    ] {
        no_panic(name, &seed, |d| {
            let _ = zstd::decompress(d);
        });
    }
}

#[test]
fn lz4_mutations_do_not_panic() {
    // token 0x52: 5 literals, match length 2 + 4. Then an offset of 3.
    let seed = [0x52u8, b'a', b'b', b'c', b'd', b'e', 3, 0, 0x10, b'z'];
    no_panic("lz4", &seed, |d| {
        for size in [0usize, 5, 16, 64] {
            let _ = lz4::block(d, size);
        }
    });
}

#[test]
fn yaz0_mutations_do_not_panic() {
    // header: magic, size 12, then one group: 8 literal bytes would need 0xFF. Here: 4 literals and a copy.
    let mut seed = b"Yaz0".to_vec();
    seed.extend_from_slice(&12u32.to_be_bytes());
    seed.extend_from_slice(&[0; 8]);
    seed.extend_from_slice(&[0xF0, b'a', b'b', b'c', b'd', 0x30, 0x03]);
    no_panic("yaz0", &seed, |d| {
        let _ = yaz0::decode(d, "test");
    });
}
