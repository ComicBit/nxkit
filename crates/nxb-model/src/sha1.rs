//! SHA-1 (FIPS 180-4), needed only for `CodeResources` `files` (design-pipeline-rest 8), and the one-pass
//! SHA-1 + SHA-256 [`DualHasher`].
//!
//! SHA-1 is not used for identity anywhere in this workspace (keys and store use SHA-256). Two backends with identical
//! output, as for SHA-256: the ARMv8 SHA1 instructions through `nxb-sys`, else scalar (ADR W17).

use crate::digest::{FileDigest, to_hex};
use crate::sha256::{Backend, Sha256};
use std::fmt;
use std::io::{self, Write};

const H0: [u32; 5] = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0];

/// A SHA-1 digest.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sha1Digest(pub [u8; 20]);

impl Sha1Digest {
    /// 40 lower-case hex characters.
    pub fn to_hex(&self) -> String {
        to_hex(&self.0)
    }
}

impl fmt::Display for Sha1Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Debug for Sha1Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Sha1Digest({})", self.to_hex())
    }
}

/// Incremental SHA-1.
#[derive(Clone)]
pub struct Sha1 {
    backend: Backend,
    state: [u32; 5],
    block: [u8; 64],
    block_len: usize,
    total_len: u64,
}

impl fmt::Debug for Sha1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sha1")
            .field("total_len", &self.total_len)
            .finish_non_exhaustive()
    }
}

impl Default for Sha1 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha1 {
    /// A fresh hasher (hardware path when available).
    pub fn new() -> Self {
        Self::with_backend(Backend::Auto)
    }

    /// A fresh hasher with a chosen backend.
    pub fn with_backend(backend: Backend) -> Self {
        Sha1 {
            backend,
            state: H0,
            block: [0; 64],
            block_len: 0,
            total_len: 0,
        }
    }

    /// Bytes fed so far.
    pub fn len(&self) -> u64 {
        self.total_len
    }

    /// True when nothing was fed.
    pub fn is_empty(&self) -> bool {
        self.total_len == 0
    }

    /// Feed bytes.
    pub fn update(&mut self, mut data: &[u8]) {
        self.total_len = self.total_len.wrapping_add(data.len() as u64);
        if self.block_len > 0 {
            let take = (64 - self.block_len).min(data.len());
            self.block[self.block_len..self.block_len + take].copy_from_slice(&data[..take]);
            self.block_len += take;
            data = &data[take..];
            if self.block_len < 64 {
                return;
            }
            let block = self.block;
            self.compress_blocks(&[block]);
            self.block_len = 0;
        }
        let (blocks, rest) = data.as_chunks::<64>();
        self.compress_blocks(blocks);
        self.block[..rest.len()].copy_from_slice(rest);
        self.block_len = rest.len();
    }

    /// Finish.
    pub fn finalize(mut self) -> Sha1Digest {
        let bit_len = self.total_len.wrapping_mul(8);
        let mut pad = [0u8; 72];
        pad[0] = 0x80;
        let pad_len = if self.block_len < 56 {
            56 - self.block_len
        } else {
            120 - self.block_len
        };
        let saved = self.total_len;
        self.update(&pad[..pad_len]);
        self.update(&bit_len.to_be_bytes());
        self.total_len = saved;
        let mut out = [0u8; 20];
        for (dst, w) in out.as_chunks_mut::<4>().0.iter_mut().zip(self.state.iter()) {
            *dst = w.to_be_bytes();
        }
        Sha1Digest(out)
    }

    fn compress_blocks(&mut self, blocks: &[[u8; 64]]) {
        if blocks.is_empty() {
            return;
        }
        if self.backend == Backend::Auto && nxb_sys::sha1::compress_blocks(&mut self.state, blocks)
        {
            return;
        }
        for b in blocks {
            compress(&mut self.state, b);
        }
    }
}

fn compress(state: &mut [u32; 5], block: &[u8; 64]) {
    let mut w = [0u32; 80];
    for (i, word) in block.as_chunks::<4>().0.iter().enumerate() {
        w[i] = u32::from_be_bytes(*word);
    }
    for i in 16..80 {
        w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
    }
    let [mut a, mut b, mut c, mut d, mut e] = *state;
    for (i, wi) in w.iter().enumerate() {
        let (f, k) = match i / 20 {
            0 => ((b & c) | (!b & d), 0x5a827999),
            1 => (b ^ c ^ d, 0x6ed9eba1),
            2 => ((b & c) | (b & d) | (c & d), 0x8f1bbcdc),
            _ => (b ^ c ^ d, 0xca62c1d6),
        };
        let t = a
            .rotate_left(5)
            .wrapping_add(f)
            .wrapping_add(e)
            .wrapping_add(k)
            .wrapping_add(*wi);
        e = d;
        d = c;
        c = b.rotate_left(30);
        b = a;
        a = t;
    }
    for (s, v) in state.iter_mut().zip([a, b, c, d, e]) {
        *s = s.wrapping_add(v);
    }
}

