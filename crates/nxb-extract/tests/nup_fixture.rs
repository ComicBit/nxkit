//! NUP reading on small synthetic containers built here (no game data): layout, image contents, control data, DLC,
//! a standalone update, and the checks on damaged or hostile containers.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use nxb_extract::nup::{NupRequest, extract_nup};

/// One tree node: a folder with children or a file with content (stored raw, or as a zstd frame when `z`).
enum Node {
    Dir(&'static str, Vec<Node>),
    File(&'static str, Vec<u8>, bool),
}

struct Pkg {
    title: u64,
    kind: u8,
    version: u32,
    flags: u8,
    exefs: Option<Vec<Node>>,
    romfs: Option<Vec<Node>>,
    meta: Vec<(u16, Vec<u8>)>,
}

/// A zstd frame of one raw block (what a packer writes for incompressible data).
fn zstd_raw(data: &[u8]) -> Vec<u8> {
    let mut f = vec![0x28, 0xb5, 0x2f, 0xfd, 0xe0];
    f.extend_from_slice(&(data.len() as u64).to_le_bytes());
    let h = ((data.len() as u32) << 3) | 1;
    f.extend_from_slice(&h.to_le_bytes()[..3]);
    f.extend_from_slice(data);
    f
}

fn npdm(program: u64) -> Vec<u8> {
    let mut d = vec![0u8; 0x200];
    d[..4].copy_from_slice(b"META");
    d[0x70..0x74].copy_from_slice(&0x100u32.to_le_bytes());
    d[0x74..0x78].copy_from_slice(&0x40u32.to_le_bytes());
    d[0x100..0x104].copy_from_slice(b"ACI0");
    d[0x110..0x118].copy_from_slice(&program.to_le_bytes());
    d
}

#[derive(Default)]
struct Builder {
    dirs: Vec<(Vec<u8>, Vec<u32>)>,
    files: Vec<(Vec<u8>, Vec<u8>, bool)>,
}

impl Builder {
    /// Adds a folder and returns its index, children are slot values (folders: index, files: dirs + index, fixed later).
    fn add(&mut self, name: &str, kids: &[Node]) -> u32 {
        let me = self.dirs.len() as u32;
        self.dirs.push((name.as_bytes().to_vec(), Vec::new()));
        let mut slots = Vec::new();
        for k in kids {
            match k {
                Node::Dir(n, c) => slots.push(self.add(n, c)),
                Node::File(n, d, z) => {
                    self.files.push((n.as_bytes().to_vec(), d.clone(), *z));
                    slots.push(0x8000_0000 | (self.files.len() - 1) as u32);
                }
            }
        }
        self.dirs[me as usize].1 = slots;
        me
    }
}

fn build(pkgs: &[Pkg], corrupt_toc: bool) -> Vec<u8> {
    let mut b = Builder::default();
    let mut recs = Vec::new();
    for p in pkgs {
        let ex = p.exefs.as_ref().map(|k| b.add("exefs", k));
        let ro = p.romfs.as_ref().map(|k| b.add("romfs", k));
        let root = b.add("root", &[]);
        let mut slots = Vec::new();
        slots.extend(ex);
        slots.extend(ro);
        b.dirs[root as usize].1 = slots;
        recs.push((root, ex.unwrap_or(u32::MAX), ro.unwrap_or(u32::MAX)));
    }
    let nd = b.dirs.len() as u32;
    let mut pool = Vec::new();
    let mut name = |n: &[u8]| {
        let o = pool.len() as u32;
        pool.extend_from_slice(n);
        (o, n.len() as u32)
    };
    let names_d: Vec<(u32, u32)> = b.dirs.iter().map(|d| name(&d.0)).collect();
    let names_f: Vec<(u32, u32)> = b.files.iter().map(|f| name(&f.0)).collect();
    let mut slots = Vec::new();
    let mut firsts = Vec::new();
    for d in &b.dirs {
        firsts.push((slots.len() as u32, d.1.len() as u32));
        for &s in &d.1 {
            slots.push(if s & 0x8000_0000 != 0 {
                nd + (s & 0x7fff_ffff)
            } else {
                s
            });
        }
    }
    let toc_len_fixed =
        0x30 + pkgs.len() * 0x40 + b.dirs.len() * 0x18 + b.files.len() * 0x50 + slots.len() * 4;
    let pool_off = toc_len_fixed;
    let toc_size = pool_off + pool.len();
    let blob = (0x80 + toc_size).div_ceil(16) * 16;
    // data region: files, then meta blocks
    let mut data = Vec::new();
    let mut extents = Vec::new();
    for (_, d, z) in &b.files {
        if d.is_empty() {
            extents.push((0u64, 0u64, false));
            continue;
        }
        while !(blob + data.len()).is_multiple_of(16) {
            data.push(0);
        }
        let stored = if *z { zstd_raw(d) } else { d.clone() };
        extents.push(((blob + data.len()) as u64, stored.len() as u64, *z));
        data.extend_from_slice(&stored);
    }
    let mut metas = Vec::new();
    for p in pkgs {
        let mut m = Vec::new();
        for (t, v) in &p.meta {
            m.extend_from_slice(&t.to_le_bytes());
            m.extend_from_slice(&0u16.to_le_bytes());
            m.extend_from_slice(&(v.len() as u32).to_le_bytes());
            m.extend_from_slice(v);
        }
        metas.push(if m.is_empty() {
            (0u64, 0u32)
        } else {
            ((blob + data.len()) as u64, m.len() as u32)
        });
        data.extend_from_slice(&m);
    }
    let mut toc = Vec::new();
    toc.extend_from_slice(b"TOC\x01");
    toc.extend_from_slice(&(pkgs.len() as u32).to_le_bytes());
    for v in [
        b.dirs.len() as u64,
        b.files.len() as u64,
        pool_off as u64,
        pool.len() as u64,
        0,
    ] {
        toc.extend_from_slice(&v.to_le_bytes());
    }
    for (i, p) in pkgs.iter().enumerate() {
        let (root, ex, ro) = recs[i];
        toc.extend_from_slice(&p.title.to_le_bytes());
        for v in [root, ex, ro, p.version] {
            toc.extend_from_slice(&v.to_le_bytes());
        }
        toc.extend_from_slice(&[p.kind, p.flags, 0, 0, 0, 0, 0, 0]);
        toc.extend_from_slice(&metas[i].0.to_le_bytes());
        toc.extend_from_slice(&metas[i].1.to_le_bytes());
        toc.extend_from_slice(&[0u8; 20]);
    }
    for (i, _) in b.dirs.iter().enumerate() {
        let (o, n) = names_d[i];
        for v in [o, n, firsts[i].0, firsts[i].1] {
            toc.extend_from_slice(&v.to_le_bytes());
        }
        toc.extend_from_slice(&0u64.to_le_bytes());
    }
    for (i, (_, d, _)) in b.files.iter().enumerate() {
        let (o, n) = names_f[i];
        toc.extend_from_slice(&o.to_le_bytes());
        toc.extend_from_slice(&n.to_le_bytes());
        let (off, stored, z) = extents[i];
        for v in [d.len() as u64, off, stored] {
            toc.extend_from_slice(&v.to_le_bytes());
        }
        toc.extend_from_slice(&nxb_model::sha256(d).0);
        toc.extend_from_slice(&[u8::from(z), 0, 0, 0]);
        toc.extend_from_slice(&[0u8; 12]);
    }
    for s in &slots {
        toc.extend_from_slice(&s.to_le_bytes());
    }
    toc.extend_from_slice(&pool);
    assert_eq!(toc.len(), toc_size);
    let mut sha = nxb_model::sha256(&toc).0;
    if corrupt_toc {
        sha[0] ^= 1;
    }
    let total = blob + data.len();
    let mut out = Vec::new();
    out.extend_from_slice(b"NUP\x01");
    out.extend_from_slice(&0x80u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0x80u64.to_le_bytes());
    out.extend_from_slice(&(toc_size as u64).to_le_bytes());
    out.extend_from_slice(&sha);
    out.extend_from_slice(&(blob as u64).to_le_bytes());
    out.extend_from_slice(&(total as u64).to_le_bytes());
    out.extend_from_slice(&(pkgs.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&[0u8; 48]);
    out.extend_from_slice(&toc);
    out.resize(blob, 0);
    out.extend_from_slice(&data);
    out
}

const APP: u64 = 0x0400_4860_24DE_C000;

fn code(main: &[u8]) -> Vec<Node> {
    vec![
        Node::File("main", main.to_vec(), true),
        Node::File("main.npdm", npdm(APP), false),
        Node::File(".hidden", b"x".to_vec(), false),
        Node::File("sdk", vec![7u8; 3000], true),
    ]
}

fn romfs(tag: u8) -> Vec<Node> {
    vec![
        Node::File("b.bin", vec![tag; 70_000], true),
        Node::Dir(
            "Data",
            vec![
                Node::File("z.txt", b"zz".to_vec(), false),
                Node::File("empty", Vec::new(), false),
            ],
        ),
        Node::File("A.bin", vec![tag ^ 0x55; 33], false),
    ]
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::var_os("NXB_TEST_TMP")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("nxb-nup-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn req(nup: &Path, work: &Path) -> NupRequest {
    NupRequest {
        nup: nup.to_path_buf(),
        version: None,
        work: work.to_path_buf(),
        threads: 2,
        mem_limit: 64 << 20,
        discard_images: false,
        digests: None,
        skip_images: false,
        progress: None,
    }
}

/// (paths in index order, (path, offset, size) of files) of a DMROMFS1 image.
fn read_image(img: &[u8]) -> Vec<(String, bool, u64, u64)> {
    let u64at = |o: usize| u64::from_le_bytes(img[o..o + 8].try_into().unwrap());
    assert_eq!(&img[..8], b"DMROMFS1");
    let (idx, n) = (u64at(8) as usize, u64at(24) as usize);
    let names = idx + n * 24;
    (0..n)
        .map(|i| {
            let e = idx + i * 24;
            let no = u32::from_le_bytes(img[e + 16..e + 20].try_into().unwrap()) as usize;
            let nl = u16::from_le_bytes(img[e + 20..e + 22].try_into().unwrap()) as usize;
            (
                String::from_utf8_lossy(&img[names + no..names + no + nl]).into_owned(),
                img[e + 22] == 0,
                u64at(e),
                u64at(e + 8),
            )
        })
        .collect()
}

#[test]
fn base_update_and_dlc() {
    let d = tmp("full");
    let pkgs = [
        Pkg {
            title: APP,
            kind: 0,
            version: 0,
            flags: 0,
            exefs: Some(code(b"base main")),
            romfs: Some(romfs(1)),
            meta: vec![(2, vec![9u8; 64]), (3, b"JPEG".to_vec())],
        },
        Pkg {
            title: APP + 0x800,
            kind: 1,
            version: 65536,
            flags: 0,
            exefs: Some(code(b"update main")),
            romfs: Some(romfs(2)),
            meta: vec![],
        },
        Pkg {
            title: APP + 0x1001,
            kind: 2,
            version: 0,
            flags: 0,
            exefs: None,
            romfs: Some(romfs(3)),
            meta: vec![],
        },
    ];
    let nup = d.join("g.nup");
    std::fs::write(&nup, build(&pkgs, false)).unwrap();
    // NXB_NUP_DUMP=<file>: keep the container, for a run of the engine's packer as the oracle
    if let Some(p) = std::env::var_os("NXB_NUP_DUMP") {
        std::fs::copy(&nup, p).unwrap();
    }
    let r = extract_nup(&req(&nup, &d)).unwrap();
    assert_eq!(r.len(), 2);
    let out = d.join("extract/v65536");
    assert_eq!(
        std::fs::read(out.join("ExeFS/main")).unwrap(),
        b"update main"
    );
    assert_eq!(
        std::fs::read(out.join("ExeFS/sdk")).unwrap(),
        vec![7u8; 3000]
    );
    assert!(!out.join("ExeFS/.hidden").exists());
    // the update carries no control data: none written (no fallback while a base game is there)
    assert!(!out.join("control.nacp").exists());
    let img = std::fs::read(out.join("RomFS.img")).unwrap();
    let ents = read_image(&img);
    let names: Vec<&str> = ents.iter().map(|e| e.0.as_str()).collect();
    assert_eq!(names, ["", "A.bin", "Data", "b.bin", "empty", "z.txt"]);
    let get = |n: &str| {
        ents.iter()
            .find(|e| e.0 == n)
            .map(|e| img[e.2 as usize..(e.2 + e.3) as usize].to_vec())
            .unwrap()
    };
    assert_eq!(get("b.bin"), vec![2u8; 70_000]);
    assert_eq!(get("A.bin"), vec![2u8 ^ 0x55; 33]);
    assert_eq!(get("z.txt"), b"zz");
    let aoc = std::fs::read(d.join("aoc/1.img")).unwrap();
    assert_eq!(read_image(&aoc).len(), 6);
    // the base version: its own control data
    let mut rb = req(&nup, &d);
    rb.version = Some(0);
    extract_nup(&rb).unwrap();
    assert_eq!(
        std::fs::read(d.join("extract/v0/control.nacp")).unwrap(),
        vec![9u8; 64]
    );
    assert_eq!(
        std::fs::read(d.join("extract/v0/icon.jpg")).unwrap(),
        b"JPEG"
    );
    // digests only: no image file, the same digest
    let d2 = tmp("discard");
    let mut rd = req(&nup, &d2);
    rd.discard_images = true;
    let r2 = extract_nup(&rd).unwrap();
    assert!(!d2.join("extract/v65536/RomFS.img").exists());
    assert_eq!(r2[0].digest, r[0].digest);
    assert_eq!(r2[0].digest.file.sha256, nxb_model::sha256(&img));
}

#[test]
fn standalone_update_is_the_whole_game() {
    let d = tmp("standalone");
    let pkgs = [Pkg {
        title: APP + 0x800,
        kind: 1,
        version: 131072,
        flags: 1,
        exefs: Some(code(b"m")),
        romfs: Some(romfs(4)),
        meta: vec![(
            10,
            [
                vec![5u8],
                b"en-US".to_vec(),
                3u32.to_le_bytes().to_vec(),
                b"ICO".to_vec(),
            ]
            .concat(),
        )],
    }];
    let nup = d.join("s.nup");
    std::fs::write(&nup, build(&pkgs, false)).unwrap();
    extract_nup(&req(&nup, &d)).unwrap();
    assert_eq!(
        std::fs::read(d.join("extract/v131072/icon.jpg")).unwrap(),
        b"ICO"
    );
    let mut r0 = req(&nup, &d);
    r0.version = Some(0);
    assert!(
        extract_nup(&r0).is_err(),
        "no version 0 without a base game"
    );
    // an update that is not standalone needs a base game
    let pkgs = [Pkg {
        flags: 0,
        ..Pkg {
            title: APP + 0x800,
            kind: 1,
            version: 1,
            flags: 0,
            exefs: Some(code(b"m")),
            romfs: Some(romfs(4)),
            meta: vec![],
        }
    }];
    std::fs::write(&nup, build(&pkgs, false)).unwrap();
    let e = extract_nup(&req(&nup, &d))
        .err()
        .map(|e| e.to_string())
        .unwrap_or_default();
    assert!(e.contains("no base game"), "{e}");
}

#[test]
fn damaged_and_hostile_containers_are_refused() {
    let d = tmp("bad");
    let good = [Pkg {
        title: APP,
        kind: 0,
        version: 0,
        flags: 0,
        exefs: Some(code(b"m")),
        romfs: Some(romfs(1)),
        meta: vec![],
    }];
    let nup = d.join("b.nup");
    std::fs::write(&nup, build(&good, true)).unwrap();
    assert!(
        extract_nup(&req(&nup, &d))
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default()
            .contains("SHA-256")
    );
    // cut short
    let full = build(&good, false);
    std::fs::write(&nup, &full[..full.len() - 1]).unwrap();
    assert!(extract_nup(&req(&nup, &d)).is_err());
    // a file whose content does not match its SHA-256
    let mut flipped = full.clone();
    let last = flipped.len() - 1;
    flipped[last - 10] ^= 0xff;
    std::fs::write(&nup, &flipped).unwrap();
    assert!(extract_nup(&req(&nup, &d)).is_err());
    // an ExeFS name that would leave the folder
    let mut ex = code(b"m");
    ex.push(Node::File("../evil", b"x".to_vec(), false));
    let hostile = [Pkg {
        exefs: Some(ex),
        ..Pkg {
            title: APP,
            kind: 0,
            version: 0,
            flags: 0,
            exefs: None,
            romfs: Some(romfs(1)),
            meta: vec![],
        }
    }];
    std::fs::write(&nup, build(&hostile, false)).unwrap();
    let e = extract_nup(&req(&nup, &d))
        .err()
        .map(|e| e.to_string())
        .unwrap_or_default();
    assert!(e.contains("invalid ExeFS name"), "{e}");
    assert!(!d.join("extract/evil").exists());
    // not a NUP
    std::fs::write(&nup, b"NSP0 something else").unwrap();
    assert!(extract_nup(&req(&nup, &d)).is_err());
}
