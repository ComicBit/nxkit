//! Robustness of the NSP, NCA and RomFS readers against damaged input.
//!
//! No game data and no external fuzzer. The seeds are small synthetic files. They are mutated in a deterministic way
//! (single byte changes, 32-bit words set to extreme values, cut-off tails, random garbage from a fixed seed).
//! A reader may return an error. It must not panic. The tests run with overflow checks on in the default test profile.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use nxb_extract::keys::KeySet;
use nxb_extract::nca::{open_nsp, read_pfs0};
use nxb_extract::romfs::read_tables;
use nxb_extract::storage::MemStorage;

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

/// Mutations of the first `window` bytes of `seed`, plus cut-off tails and random garbage.
fn mutations(seed: &[u8], window: usize) -> Vec<Vec<u8>> {
    let mut out = vec![seed.to_vec()];
    let w = window.min(seed.len());
    for i in 0..w {
        for v in [0u8, 1, 0x7f, 0x80, 0xff] {
            let mut m = seed.to_vec();
            m[i] = v;
            out.push(m);
        }
    }
    for i in (0..w.saturating_sub(3)).step_by(2) {
        for word in [0u32, 0xffff_ffff, 0x8000_0000, 0x7fff_ffff, 0x0100_0000] {
            let mut m = seed.to_vec();
            m[i..i + 4].copy_from_slice(&word.to_le_bytes());
            out.push(m);
        }
        // 64-bit fields: a large value in the low word and the high word
        let mut m = seed.to_vec();
        if i + 8 <= m.len() {
            m[i..i + 8].copy_from_slice(&u64::MAX.to_le_bytes());
            out.push(m);
        }
    }
    for n in 0..w {
        out.push(seed[..n].to_vec());
    }
    let mut rng = Rng(0x1234_5678_9ABC_DEF1);
    for _ in 0..300 {
        let mut m = seed.to_vec();
        for _ in 0..1 + rng.next() % 4 {
            let at = (rng.next() as usize) % w.max(1);
            if let Some(b) = m.get_mut(at) {
                *b = rng.next() as u8;
            }
        }
        out.push(m);
    }
    out
}

fn no_panic(name: &str, seed: &[u8], window: usize, f: impl Fn(&[u8])) {
    for (k, m) in mutations(seed, window).into_iter().enumerate() {
        let r = catch_unwind(AssertUnwindSafe(|| f(&m)));
        assert!(
            r.is_ok(),
            "{name}: panic on mutation {k}: {:02x?}",
            &m[..m.len().min(128)]
        );
    }
}

/// A PFS0 package with the given (name, data) files.
fn pfs0(files: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut strings = Vec::new();
    let mut name_offs = Vec::new();
    for (n, _) in files {
        name_offs.push(strings.len() as u32);
        strings.extend_from_slice(n.as_bytes());
        strings.push(0);
    }
    let mut out = b"PFS0".to_vec();
    out.extend_from_slice(&(files.len() as u32).to_le_bytes());
    out.extend_from_slice(&(strings.len() as u32).to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    let mut off = 0u64;
    for (i, (_, d)) in files.iter().enumerate() {
        out.extend_from_slice(&off.to_le_bytes());
        out.extend_from_slice(&(d.len() as u64).to_le_bytes());
        out.extend_from_slice(&name_offs[i].to_le_bytes());
        out.extend_from_slice(&[0; 4]);
        off += d.len() as u64;
    }
    out.extend_from_slice(&strings);
    for (_, d) in files {
        out.extend_from_slice(d);
    }
    out
}

fn sample_package() -> Vec<u8> {
    let body: Vec<u8> = (0..0x4400u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8)
        .collect();
    pfs0(&[
        ("a.nca", body.clone()),
        ("b.cnmt.nca", body[..0x600].to_vec()),
        ("c.tik", vec![7; 0x2c0]),
    ])
}

#[test]
fn pfs0_seed_parses() {
    let s = MemStorage(sample_package());
    let e = read_pfs0(&s).expect("seed must parse");
    assert_eq!(e.len(), 3);
    assert_eq!(e[1].name, "b.cnmt.nca");
}

#[test]
fn pfs0_mutations_do_not_panic() {
    let seed = sample_package();
    no_panic("read_pfs0", &seed, 160, |d| {
        let _ = read_pfs0(&MemStorage(d.to_vec()));
    });
}

#[test]
fn nsp_open_mutations_do_not_panic() {
    // A key file with a header key. The NCA headers decrypt to noise, so the readers must reject them cleanly.
    let keys = format!(
        "header_key = {}\n",
        "00112233445566778899aabbccddeeff".repeat(2)
    );
    let seed = sample_package();
    no_panic("open_nsp", &seed, 160, |d| {
        let mut ks = KeySet::parse(&keys);
        let _ = open_nsp(&mut ks, Arc::new(MemStorage(d.to_vec())));
    });
}

/// A RomFS with one directory table entry (the root) and one file `test.txt`.
fn sample_romfs() -> Vec<u8> {
    let none = u32::MAX;
    let mut dirs = Vec::new();
    for v in [0u32, none, none, 0, none, 0] {
        dirs.extend_from_slice(&v.to_le_bytes());
    }
    let mut files = Vec::new();
    for v in [0u32, none] {
        files.extend_from_slice(&v.to_le_bytes());
    }
    files.extend_from_slice(&0u64.to_le_bytes());
    files.extend_from_slice(&4u64.to_le_bytes());
    files.extend_from_slice(&none.to_le_bytes());
    files.extend_from_slice(&8u32.to_le_bytes());
    files.extend_from_slice(b"test.txt");
    let mut h = vec![0u8; 0x50];
    h[0..8].copy_from_slice(&0x50u64.to_le_bytes());
    let dm_off = 0x50u64;
    let fm_off = dm_off + dirs.len() as u64;
    let data = fm_off + files.len() as u64;
    h[0x18..0x20].copy_from_slice(&dm_off.to_le_bytes());
    h[0x20..0x28].copy_from_slice(&(dirs.len() as u64).to_le_bytes());
    h[0x38..0x40].copy_from_slice(&fm_off.to_le_bytes());
    h[0x40..0x48].copy_from_slice(&(files.len() as u64).to_le_bytes());
    h[0x48..0x50].copy_from_slice(&data.to_le_bytes());
    let mut out = h;
    out.extend_from_slice(&dirs);
    out.extend_from_slice(&files);
    out.extend_from_slice(b"data");
    out
}

#[test]
fn romfs_seed_parses() {
    let e = read_tables(&MemStorage(sample_romfs())).expect("seed must parse");
    assert_eq!(e.len(), 1);
}

#[test]
fn romfs_mutations_do_not_panic() {
    let seed = sample_romfs();
    no_panic("read_tables", &seed, seed.len(), |d| {
        let _ = read_tables(&MemStorage(d.to_vec()));
    });
}
