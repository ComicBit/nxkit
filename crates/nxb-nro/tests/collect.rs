//! A synthetic image: one NRR that registers a loose NRO and one hash nobody has.

use nxb_nro::{Fail, collect};

fn image(files: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let n = files.len() + 1;
    let (mut pool, mut data) = (Vec::new(), Vec::new());
    let base = 32u64;
    let mut ents = vec![(1u64, files.len() as u64, 0u32, 0u16, 0u8)];
    for (name, d) in files {
        ents.push((
            base + data.len() as u64,
            d.len() as u64,
            pool.len() as u32,
            name.len() as u16,
            1,
        ));
        pool.extend_from_slice(name.as_bytes());
        data.extend_from_slice(d);
    }
    let off = base + data.len() as u64;
    let mut idx = Vec::new();
    for (a, b, no, nl, t) in ents {
        idx.extend_from_slice(&a.to_le_bytes());
        idx.extend_from_slice(&b.to_le_bytes());
        idx.extend_from_slice(&no.to_le_bytes());
        idx.extend_from_slice(&nl.to_le_bytes());
        idx.extend_from_slice(&[t, 0]);
    }
    idx.extend_from_slice(&pool);
    let mut v = b"DMROMFS1".to_vec();
    for x in [off, idx.len() as u64, n as u64] {
        v.extend_from_slice(&x.to_le_bytes());
    }
    v.extend_from_slice(&data);
    v.extend_from_slice(&idx);
    v
}

fn nro(tag: u8) -> Vec<u8> {
    let mut v = vec![tag; 0x80];
    v[0x10..0x14].copy_from_slice(b"NRO0");
    v[0x18..0x1c].copy_from_slice(&0x80u32.to_le_bytes());
    v
}

fn nrr(hashes: &[[u8; 32]]) -> Vec<u8> {
    let mut v = vec![0u8; 0x350 + 32 * hashes.len()];
    v[..4].copy_from_slice(b"NRR0");
    v[0x340..0x344].copy_from_slice(&0x350u32.to_le_bytes());
    v[0x344..0x348].copy_from_slice(&(hashes.len() as u32).to_le_bytes());
    for (i, h) in hashes.iter().enumerate() {
        v[0x350 + 32 * i..0x350 + 32 * i + 32].copy_from_slice(h);
    }
    v
}

#[test]
fn loose_nro_is_collected_and_missing_hash_is_unsupported() {
    let d = std::env::temp_dir().join(format!("nxb-nro-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("dir");
    let (a, b) = (nro(1), nro(2));
    let (ha, hb) = (nxb_model::sha256(&a).0, nxb_model::sha256(&b).0);
    let img = d.join("RomFS.img");
    std::fs::write(
        &img,
        image(&[
            ("a.nrr", nrr(&[ha, hb])),
            ("m.nro", a.clone()),
            ("z.nro", b.clone()),
        ]),
    )
    .expect("img");
    let out = d.join("out");
    let s = collect(&img, &out, None, false).ok().expect("collect");
    assert_eq!((s.written, s.refused, s.bytes), (2, 0, 0x100));
    let tsv = std::fs::read_to_string(out.join("nro.tsv")).expect("tsv");
    assert!(tsv.starts_with("# name\tsha256\tsize\tnrr\twhere\nnro0\t"));
    assert_eq!(tsv.lines().count(), 3);
    assert!(tsv.contains("\t128\ta.nrr\tloose m.nro"));
    assert_eq!(
        std::fs::read(out.join("nro0.nro")).expect("nro0").len(),
        0x80
    );
    // a registered hash that is nowhere: an error, or with missing_ok a row of missing.tsv
    std::fs::write(&img, image(&[("a.nrr", nrr(&[ha, [9; 32]])), ("m.nro", a)])).expect("img2");
    assert!(matches!(
        collect(&img, &d.join("o2"), None, false),
        Err(Fail::Error(_))
    ));
    assert!(!d.join("o2").exists());
    let s = collect(&img, &d.join("o3"), None, true)
        .ok()
        .expect("missing ok");
    assert_eq!((s.written, s.missing), (1, 1));
    assert_eq!(
        std::fs::read_to_string(d.join("o3/missing.tsv")).expect("missing"),
        format!("{}\ta.nrr\n", "09".repeat(32))
    );
    let _ = std::fs::remove_dir_all(&d);
}
