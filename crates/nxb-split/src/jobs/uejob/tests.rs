use super::*;

fn arc(shaders: &[(u64, u32, u32, u8)], code: &[u8]) -> Vec<u8> {
    let mut b = ARCHIVE_VERSION.to_le_bytes().to_vec();
    for (n, e) in [(1u32, 20usize), (0, 20), (1, 16)] {
        b.extend(n.to_le_bytes());
        b.extend(vec![0xeeu8; n as usize * e]);
    }
    b.extend((shaders.len() as u32).to_le_bytes());
    for (o, s, r, f) in shaders {
        b.extend(o.to_le_bytes());
        b.extend(s.to_le_bytes());
        b.extend(r.to_le_bytes());
        b.push(*f);
    }
    b.extend(0u32.to_le_bytes());
    b.extend(1u32.to_le_bytes());
    b.extend(7u32.to_le_bytes());
    b.extend_from_slice(code);
    b
}

#[test]
fn lz4_shaders_decode_in_place() {
    // shader 1: one literal-only LZ4 sequence of an LSLG record
    let a = arc(&[(0, 4, 4, 1), (4, 9, 8, 2)], b"abcd\x80LSLG1234");
    let (sh, _, code) = tables(&a, "x").expect("tables");
    let raws = lz4_shaders(&a, &sh, code).ok().flatten().expect("LZ4");
    let d = decoded_archive(&a, &raws).expect("decoded");
    assert_eq!(d, arc(&[(0, 4, 4, 1), (4, 8, 8, 2)], b"abcdLSLG1234"));
    // no LSLG record: the game's decoder is needed
    let o = arc(&[(0, 9, 8, 0)], b"\x80XXXX1234");
    let (sh, _, code) = tables(&o, "x").expect("tables");
    assert!(matches!(lz4_shaders(&o, &sh, code), Ok(None)));
    assert!(decoded_archive(&a, &raws[..1]).is_err());
    assert!(tables(&a[..30], "x").is_err());
}

#[test]
fn job_file_and_finish() {
    let a = arc(&[(0, 4, 4, 0), (4, 3, 5, 1)], b"abcdxyz");
    let mut job = UeJob::default();
    job.archives.push(UeArchive {
        name: "ShaderArchive-G-SF_X.ushaderbytecode".into(),
        pak: "Content/Paks/a.pak".into(),
        path: "G/Content/ShaderArchive-G-SF_X.ushaderbytecode".into(),
        shaders: 2,
        compressed: 1,
        compression: "Oodle",
        bytes: a,
    });
    job.recs.push((0, 1, b"xyz".to_vec(), 5));
    assert_eq!(job.records(), 1);
    let mut want = b"DMOJ".to_vec();
    want.extend(1u32.to_le_bytes());
    want.extend(3u32.to_le_bytes());
    want.extend(5u32.to_le_bytes());
    want.extend(b"xyz");
    assert_eq!(job.job_file(), want);
    let out = |l: u32, d: &[u8]| {
        let mut o = b"DMOR".to_vec();
        o.extend(1u32.to_le_bytes());
        o.extend(l.to_le_bytes());
        o.extend_from_slice(d);
        o
    };
    let r = job.finish(Some(&out(5, b"hello"))).expect("finish");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].1, arc(&[(0, 4, 4, 0), (4, 5, 5, 1)], b"abcdhello"));
    let e = job.finish(Some(&out(0, b""))).err().map(|e| e.to_string());
    assert!(e.is_some_and(|e| e.contains("shader 1 was not decoded")));
    assert!(job.finish(None).is_err());
    assert!(job.finish(Some(&out(5, b"hel"))).is_err());
}

#[test]
fn code_keys_follow_adrp_and_ldr() {
    let mut img = vec![0u8; 0x1100];
    let ins: [u32; 4] = [
        0xB000_0001,                        // adrp x1, page +0x1000
        0x3DC0_0000 | 1 << 10 | 1 << 5 | 2, // ldr q2, [x1, #0x10]
        0x3DC0_0000 | 2 << 10 | 1 << 5 | 3, // ldr q3, [x1, #0x20]
        0xAD00_0000 | 3 << 10 | 2,          // stp q2, q3, [x0]
    ];
    for (i, w) in ins.iter().enumerate() {
        img[4 * i..4 * i + 4].copy_from_slice(&w.to_le_bytes());
    }
    for i in 0..32 {
        img[0x1010 + i] = i as u8 + 1;
    }
    let k = code_keys(&img, 0x1000);
    assert_eq!(k.len(), 1);
    assert_eq!(k[0].to_vec(), (1..=32).collect::<Vec<u8>>());
    // a write to the page register in between drops it
    img[4..8].copy_from_slice(&0xD280_0001u32.to_le_bytes());
    img[8..12].copy_from_slice(&(0x3DC0_0000u32 | 1 << 10 | 1 << 5 | 2).to_le_bytes());
    assert!(code_keys(&img, 0x1000).is_empty());
    assert_eq!(find_key(&img, 0x1000, img.len(), &[0; 16]), None);
}
