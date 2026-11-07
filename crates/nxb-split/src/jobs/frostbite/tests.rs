use super::*;
use crate::jobs::own;
use crate::zstd::Dicts;

fn leb(mut n: usize) -> Vec<u8> {
    let mut v = Vec::new();
    loop {
        let b = (n & 0x7f) as u8;
        n >>= 7;
        if n == 0 {
            v.push(b);
            return v;
        }
        v.push(b | 0x80);
    }
}

/// A named DbObject field.
fn field(name: &str, kind: u8, payload: &[u8]) -> Vec<u8> {
    let mut v = vec![kind];
    v.extend_from_slice(name.as_bytes());
    v.push(0);
    v.extend_from_slice(payload);
    v
}

/// The payload of a list or object.
fn items(parts: &[Vec<u8>]) -> Vec<u8> {
    let body: Vec<u8> = parts.iter().flatten().copied().chain([0]).collect();
    let mut v = leb(body.len());
    v.extend(body);
    v
}

fn string(s: &str) -> Vec<u8> {
    let mut v = leb(s.len() + 1);
    v.extend_from_slice(s.as_bytes());
    v.push(0);
    v
}

fn layout() -> Vec<u8> {
    let chunk = |i: i32, n: &str| {
        let mut v = vec![0x82];
        v.extend(items(&[
            field("persistentIndex", 8, &i.to_le_bytes()),
            field("name", 7, &string(n)),
        ]));
        v
    };
    let im = field(
        "installManifest",
        2,
        &items(&[field("installChunks", 1, &items(&[chunk(7, "chunk7")]))]),
    );
    let mut d = TOC_MAGIC.to_vec();
    d.resize(TOC_BODY, 0);
    d.push(0x82);
    d.extend(items(&[
        field("ok", 6, &[1]),
        field("f", 0xb, &1.5f32.to_le_bytes()),
        im,
    ]));
    d
}

fn cid(pi: u32, n: u16) -> [u8; 8] {
    let mut c = [0u8; 8];
    c[2..6].copy_from_slice(&pi.to_be_bytes());
    c[6..8].copy_from_slice(&n.to_be_bytes());
    c
}

/// A toc with one bundle of `ents`.
fn toc(ents: &[Ent]) -> Vec<u8> {
    let mut d = TOC_MAGIC.to_vec();
    d.resize(TOC_BODY, 0);
    let b = d.len();
    d.resize(b + 0x20, 0);
    d[b + 4..b + 8].copy_from_slice(&0x20u32.to_be_bytes());
    d[b + 8..b + 12].copy_from_slice(&1u32.to_be_bytes());
    let mut ent = Vec::new();
    let mut flags = Vec::new();
    let mut last = None;
    for (c, o, s) in ents {
        if last != Some(*c) {
            ent.extend_from_slice(c);
            flags.push(0x80);
            last = Some(*c);
        } else {
            flags.push(0);
        }
        ent.extend_from_slice(&o.to_be_bytes());
        ent.extend_from_slice(&s.to_be_bytes());
    }
    let fo = 0x14 + ent.len();
    let mut s = vec![0u8; 0x14];
    s[8..12].copy_from_slice(&(fo as u32).to_be_bytes());
    s[12..16].copy_from_slice(&(ents.len() as u32).to_be_bytes());
    s[16..20].copy_from_slice(&0x14u32.to_be_bytes());
    s.extend(ent);
    s.extend(flags);
    let mut rec = vec![0u8; 16];
    rec[4..8].copy_from_slice(&(s.len() as u32).to_be_bytes());
    rec[8..16].copy_from_slice(&0x30u64.to_be_bytes());
    d.extend(rec);
    d.extend(s);
    d
}

/// A bundle manifest with one ebx and the given res (name, size, type).
fn bundle(res: &[(&str, u32, u32)]) -> Vec<u8> {
    let (ne, nr) = (1usize, res.len());
    let tot = ne + nr;
    let mut names = Vec::new();
    let mut offs = Vec::new();
    for (n, _, _) in res {
        offs.push(names.len() as u32);
        names.extend_from_slice(n.as_bytes());
        names.push(0);
    }
    let mut h = vec![0u8; 0x20 + 20 * tot];
    h.extend([0u8; 8]);
    for ((_, s, _), o) in res.iter().zip(&offs) {
        h.extend_from_slice(&o.to_le_bytes());
        h.extend_from_slice(&s.to_le_bytes());
    }
    for (_, _, t) in res {
        h.extend_from_slice(&t.to_le_bytes());
    }
    h.extend(vec![0u8; 24 * nr]);
    let so = h.len();
    h.extend(names);
    for (i, v) in [BUNDLE_MAGIC, tot as u32, ne as u32, nr as u32, 0, so as u32]
        .into_iter()
        .enumerate()
    {
        h[4 * i..4 * i + 4].copy_from_slice(&v.to_le_bytes());
    }
    let mut d = (h.len() as u32).to_be_bytes().to_vec();
    d.extend(h);
    d
}

