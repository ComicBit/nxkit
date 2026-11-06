//! The engine's template helpers, recognised by the hash of their whitespace-normalised text.
//!
//! The prototype joins the texts of a template item's tokens with single spaces and takes the
//! first 16 hex digits of the SHA-1 of that string (`text_hash`). `prelude.json` maps those
//! hashes to helper names. Only the helpers in [`ALLOWED`] have a type rule in the checker:
//! a template with another known hash, or an unknown hash, rejects the item.

use msl_lex::Token;

/// hash -> helper name, the content of the prototype's `prelude.json`.
pub const PRELUDE: [(&str, &str); 20] = [
    ("5320f275455261ce", "dm_bs"),
    ("41926c7574d941ba", "spvFAdd"),
    ("001ab26fd933eecb", "mod"),
    ("8dbe309a5c5e6b8e", "spvUnsafeArray"),
    ("91abe1dcaf48d0d7", "spvDepthCast"),
    ("1d7d7ff7867ab45d", "spvDepthCast"),
    ("ba7c4149921fc1bc", "spvDepthCast"),
    ("1d13ca3cd093fc07", "spvDepthCast"),
    ("722b963bb859d9c7", "decltype"),
    ("9c30b35ed7b9843e", "spvGatherConstOffsets"),
    ("88e2fee312dd992e", "spvGatherConstOffsets"),
    ("23d3816f711320e1", "spvGatherConstOffsets"),
    ("3e45b4f9c7c50ea3", "spvFindUMSB"),
    ("acf86140e0548211", "spvSubgroupShuffle"),
    ("9a8723f89c0ed1b4", "spvSubgroupShuffle"),
    ("ee963ed3033b22e9", "spvSubgroupShuffle"),
    ("00a964ba6150992d", "spvSubgroupShuffle"),
    ("39d7fef78d7770be", "spvSubgroupShuffle"),
    ("5bf10a85e0d35b52", "spvSubgroupShuffle"),
    ("d0b72973eeaa2916", "dm_pool_mark"),
];

/// Helpers with a type rule (`PRELUDE_ALLOWED`).
pub const ALLOWED: [&str; 5] = [
    "dm_bs",
    "dm_pool_mark",
    "spvDepthCast",
    "spvFAdd",
    "spvUnsafeArray",
];

/// The helper name for a hash in the built-in table.
pub fn lookup(hash: &str) -> Option<&'static str> {
    PRELUDE.iter().find(|(h, _)| *h == hash).map(|(_, n)| *n)
}

/// A hash -> helper name table that replaces or extends the built-in one, for tests and for
/// new engine helper texts (the prototype regenerates `prelude.json` with `--make-prelude`).
#[derive(Clone, Debug, Default)]
pub struct Table {
    entries: Vec<(String, String)>,
}

impl Table {
    /// The built-in table.
    pub fn builtin() -> Table {
        Table {
            entries: PRELUDE
                .iter()
                .map(|(h, n)| (h.to_string(), n.to_string()))
                .collect(),
        }
    }

    pub fn empty() -> Table {
        Table::default()
    }

    /// Adds (or overrides) one entry.
    pub fn with(mut self, hash: &str, name: &str) -> Table {
        self.entries.retain(|(h, _)| h != hash);
        self.entries.push((hash.to_string(), name.to_string()));
        self
    }

    pub fn lookup(&self, hash: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(h, _)| h == hash)
            .map(|(_, n)| n.as_str())
    }
}

pub fn is_allowed(name: &str) -> bool {
    ALLOWED.contains(&name)
}

/// The static name of an allowed helper.
pub fn allowed(name: &str) -> Option<&'static str> {
    ALLOWED.iter().copied().find(|a| *a == name)
}

/// `text_hash(toks)`: first 16 hex digits of SHA-1 over the token texts joined by spaces.
pub fn text_hash(toks: &[Token<'_>]) -> String {
    let mut h = Sha1::new();
    for (i, t) in toks.iter().enumerate() {
        if i > 0 {
            h.update(b" ");
        }
        h.update(t.text.as_bytes());
    }
    let d = h.finish();
    let mut s = String::with_capacity(16);
    for b in &d[..8] {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// A small SHA-1 (FIPS 180-4). Only used to identify helper texts, not for security.
pub struct Sha1 {
    state: [u32; 5],
    buf: [u8; 64],
    buf_len: usize,
    total: u64,
}

impl Default for Sha1 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha1 {
    pub fn new() -> Sha1 {
        Sha1 {
            state: [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0],
            buf: [0; 64],
            buf_len: 0,
            total: 0,
        }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.total += data.len() as u64;
        while !data.is_empty() {
            let take = (64 - self.buf_len).min(data.len());
            self.buf[self.buf_len..self.buf_len + take].copy_from_slice(&data[..take]);
            self.buf_len += take;
            data = &data[take..];
            if self.buf_len == 64 {
                let block = self.buf;
                self.compress(&block);
                self.buf_len = 0;
            }
        }
    }

    pub fn finish(mut self) -> [u8; 20] {
        let bits = self.total.wrapping_mul(8);
        self.update(&[0x80]);
        while self.buf_len != 56 {
            self.update(&[0]);
        }
        self.update(&bits.to_be_bytes());
        let mut out = [0u8; 20];
        for (i, w) in self.state.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&w.to_be_bytes());
        }
        out
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = self.state;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
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
        for (s, x) in self.state.iter_mut().zip([a, b, c, d, e]) {
            *s = s.wrapping_add(x);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(d: [u8; 20]) -> String {
        d.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn sha1_vectors() {
        let mut h = Sha1::new();
        h.update(b"abc");
        assert_eq!(hex(h.finish()), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            hex(Sha1::new().finish()),
            "da39a3ee5e6b4b0d3255bfef95601890afd80709"
        );
        // Feeding the input in pieces must not change the digest.
        let mut chunked = Sha1::new();
        for _ in 0..1000 {
            chunked.update(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq");
        }
        let mut one = Sha1::new();
        one.update(&b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq".repeat(1000));
        assert_eq!(hex(chunked.finish()), hex(one.finish()));
        let mut h = Sha1::new();
        h.update(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq");
        assert_eq!(hex(h.finish()), "84983e441c3bd26ebaae4aa1f95129e5e54670f1");
    }

    #[test]
    fn table() {
        assert_eq!(lookup("5320f275455261ce"), Some("dm_bs"));
        assert_eq!(lookup("d0b72973eeaa2916"), Some("dm_pool_mark"));
        assert!(is_allowed("dm_pool_mark"));
        assert_eq!(lookup("0000000000000000"), None);
        assert!(is_allowed("spvFAdd"));
        assert!(!is_allowed("mod"));
    }
}
