use super::*;
use crate::jobs::own;
use crate::oodle::{Answer, Decoder, set_local_decoder};
use crate::rd::u64le;

/// How the fixture stores a chunk's blocks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::jobs) enum M {
    Stored,
    Zlib,
    Lz4,
    Oodle,
}

/// A synthetic IoStore container.
pub(in crate::jobs) struct Fix {
    pub(in crate::jobs) version: u8,
    pub(in crate::jobs) block: usize,
    /// Two partitions of this size, else one.
    pub(in crate::jobs) part: Option<u64>,
    pub(in crate::jobs) encrypted: bool,
    pub(in crate::jobs) dir: Vec<u8>,
    pub(in crate::jobs) chunks: Vec<([u8; 12], Vec<u8>, M)>,
}

impl Fix {
    pub(in crate::jobs) fn new(chunks: Vec<([u8; 12], Vec<u8>, M)>) -> Fix {
        Fix {
            version: 5,
            block: 64,
            part: None,
            encrypted: false,
            dir: Vec::new(),
            chunks,
        }
    }
}

pub(in crate::jobs) fn id(n: u8, t: u8) -> [u8; 12] {
    let mut x = [n; 12];
    x[11] = t;
    x
}

fn adler(d: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in d {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    b << 16 | a
}

/// A zlib stream of one stored deflate block.
fn zlib(d: &[u8]) -> Vec<u8> {
    let n = d.len() as u16;
    let mut o = vec![0x78, 0x01, 1];
    o.extend(n.to_le_bytes());
    o.extend((!n).to_le_bytes());
    o.extend(d);
    o.extend(adler(d).to_be_bytes());
    o
}

/// An LZ4 block of literals only.
fn lz4(d: &[u8]) -> Vec<u8> {
    let mut o = Vec::new();
    if d.len() < 15 {
        o.push((d.len() as u8) << 4);
    } else {
        o.push(0xf0);
        let mut r = d.len() - 15;
        while r >= 255 {
            o.push(255);
            r -= 255;
        }
        o.push(r as u8);
    }
    o.extend(d);
    o
}

/// The fake game decoder: `0xAA` and the bytes reversed, or `0xBB` and a pattern repeated to the raw length.
pub(in crate::jobs) struct Fake;

impl Decoder for Fake {
    fn ask(&self, comp: &[u8], n: usize) -> Answer {
        match comp.split_first() {
            Some((0xaa, r)) if r.len() == n => Answer::Bytes(r.iter().rev().copied().collect()),
            Some((0xbb, r)) if !r.is_empty() => {
                Answer::Bytes(r.iter().cycle().take(n).copied().collect())
            }
            _ => Answer::Refused,
        }
    }
    fn said(&self) -> String {
        "refused".into()
    }
}

pub(in crate::jobs) fn with_fake() {
    set_local_decoder(Some(Some(std::sync::Arc::new(Fake))));
}

fn put(o: &mut Vec<u8>, v: u64, n: usize, big: bool) {
    let b = v.to_le_bytes();
    if big {
        o.extend(b[..n].iter().rev());
    } else {
        o.extend(&b[..n]);
    }
}

/// The TOC and the `.ucas` partitions of `f`.
pub(in crate::jobs) fn build(f: &Fix) -> (Vec<u8>, Vec<Vec<u8>>) {
    let (mut parts, mut blocks, mut ranges) = (vec![Vec::new()], Vec::new(), Vec::new());
    for (_, data, m) in &f.chunks {
        ranges.push(((blocks.len() * f.block) as u64, data.len() as u64));
        for piece in data.chunks(f.block) {
            let (c, meth) = match m {
                M::Stored => (piece.to_vec(), 0u8),
                M::Zlib => (zlib(piece), 1),
                M::Lz4 => (lz4(piece), 2),
                M::Oodle => (
                    [
                        &[0xaa][..],
                        &piece.iter().rev().copied().collect::<Vec<_>>(),
                    ]
                    .concat(),
                    3,
                ),
            };
            let padded = (c.len() + 15) & !15;
            if let Some(p) = f.part
                && (parts[parts.len() - 1].len() + padded) as u64 > p
            {
                parts.push(Vec::new());
            }
            let k = parts.len() - 1;
            let at = k as u64 * f.part.unwrap_or(0) + parts[k].len() as u64;
            blocks.push((at, c.len() as u64, piece.len() as u64, meth));
            let end = parts[k].len() + padded;
            parts[k].extend(&c);
            parts[k].resize(end, 0);
        }
    }
    let flags = if f.encrypted { 2 } else { 0 } | if f.dir.is_empty() { 0 } else { 8 };
    let mut b = TOC_MAGIC.to_vec();
    b.extend([f.version, 0, 0, 0]);
    let n = f.chunks.len();
    for w in [
        144,
        n,
        blocks.len(),
        12,
        3,
        32,
        f.block,
        f.dir.len(),
        parts.len(),
    ] {
        b.extend((w as u32).to_le_bytes());
    }
    b.resize(80, 0);
    b.push(flags);
    b.resize(84, 0);
    b.extend(0u32.to_le_bytes());
    b.extend(f.part.unwrap_or(0).to_le_bytes());
    b.extend(0u32.to_le_bytes());
    b.resize(144, 0);
    for (i, _, _) in &f.chunks {
        b.extend(i);
    }
    for (o, l) in ranges {
        put(&mut b, o, 5, true);
        put(&mut b, l, 5, true);
    }
    for (at, size, raw, m) in blocks {
        put(&mut b, at, 5, false);
        put(&mut b, size, 3, false);
        put(&mut b, raw, 3, false);
        b.push(m);
    }
    for m in ["Zlib", "LZ4", "Oodle"] {
        let mut x = m.as_bytes().to_vec();
        x.resize(32, 0);
        b.extend(x);
    }
    b.extend(&f.dir);
    (b, parts)
}

/// The files of the container `<stem>.utoc`: the TOC and its partitions.
pub(in crate::jobs) fn files(stem: &str, f: &Fix) -> Vec<(String, Arc<dyn Src>)> {
    let (toc, parts) = build(f);
    let mut out = vec![(format!("{stem}.utoc"), own(toc))];
    for (k, p) in parts.into_iter().enumerate() {
        let s = if k == 0 {
            String::new()
        } else {
            format!("_s{k}")
        };
        out.push((format!("{stem}{s}.ucas"), own(p)));
    }
    out
}

fn opened(stem: &str, f: &Fix) -> (Toc, Vec<Option<Arc<dyn Src>>>) {
    let fs = files(stem, f);
    let toc = Toc::load(fs[0].1.as_ref(), &fs[0].0).expect("toc");
    let ucas = fs[1..].iter().map(|x| Some(x.1.clone())).collect();
    (toc, ucas)
}

fn fstr(o: &mut Vec<u8>, s: &str) {
    o.extend((s.len() as i32 + 1).to_le_bytes());
    o.extend(s.as_bytes());
    o.push(0);
}

/// Engine/a.uasset (chunk 0), Engine/b.uasset (chunk 2), Game/b.uasset (chunk 1).
fn dir_index() -> Vec<u8> {
    let no = u32::MAX;
    let mut o = Vec::new();
    fstr(&mut o, "../../../");
    o.extend(3i32.to_le_bytes());
    for d in [[no, 1, no, no], [0, no, 2, 0], [1, no, no, 1]] {
        d.iter().for_each(|w| o.extend(w.to_le_bytes()));
    }
    o.extend(3i32.to_le_bytes());
    for f in [[2, 2, 0], [3, no, 1], [3, no, 2]] {
        f.iter().for_each(|w| o.extend(w.to_le_bytes()));
    }
    o.extend(4i32.to_le_bytes());
    for s in ["Engine", "Game", "a.uasset", "b.uasset"] {
        fstr(&mut o, s);
    }
    o
}

fn data(n: usize, seed: u8) -> Vec<u8> {
    (0..n)
        .map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

fn mixed() -> Fix {
    let mut f = Fix::new(vec![
        (id(1, 1), data(40, 1), M::Stored),
        (id(2, 1), data(20, 2), M::Zlib),
        (id(3, 2), data(33, 3), M::Lz4),
        (id(4, 3), data(10, 4), M::Oodle),
        (id(5, 3), Vec::new(), M::Stored),
    ]);
    f.block = 16;
    f.part = Some(128);
    f.dir = dir_index();
    f
}

#[test]
fn reads_every_block_kind_over_two_partitions() {
    with_fake();
    let f = mixed();
    let fs = files("Game/Content/Paks/pakchunk0", &f);
    let names: Vec<&str> = fs.iter().map(|x| x.0.as_str()).collect();
    assert_eq!(
        names,
        [
            "Game/Content/Paks/pakchunk0.utoc",
            "Game/Content/Paks/pakchunk0.ucas",
            "Game/Content/Paks/pakchunk0_s1.ucas"
        ]
    );
    let (toc, ucas) = opened("Game/Content/Paks/pakchunk0", &f);
    assert_eq!(toc.ucas_paths(), names[1..]);
    for (i, (_, d, _)) in f.chunks.iter().enumerate() {
        assert_eq!(toc.read(i, &ucas).expect("read"), *d, "chunk {i}");
    }
    let names = toc.names().expect("names");
    assert_eq!(names.len(), 3);
    assert_eq!(names[&0], "Engine/a.uasset");
    assert_eq!(names[&2], "Engine/b.uasset");
    assert_eq!(names[&1], "Game/b.uasset");
    let mut looped = mixed();
    // folder 1 (at 18 + 16) is its own next folder
    looped.dir[42..46].copy_from_slice(&1u32.to_le_bytes());
    let (t, _) = opened("l", &looped);
    assert!(t.names().is_err());
    assert_eq!(toc.export_bundles(), [0, 1]);
    assert_eq!(toc.chunks_of(3), [3, 4]);
    let mut v3 = mixed();
    v3.version = 3;
    let (t3, _) = opened("a", &v3);
    assert_eq!(t3.export_bundles(), [2]);
    assert!(!t3.of_type(1) && t3.chunks_of(1).is_empty());
    set_local_decoder(None);
}

#[test]
fn container_order_and_folders() {
    let mut v = [
        "P/x_2_P.utoc",
        "P/x_P.utoc",
        "P/x.utoc",
        "P/x3_P.utoc",
        "P/x_10_P.utoc",
    ]
    .map(String::from)
    .to_vec();
    v.sort_by_cached_key(|r| mount_order(r));
    assert_eq!(
        v,
        [
            "P/x.utoc",
            "P/x3_P.utoc",
            "P/x_P.utoc",
            "P/x_2_P.utoc",
            "P/x_10_P.utoc"
        ]
    );
    assert_eq!(dirname("a/b/c.utoc"), "a/b");
    assert_eq!(dirname("c.utoc"), "");
    let exports = build(&Fix::new(vec![(id(1, 1), data(4, 0), M::Stored)])).0;
    let libs = build(&Fix::new(vec![(
        id(2, IO_SHADER_LIBRARY),
        data(4, 0),
        M::Stored,
    )]))
    .0;
    let e: &dyn Src = &crate::src::Mem(&exports);
    let l: &dyn Src = &crate::src::Mem(&libs);
    assert!(inline_in_containers([("a.utoc", e)]));
    assert!(!inline_in_containers([("a.utoc", e), ("b.utoc", l)]));
    assert!(!inline_in_containers([("a.utoc", l)]));
    assert!(!inline_in_containers([(
        "a.utoc",
        &crate::src::Mem(b"junk") as &dyn Src
    )]));
}

#[test]
fn encrypted_container_needs_the_key() {
    let hex = |s: &str| -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .filter_map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
            .collect()
    };
    // SP 800-38A F.1.6: ECB-AES256 ciphertext and plaintext
    let key: [u8; 32] = hex("603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4")
        .try_into()
        .expect("key");
    let c = hex("f3eed1bdb5d2a03c064b5a7e3db181f8591ccb10d410ed26dc5ba74a31362870");
    let p = hex("6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e51");
    let mut f = Fix::new(vec![(id(1, 1), c, M::Stored)]);
    f.encrypted = true;
    f.dir = dir_index();
    let (mut toc, ucas) = opened("e", &f);
    assert!(toc.encrypted);
    assert!(toc.names().expect("names").is_empty());
    let e = toc
        .read(0, &ucas)
        .err()
        .map(|e| e.to_string())
        .unwrap_or_default();
    assert!(e.contains("encrypted and no key"), "{e}");
    toc.set_key(Some(key));
    assert_eq!(toc.read(0, &ucas).expect("read"), p);
    let short: Vec<Option<Arc<dyn Src>>> = vec![Some(own(ucas[0]
        .as_ref()
        .map(|s| s.all())
        .expect("s")
        .expect("all")[..20]
        .to_vec()))];
    assert!(toc.read(0, &short).is_err());
}

fn err<T>(r: R<T>) -> String {
    match r {
        Ok(_) => panic!("no error"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn damaged_containers_are_errors() {
    with_fake();
    let f = mixed();
    let (b, parts) = build(&f);
    assert!(err(Toc::open(&b[..10], "t")).contains("not an IoStore TOC"));
    assert!(err(Toc::open(&b[..16], "t")).contains("header ends early"));
    assert!(Toc::open(&b[..60], "t").is_err());
    let mut m = b.clone();
    m[0] = b'+';
    assert!(err(Toc::open(&m, "t")).contains("not an IoStore TOC"));
    let mut m = b.clone();
    m[24..28].copy_from_slice(&0x0100_0000u32.to_le_bytes());
    assert!(err(Toc::open(&m, "t")).contains("ends before its tables"));
    assert!(
        err(Toc::open(&b[..b.len() - f.dir.len() - 1], "t")).contains("ends before its tables")
    );
    let ucas: Vec<Option<Arc<dyn Src>>> = parts.iter().map(|p| Some(own(p.clone()))).collect();
    // chunk 0 starts at block 100, past the 9 blocks
    let mut m = b.clone();
    m[144 + 60..144 + 65].copy_from_slice(&[0, 0, 0, 6, 64]);
    let t = Toc::open(&m, "t").expect("toc");
    assert!(err(t.read(0, &ucas)).contains("IndexError"));
    assert!(err(t.read(99, &ucas)).contains("IndexError"));
    let t = Toc::open(&b, "t").expect("toc");
    let one = vec![ucas[0].clone(), None];
    assert!(err(t.read(3, &one)).contains("FileNotFoundError"));
    assert!(err(t.read(0, &[])).contains("FileNotFoundError"));
    // the checksum of chunk 1's first zlib block (block 3), then the method name
    let at = 144 + 22 * 5;
    let z3 = u64le(&b, at + 36).expect("block") & 0xff_ffff_ffff;
    let (k, local) = ((z3 / 128) as usize, (z3 % 128) as usize);
    let mut bad = ucas.clone();
    let mut z = parts[k].clone();
    z[local + 2 + 5 + 16] ^= 1;
    bad[k] = Some(own(z));
    assert!(err(t.read(1, &bad)).contains("zlib.error"));
    let mut m = b.clone();
    let names = at + 12 * 9;
    m[names..names + 4].copy_from_slice(b"Zstd");
    let t = Toc::open(&m, "t").expect("toc");
    assert!(err(t.read(1, &ucas)).contains("compression method Zstd"));
    let mut m = b.clone();
    m[44..48].copy_from_slice(&0u32.to_le_bytes());
    let t = Toc::open(&m, "t").expect("toc");
    assert!(err(t.read(0, &ucas)).contains("size of 0"));
    set_local_decoder(None);
}

#[test]
fn cut_or_flipped_containers_never_panic() {
    with_fake();
    let (b, parts) = build(&mixed());
    let ucas: Vec<Option<Arc<dyn Src>>> = parts.iter().map(|p| Some(own(p.clone()))).collect();
    let probe = |b: &[u8]| {
        if let Ok(t) = Toc::open(b, "t") {
            let _ = t.names();
            for i in 0..t.ids.len() {
                let _ = t.read(i, &ucas);
            }
        }
    };
    for k in 0..b.len() {
        probe(&b[..k]);
    }
    for k in 16..b.len() {
        for bit in [0x01u8, 0x80] {
            let mut m = b.clone();
            m[k] ^= bit;
            probe(&m);
        }
    }
    set_local_decoder(None);
}
