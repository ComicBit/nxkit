//! Synthetic game packages for the container tests: real NCA3 files (XTS header, AES-CTR sections, PFS0 and IVFC hash
//! trees) built from fake keys, wrapped as NSP, XCI, NSZ. No game data and no real key appears here.
#![allow(
    dead_code,
    unreachable_pub,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]

use std::path::{Path, PathBuf};

use nxb_extract::aes::Aes128;
use nxb_model::sha256;

pub const HEADER_KEY: [u8; 32] = [0x11; 32];
pub const KAK: [u8; 16] = [0x22; 16];
pub const CTR_KEY: [u8; 16] = [0x33; 16];

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// A key file the synthetic NCAs open with.
pub fn keys_text() -> String {
    format!(
        "header_key = {}\nkey_area_key_application_00 = {}\n",
        hex(&HEADER_KEY),
        hex(&KAK)
    )
}

/// Removes a test folder unless `NXB_FIXTURE_DIR` asks to keep the fixtures (for runs of the reference extractor).
pub fn cleanup(d: &Path) {
    if std::env::var_os("NXB_FIXTURE_DIR").is_none() {
        let _ = std::fs::remove_dir_all(d);
    }
}

pub fn tmp(name: &str) -> PathBuf {
    if let Some(k) = std::env::var_os("NXB_FIXTURE_DIR") {
        let d = PathBuf::from(k).join(name);
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        return d;
    }
    let base = std::env::var_os("NXB_TEST_TMP")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let d = base.join(format!("nxb-extract-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

pub fn write_keys(dir: &Path) -> PathBuf {
    let p = dir.join("prod.keys");
    std::fs::write(&p, keys_text()).unwrap();
    p
}

fn align(n: usize, a: usize) -> usize {
    n.div_ceil(a) * a
}

/// Package files: (name, bytes).
pub type Files = Vec<(String, Vec<u8>)>;

/// PFS0 bytes.
pub fn pfs0(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut names = Vec::new();
    let mut ents = Vec::new();
    let mut off = 0u64;
    for (n, d) in files {
        ents.push((off, d.len() as u64, names.len() as u32));
        names.extend_from_slice(n.as_bytes());
        names.push(0);
        off += d.len() as u64;
    }
    let mut v = b"PFS0".to_vec();
    v.extend_from_slice(&(files.len() as u32).to_le_bytes());
    v.extend_from_slice(&(names.len() as u32).to_le_bytes());
    v.extend_from_slice(&0u32.to_le_bytes());
    for (o, s, n) in ents {
        v.extend_from_slice(&o.to_le_bytes());
        v.extend_from_slice(&s.to_le_bytes());
        v.extend_from_slice(&n.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
    }
    v.extend_from_slice(&names);
    for (_, d) in files {
        v.extend_from_slice(d);
    }
    v
}

/// HFS0 bytes (entries of 0x40 bytes, hashes left zero).
pub fn hfs0(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut names = Vec::new();
    let mut ents = Vec::new();
    let mut off = 0u64;
    for (n, d) in files {
        ents.push((off, d.len() as u64, names.len() as u32));
        names.extend_from_slice(n.as_bytes());
        names.push(0);
        off += d.len() as u64;
    }
    let mut v = b"HFS0".to_vec();
    v.extend_from_slice(&(files.len() as u32).to_le_bytes());
    v.extend_from_slice(&(names.len() as u32).to_le_bytes());
    v.extend_from_slice(&0u32.to_le_bytes());
    for (o, s, n) in ents {
        v.extend_from_slice(&o.to_le_bytes());
        v.extend_from_slice(&s.to_le_bytes());
        v.extend_from_slice(&n.to_le_bytes());
        v.extend_from_slice(&[0u8; 44]);
    }
    v.extend_from_slice(&names);
    for (_, d) in files {
        v.extend_from_slice(d);
    }
    v
}

/// A gamecard dump: the card header (`HEAD` at 0x100, the root HFS0's offset at 0x130) and the root partition with the
/// given partitions (name, files).
pub fn xci(parts: &[(&str, Files)]) -> Vec<u8> {
    let root_at = 0xF000usize;
    let root = hfs0(
        &parts
            .iter()
            .map(|(n, f)| (n.to_string(), hfs0(f)))
            .collect::<Vec<_>>(),
    );
    let mut v = vec![0u8; root_at];
    v[0x100..0x104].copy_from_slice(b"HEAD");
    v[0x130..0x138].copy_from_slice(&(root_at as u64).to_le_bytes());
    v[0x138..0x140].copy_from_slice(&(root.len() as u64).to_le_bytes());
    v.extend_from_slice(&root);
    v
}

/// A section's start in its NCA, bytes and file-system header, decrypted.
pub type Plain = (u64, Vec<u8>, Vec<u8>);

/// A section: its bytes (hash layers and data, a multiple of 0x200) and its 0x200-byte file-system header.
pub struct Section {
    pub bytes: Vec<u8>,
    pub fs: Vec<u8>,
}

/// A PFS0 section with a hierarchical SHA-256 tree (block size 0x1000).
pub fn pfs0_section(files: &[(String, Vec<u8>)]) -> Section {
    let mut data = pfs0(files);
    data.resize(align(data.len(), 0x1000), 0);
    let table: Vec<u8> = data
        .chunks(0x1000)
        .flat_map(|b| sha256(b).0.to_vec())
        .collect();
    let doff = align(table.len(), 0x200);
    let mut bytes = table.clone();
    bytes.resize(doff, 0);
    bytes.extend_from_slice(&data);
    bytes.resize(align(bytes.len(), 0x200), 0);
    let mut fs = vec![0u8; 0x200];
    fs[0..2].copy_from_slice(&2u16.to_le_bytes());
    fs[2] = 1; // PFS0
    fs[3] = 2; // hierarchical SHA-256
    fs[4] = 3; // CTR
    fs[0x8..0x28].copy_from_slice(&sha256(&table).0);
    fs[0x28..0x2C].copy_from_slice(&0x1000u32.to_le_bytes());
    fs[0x2C..0x30].copy_from_slice(&2u32.to_le_bytes());
    fs[0x30..0x38].copy_from_slice(&0u64.to_le_bytes());
    fs[0x38..0x40].copy_from_slice(&(table.len() as u64).to_le_bytes());
    fs[0x40..0x48].copy_from_slice(&(doff as u64).to_le_bytes());
    fs[0x48..0x50].copy_from_slice(&(data.len() as u64).to_le_bytes());
    fs[0x140..0x148].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
    Section { bytes, fs }
}

/// A RomFS image with the given files directly under the root.
fn romfs_image(files: &[(&str, Vec<u8>)]) -> Vec<u8> {
    const NONE: u32 = 0xFFFF_FFFF;
    let mut dirs = vec![0u8; 0x18];
    dirs[4..8].copy_from_slice(&NONE.to_le_bytes());
    dirs[8..12].copy_from_slice(&NONE.to_le_bytes());
    dirs[16..20].copy_from_slice(&NONE.to_le_bytes());
    let mut ftab = Vec::new();
    let mut data = Vec::new();
    for (i, (name, d)) in files.iter().enumerate() {
        let at = ftab.len() as u32;
        let mut e = vec![0u8; 0x20];
        let next = if i + 1 < files.len() {
            at + 0x20 + align(name.len(), 4) as u32
        } else {
            NONE
        };
        e[4..8].copy_from_slice(&next.to_le_bytes());
        e[8..16].copy_from_slice(&(data.len() as u64).to_le_bytes());
        e[16..24].copy_from_slice(&(d.len() as u64).to_le_bytes());
        e[24..28].copy_from_slice(&next.to_le_bytes()); // one hash bucket: every file chains to the next
        e[28..32].copy_from_slice(&(name.len() as u32).to_le_bytes());
        ftab.extend_from_slice(&e);
        ftab.extend_from_slice(name.as_bytes());
        ftab.resize(align(ftab.len(), 4), 0);
        data.extend_from_slice(d);
    }
    let first = if files.is_empty() { NONE } else { 0 };
    dirs[12..16].copy_from_slice(&first.to_le_bytes());
    // one hash bucket for directories (the root) and one for files (the first file)
    let (dh, fh) = (0u32.to_le_bytes(), first.to_le_bytes());
    let mut v = vec![0u8; 0x50];
    v[0..8].copy_from_slice(&0x50u64.to_le_bytes());
    let mut at = 0x50u64;
    let mut body = Vec::new();
    let mut put = |v: &mut Vec<u8>, o: usize, t: &[u8]| {
        v[o..o + 8].copy_from_slice(&at.to_le_bytes());
        v[o + 8..o + 16].copy_from_slice(&(t.len() as u64).to_le_bytes());
        body.extend_from_slice(t);
        body.resize(align(body.len(), 4), 0);
        at = 0x50 + body.len() as u64;
    };
    put(&mut v, 0x8, &dh);
    put(&mut v, 0x18, &dirs);
    put(&mut v, 0x28, &fh);
    put(&mut v, 0x38, &ftab);
    let dat = align(at as usize, 0x10) as u64;
    v[0x48..0x50].copy_from_slice(&dat.to_le_bytes());
    v.extend_from_slice(&body);
    v.resize(dat as usize, 0);
    v.extend_from_slice(&data);
    v
}

/// A RomFS section with an IVFC tree (two stored levels, block size 0x4000).
pub fn romfs_section(files: &[(&str, Vec<u8>)]) -> Section {
    let mut data = romfs_image(files);
    data.resize(align(data.len(), 0x4000), 0);
    let hashes: Vec<u8> = data
        .chunks(0x4000)
        .flat_map(|b| {
            let mut p = b.to_vec();
            p.resize(0x4000, 0);
            sha256(&p).0.to_vec()
        })
        .collect();
    let mut hashes = hashes;
    hashes.resize(align(hashes.len(), 0x4000), 0);
    let mut bytes = hashes.clone();
    bytes.resize(0x4000, 0);
    bytes.extend_from_slice(&data);
    bytes.resize(align(bytes.len(), 0x200), 0);
    let mut fs = vec![0u8; 0x200];
    fs[0..2].copy_from_slice(&2u16.to_le_bytes());
    fs[2] = 0; // RomFS
    fs[3] = 3; // IVFC
    fs[4] = 3; // CTR
    fs[8..12].copy_from_slice(b"IVFC");
    fs[0xC..0x10].copy_from_slice(&0x2_0000u32.to_le_bytes());
    fs[0x10..0x14].copy_from_slice(&0x20u32.to_le_bytes());
    fs[0x14..0x18].copy_from_slice(&3u32.to_le_bytes());
    for (i, (off, size)) in [(0u64, hashes.len() as u64), (0x4000, data.len() as u64)]
        .into_iter()
        .enumerate()
    {
        let o = 0x18 + 0x18 * i;
        fs[o..o + 8].copy_from_slice(&off.to_le_bytes());
        fs[o + 8..o + 16].copy_from_slice(&size.to_le_bytes());
        fs[o + 16..o + 20].copy_from_slice(&14u32.to_le_bytes());
    }
    fs[0xC8..0xE8].copy_from_slice(&sha256(&hashes).0);
    fs[0x140..0x148].copy_from_slice(&[9, 8, 7, 6, 5, 4, 3, 2]);
    Section { bytes, fs }
}

fn xts_encrypt(data: &mut [u8], sector_size: usize) {
    let (k1, k2) = (
        Aes128::new(&HEADER_KEY[..16].try_into().unwrap()),
        Aes128::new(&HEADER_KEY[16..].try_into().unwrap()),
    );
    for (i, sector) in data.chunks_mut(sector_size).enumerate() {
        let mut tweak = [0u8; 16];
        tweak[8..].copy_from_slice(&(i as u64).to_be_bytes());
        k2.encrypt_block(&mut tweak);
        for c in sector.chunks_mut(16) {
            let mut b = [0u8; 16];
            for j in 0..16 {
                b[j] = c[j] ^ tweak[j];
            }
            k1.encrypt_block(&mut b);
            for j in 0..16 {
                c[j] = b[j] ^ tweak[j];
            }
            let mut carry = 0u8;
            for t in tweak.iter_mut() {
                let nc = *t >> 7;
                *t = (*t << 1) | carry;
                carry = nc;
            }
            if carry != 0 {
                tweak[0] ^= 0x87;
            }
        }
    }
}

/// An NCA3 file: content type (0 Program, 1 Meta, 2 Control), program ID, the sections by index. Returns the file's
/// bytes and the plain (decrypted) layout `(start, bytes, fs header)` per used section, for the NCZ writer.
pub fn nca(content: u8, title: u64, secs: Vec<(usize, Section)>) -> (Vec<u8>, Vec<Plain>) {
    let mut h = vec![0u8; 0xC00];
    h[0x200..0x204].copy_from_slice(b"NCA3");
    h[0x205] = content;
    h[0x210..0x218].copy_from_slice(&title.to_le_bytes());
    let mut kek = CTR_KEY;
    Aes128::new(&KAK).encrypt_block(&mut kek);
    h[0x300 + 32..0x300 + 48].copy_from_slice(&kek);
    let mut file = vec![0u8; 0x4000];
    let mut plain = Vec::new();
    let mut at = 0x4000usize;
    for (i, s) in secs {
        let len = s.bytes.len();
        h[0x240 + 16 * i..0x244 + 16 * i].copy_from_slice(&((at / 0x200) as u32).to_le_bytes());
        h[0x244 + 16 * i..0x248 + 16 * i]
            .copy_from_slice(&(((at + len) / 0x200) as u32).to_le_bytes());
        h[0x400 + 0x200 * i..0x600 + 0x200 * i].copy_from_slice(&s.fs);
        h[0x280 + 0x20 * i..0x2A0 + 0x20 * i].copy_from_slice(&sha256(&s.fs).0);
        let upper: [u8; 8] = {
            let mut u: [u8; 8] = s.fs[0x140..0x148].try_into().unwrap();
            u.reverse(); // the section's counter bytes as the reader derives them
            u
        };
        let mut enc = s.bytes.clone();
        Aes128::new(&CTR_KEY).ctr(upper, at as u64, &mut enc);
        file.extend_from_slice(&enc);
        plain.push((at as u64, s.bytes, s.fs));
        at += len;
    }
    h[0x208..0x210].copy_from_slice(&(at as u64).to_le_bytes());
    xts_encrypt(&mut h, 0x200);
    file[..0xC00].copy_from_slice(&h);
    (file, plain)
}

pub fn npdm(program: u64) -> Vec<u8> {
    let mut d = vec![0u8; 0x200];
    d[..4].copy_from_slice(b"META");
    d[0x70..0x74].copy_from_slice(&0x100u32.to_le_bytes());
    d[0x74..0x78].copy_from_slice(&0x40u32.to_le_bytes());
    d[0x100..0x104].copy_from_slice(b"ACI0");
    d[0x110..0x118].copy_from_slice(&program.to_le_bytes());
    d
}

/// A content meta file for the Meta NCA: the type (0x80 application, 0x81 patch, 0x82 add-on content), title, version
/// and the content IDs it lists.
pub fn cnmt(kind: u8, title: u64, version: u32, contents: &[[u8; 16]]) -> Vec<u8> {
    let ext = match kind {
        0x81 => 0x18usize,
        0x82 => 0x10,
        _ => 0x10,
    };
    let mut v = vec![0u8; 0x20 + ext];
    v[0xE..0x10].copy_from_slice(&(ext as u16).to_le_bytes());
    v[0..8].copy_from_slice(&title.to_le_bytes());
    v[8..12].copy_from_slice(&version.to_le_bytes());
    v[0xC] = kind;
    v[0x10..0x12].copy_from_slice(&(contents.len() as u16).to_le_bytes());
    for id in contents {
        let mut e = vec![0u8; 0x38];
        e[0x20..0x30].copy_from_slice(id);
        v.extend_from_slice(&e);
    }
    if kind == 0x81 {
        v.extend_from_slice(&[0u8; 0x40]); // extended data: history, delta and fragment counts, all zero
    }
    v
}

/// One title's NCAs as package files: Program (ExeFS `main` + `main.npdm`, RomFS), Control (name, icon) and Meta.
/// `tag` makes the contents differ between titles.
pub struct Title {
    pub files: Vec<(String, Vec<u8>)>,
    pub program_id: [u8; 16],
    pub plain: Vec<Plain>,
}

pub fn title(kind: u8, app: u64, version: u32, tag: u8, with_control: bool) -> Title {
    let main = vec![tag; 0x5000];
    let sec0 = pfs0_section(&[("main".into(), main), ("main.npdm".into(), npdm(app))]);
    let rom = vec![
        ("game.bin", vec![tag ^ 0x55; 0x9000]),
        ("small.txt", format!("tag {tag}").into_bytes()),
    ];
    let (prog, plain) = nca(0, app, vec![(0, sec0), (1, romfs_section(&rom))]);
    let mut contents = vec![[tag; 16]];
    let mut files = vec![(format!("{}.nca", hex(&[tag; 16])), prog)];
    if with_control {
        let mut nacp = vec![0u8; 0x4000];
        nacp[..9].copy_from_slice(b"Test game");
        nacp[0x3060..0x3064].copy_from_slice(b"1.0.");
        let c = romfs_section(&[
            ("control.nacp", nacp),
            (
                "icon_AmericanEnglish.dat",
                vec![0xFF, 0xD8, tag, 0xFF, 0xD9],
            ),
        ]);
        let (ctl, _) = nca(2, app, vec![(0, c)]);
        let id = [tag ^ 0x80; 16];
        contents.push(id);
        files.push((format!("{}.nca", hex(&id)), ctl));
    }
    let name = hex(&[tag ^ 0x40; 16]);
    let meta = pfs0_section(&[(
        format!("{name}.cnmt"),
        cnmt(
            kind,
            if kind == 0x81 { app | 0x800 } else { app },
            version,
            &contents,
        ),
    )]);
    let (m, _) = nca(1, app, vec![(0, meta)]);
    files.push((format!("{name}.cnmt.nca"), m));
    Title {
        files,
        program_id: [tag; 16],
        plain,
    }
}

/// The files of a RomFS image `RomFS.img` as (path, bytes), read through the image index.
pub fn read_image(img: &[u8]) -> Vec<(String, Vec<u8>)> {
    assert_eq!(&img[..8], b"DMROMFS1");
    let u64at = |o: usize| u64::from_le_bytes(img[o..o + 8].try_into().unwrap());
    let (ioff, n) = (u64at(8) as usize, u64at(24) as usize);
    let pool = ioff + n * 24;
    let mut out = Vec::new();
    let mut todo = vec![(0usize, String::new())];
    while let Some((k, path)) = todo.pop() {
        let e = ioff + k * 24;
        let (a, b) = (u64at(e) as usize, u64at(e + 8) as usize);
        if img[e + 22] == 0 {
            for c in a..a + b {
                let ce = ioff + c * 24;
                let no = u32::from_le_bytes(img[ce + 16..ce + 20].try_into().unwrap()) as usize;
                let nl = u16::from_le_bytes(img[ce + 20..ce + 22].try_into().unwrap()) as usize;
                let name = String::from_utf8_lossy(&img[pool + no..pool + no + nl]).into_owned();
                todo.push((c, format!("{path}/{name}")));
            }
        } else {
            out.push((path, img[a..a + b].to_vec()));
        }
    }
    out.sort();
    out
}

/// Runs the engine's C# `nx_extract` (`NXB_DOTNET` = the `dotnet` binary, `NXB_NX_EXTRACT_DLL` = its built DLL) on a
/// package as base game with no update, `--romfs-image`. Returns its stdout, or None when the variables are absent.
pub fn oracle_extract(keys: &Path, pkg: &Path, out: &Path, extra: &[&str]) -> Option<String> {
    let dotnet = std::env::var_os("NXB_DOTNET")?;
    let dll = std::env::var_os("NXB_NX_EXTRACT_DLL")?;
    let r = std::process::Command::new(dotnet)
        .arg(dll)
        .arg(keys)
        .arg(pkg)
        .arg("-")
        .arg(out)
        .arg("--romfs-image")
        .args(extra)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&r.stdout).into_owned();
    assert!(r.status.success(), "nx_extract failed: {text}");
    Some(text)
}

/// The outputs of one version are the same files in both folders.
pub fn same_outputs(a: &Path, b: &Path) {
    for f in [
        "RomFS.img",
        "control.nacp",
        "icon.jpg",
        "ExeFS/main",
        "ExeFS/main.npdm",
    ] {
        let x = std::fs::read(a.join(f)).unwrap();
        let y = std::fs::read(b.join(f)).unwrap();
        assert!(x == y, "{f} differs");
    }
}