/// One stored block.
fn block(b: &[u8]) -> Vec<u8> {
    let mut d = (b.len() as u32).to_be_bytes().to_vec();
    d.push(0);
    d.push(0x70 | (b.len() >> 16) as u8);
    d.extend_from_slice(&(b.len() as u16).to_be_bytes());
    d.extend_from_slice(b);
    d
}

/// A CompiledBytecode resource with a VNVN record of `code` and `ctl`.
fn vnvn(code: &[u8], ctl: &[u8]) -> Vec<u8> {
    let mut b = vec![0u8; 0x20];
    b.extend_from_slice(b"VNVN");
    b.extend([0u8; 4]);
    b.extend(3u16.to_le_bytes());
    b.resize(0x31, 0);
    b.extend([0u8; 10]);
    let (co, ko) = (0x40u32, 0x40 + code.len() as u32);
    for v in [co, code.len() as u32, ko, ctl.len() as u32] {
        b.extend(v.to_le_bytes());
    }
    b.resize(0x60, 0);
    b.extend_from_slice(code);
    b.extend_from_slice(ctl);
    b
}

#[test]
fn hash_of_compiled_bytecode() {
    assert_eq!(name_hash("CompiledBytecode"), 0x0e2b_02f7);
}

#[test]
fn reads_install_chunks() {
    let c = install_chunks(&layout()).ok().expect("layout");
    assert_eq!(c.get(&7).map(String::as_str), Some("chunk7"));
    assert!(install_chunks(&TOC_MAGIC).is_err());
}

#[test]
fn reads_toc_bundles() {
    let (a, b) = (cid(7, 1), cid(7, 2));
    let ents = vec![(a, 0, 10), (a, 10, 5), (b, 0, 3)];
    let got = toc_bundles(&toc(&ents), "t").ok().expect("toc");
    assert_eq!(got, vec![ents]);
    assert!(toc_bundles(b"nope", "t").is_err());
    assert_eq!(
        cas_path(&cid(7, 3), &install_chunks(&layout()).ok().expect("l")).ok(),
        Some("Data/NX/chunk7/cas_03.cas".into())
    );
}

#[test]
fn reads_stored_blocks() {
    let d: Vec<u8> = [block(b"abc"), block(b"de")].concat();
    assert_eq!(read_item(&d, "x", 5).ok(), Some(b"abcde".to_vec()));
    assert!(read_item(&d, "x", 4).is_err());
    let mut bad = block(b"abc");
    bad[5] = 0x60;
    assert!(read_item(&bad, "x", 3).is_err());
}

#[test]
fn reads_vnvn_records() {
    let b = vnvn(b"CODE", b"CONTROL");
    let got = nvn_shader(&b).ok().flatten();
    assert_eq!(got, Some((&b"CODE"[..], &b"CONTROL"[..])));
    assert_eq!(nvn_shader(&[0u8; 0x40]).ok(), Some(None));
    let mut short = b.clone();
    short.truncate(0x62);
    assert!(nvn_shader(&short).is_err());
}

#[test]
fn plans_and_reads_the_resources() {
    let rt = name_hash("CompiledBytecode");
    let blob = vnvn(b"CODE", b"CONTROL");
    let man = bundle(&[("shader/a", blob.len() as u32, rt), ("tex", 1, 5)]);
    let item = block(&blob);
    let mut cas = man.clone();
    let at = cas.len() as u32;
    cas.extend(&item);
    cas.extend(block(b"x"));
    let c = cid(7, 1);
    let ents = vec![
        (c, 0, man.len() as u32),
        (c, 0, 0),
        (c, at, item.len() as u32),
        (c, at + item.len() as u32, 9),
    ];
    let files = vec![
        File {
            rel: "Data/layout.toc".into(),
            src: own(layout()),
            live: true,
        },
        File {
            rel: "Data/NX/chunk7/a.toc".into(),
            src: own(toc(&ents)),
            live: true,
        },
        File {
            rel: "Data/NX/chunk7/cas_01.cas".into(),
            src: own(cas),
            live: true,
        },
        File {
            rel: "other.bin".into(),
            src: own(vec![1]),
            live: false,
        },
    ];
    let mut p = Plan::default();
    plan(&files, &mut p).ok().expect("plan");
    assert_eq!(p.jobs.len(), 1);
    assert_eq!(p.jobs[0].rel, "Data/NX/chunk7/cas_01.cas");
    assert_eq!(p.taken, [0, 1, 2].into_iter().collect());
    let dicts = Dicts::default();
    let ctx = Ctx {
        files: &files,
        dicts: &dicts,
    };
    let mut notes = Vec::new();
    let out = (p.jobs[0].run)(&ctx, &mut notes).ok().expect("run");
    assert!(out.found.is_empty());
    assert_eq!(notes.len(), 1);
    assert!(
        notes[0].starts_with("[RED] /Data/NX/chunk7/cas_01.cas#shader/a: "),
        "{notes:?}"
    );
}

#[test]
fn skips_roots_without_the_magic() {
    let files = vec![File {
        rel: "Data/layout.toc".into(),
        src: own(b"something".to_vec()),
        live: true,
    }];
    let mut p = Plan::default();
    plan(&files, &mut p).ok().expect("plan");
    assert!(p.jobs.is_empty() && p.taken.is_empty());
}