/// SHA-1 of a slice.
pub fn sha1(data: &[u8]) -> Sha1Digest {
    let mut h = Sha1::new();
    h.update(data);
    h.finalize()
}

/// Length, SHA-256 and SHA-1 of one payload, computed in one pass (what a `CodeResources` seal needs per file).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct DualDigest {
    /// Length and SHA-256.
    pub file: FileDigest,
    /// SHA-1.
    pub sha1: Sha1Digest,
}

impl DualDigest {
    /// Of bytes in memory.
    pub fn of(bytes: &[u8]) -> DualDigest {
        let mut h = DualHasher::new();
        h.update(bytes);
        h.finish()
    }
}

/// SHA-1 and SHA-256 over the same bytes, fed once. Also a `Write` sink.
#[derive(Clone, Debug, Default)]
pub struct DualHasher {
    sha256: Sha256,
    sha1: Sha1,
}

impl DualHasher {
    /// A fresh dual hasher.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed bytes to both.
    pub fn update(&mut self, data: &[u8]) {
        self.sha256.update(data);
        self.sha1.update(data);
    }

    /// Finish both.
    pub fn finish(self) -> DualDigest {
        let len = self.sha256.len();
        DualDigest {
            file: FileDigest {
                len,
                sha256: self.sha256.finalize(),
            },
            sha1: self.sha1.finalize(),
        }
    }
}

impl Write for DualHasher {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.update(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nxb_testkit::check;

    fn with(b: Backend, data: &[u8]) -> Sha1Digest {
        let mut h = Sha1::with_backend(b);
        h.update(data);
        h.finalize()
    }

    #[test]
    fn fips_vectors_both_backends() {
        for b in [Backend::Auto, Backend::Scalar] {
            let v = |d: &[u8]| with(b, d).to_hex();
            assert_eq!(v(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
            assert_eq!(v(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
            assert_eq!(
                v(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
                "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
            );
            assert_eq!(
                v(&vec![b'a'; 1_000_000]),
                "34aa973cd4c4daa4f61eeb2bdbad27316534016f"
            );
        }
    }

    /// Every length 0..300 against Python `hashlib.sha1` (digest of the concatenated digests).
    #[test]
    fn all_lengths_against_hashlib_both_backends() {
        for b in [Backend::Auto, Backend::Scalar] {
            let mut acc = Vec::new();
            for n in 0..300usize {
                let data: Vec<u8> = (0..n).map(|i| ((i * 7 + 3) & 0xff) as u8).collect();
                acc.extend_from_slice(&with(b, &data).0);
            }
            assert_eq!(
                with(b, &acc).to_hex(),
                "ca9349a03c700fe25e6c2cde997aa96ac2227d8a",
                "{b:?}"
            );
        }
    }

    #[test]
    fn backends_agree_and_dual_matches_singles() {
        check(0x5eed_0003, 100, |r| {
            let n = r.below(5000) as usize;
            let data = r.bytes(n);
            let mut h = Sha1::new();
            let mut dual = DualHasher::new();
            for part in data.chunks(1 + r.below(200) as usize) {
                h.update(part);
                dual.update(part);
            }
            assert_eq!(h.finalize(), with(Backend::Scalar, &data));
            let d = dual.finish();
            assert_eq!(d, DualDigest::of(&data));
            assert_eq!(d.sha1, sha1(&data));
            assert_eq!(d.file, FileDigest::of(&data));
        });
    }
}

/// Throughput probe, not a gate: `cargo test --release -p nxb-model -- --ignored --nocapture sha1_throughput`.
#[cfg(test)]
mod throughput {
    use super::{Backend, Sha1};

    #[test]
    #[ignore = "timing probe; run in release"]
    fn sha1_throughput() {
        let data = vec![0x5au8; 1 << 30];
        let mut digests = Vec::new();
        for b in [Backend::Auto, Backend::Scalar] {
            let t = std::time::Instant::now();
            let mut h = Sha1::with_backend(b);
            h.update(&data);
            digests.push(h.finalize());
            let s = t.elapsed().as_secs_f64();
            eprintln!(
                "sha1 {b:?}: 1 GiB in {s:.2} s = {:.0} MB/s",
                1073.741824 / s
            );
        }
        assert_eq!(digests[0], digests[1]);
    }
}
