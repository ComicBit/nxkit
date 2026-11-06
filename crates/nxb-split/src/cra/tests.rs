use super::*;

const ARC: &[u8] = include_bytes!("../../tests/data/jb_cra/42");
const NRO: &[u8] = include_bytes!("../../tests/data/jb_cra/b_game.nro");
const STUB: &[u8] = include_bytes!("../../tests/data/jb_cra/a_stub.nro");
const NVN: &[u8] = include_bytes!("../../tests/data/sumo_nvnsh.bin");

// values of `cra.py` for the same input
#[test]
fn keys_match_the_reference() {
    let want: [(&str, u32); 7] = [
        ("shader/font/font.bin", 4_121_638_759),
        ("a/b/ad.bin", 861_664_312),
        ("data/_x.bin", 636_398_387),
        ("data/ab.cd_ef", 1_623_891_420),
        ("x/aa.bin", 1_224_878_382),
        ("x/al.bin", 1_078_487_927),
        ("", 636_398_387),
    ];
    for (p, k) in want {
        assert_eq!(key(p.as_bytes()), k, "{p}");
    }
}

#[test]
fn every_variant_unscrambles_as_the_reference() {
    let raw: Vec<u8> = (0..32).collect();
    let want = [
        (
            "x/ad.bin",
            "6924a654f9a7d0c47a3261de2ca60b084feabf37df0764e6ae1b12023a204f1e",
            2_763_739_466_499_837_480u64,
        ),
        (
            "x/ae.bin",
            "de9e456e2ff965034e4183d5e45fa29f0ab736b243bdf41c688f9ab612ecb8f3",
            2_331_376_362_700_166_632,
        ),
        (
            "x/ag.bin",
            "526a343db2ba94d9b5735bbb8c2caa0274a42d5ed49d7fba615a2867725ac51c",
            2_265_177_567_944_494_614,
        ),
        (
            "x/af.bin",
            "11df7fcb52ab5a03750f11bc418410d6c5f60c974ec3e10b53c108df33545b03",
            1_097_874_048_593_745_366,
        ),
        (
            "x/ab.bin",
            "1b482e5ffb777caef43f109f2c3c5e05bd1070bebd741d20f0e42b53560e8006",
            855_202_905_517_204_708,
        ),
        (
            "x/ac.bin",
            "581360d9f6a826197d2daf6c29d1b84d5cc85b959d9e380ddb75f18de3f2d97d",
            2_765_007_399_067_311_028,
        ),
        (
            "x/aa.bin",
            "0b38d47f23eddde9fe7907937e06a304ad608a9e65993b5afaa23c5fe4dacd00",
            392_060_982_668_827_896,
        ),
        (
            "x/al.bin",
            "ad75e03dd663760d905aec0866c076a7a9aedbf1d235d9003602b2e9d0e09c5a",
            1_031_211_791_620_514_902,
        ),
    ];
    let mut variants = [false; 8];
    for (p, hex, total) in want {
        let (out, t) = unscramble(p.as_bytes(), &raw);
        let got: String = out.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!((got.as_str(), t), (hex, total), "{p}");
        variants[(key(p.as_bytes()) & 7) as usize] = true;
    }
    assert!(variants.iter().all(|&v| v));
    // a tail shorter than a block is dropped
    assert_eq!(unscramble(b"x/ad.bin", &raw[..31]).0.len(), 16);
}

#[test]
fn the_index_is_read_from_the_code() {
    let ix = Index::new(NRO).unwrap().unwrap();
    let p = format!("{:?}", ix.parts());
    assert!(
        p.contains("table: 4096, count: 16, sums: 4496, names: Some(4456)"),
        "{p}"
    );
    let es = ix.files().unwrap();
    let want: [(usize, &str, u32, u64, u32); 15] = [
        (1, "shader/font/font.bin", 4712, 672, 1_694_752_440),
        (2, "shader/font/font_sub.narc", 9519, 5392, 3_016_109_657),
        (3, "sound/bgm/bgm.bin", 4728, 14912, 3_723_105_123),
        (4, "movie/movie.bin", 4728, 19648, 3_374_518_724),
        (5, "model/chr/chr/chr.bin", 754, 24384, 1_103_363_440),
        (6, "data/ad.bin", 40, 25152, 134_744_072),
        (7, "data/ae.bin", 49, 25200, 337_386_541),
        (8, "data/ag.bin", 58, 25264, 4_244_389_318),
        (9, "data/af.bin", 67, 25328, 299_419_597),
        (10, "data/ab.bin", 76, 25408, 1_767_268_181),
        (11, "data/ac.bin", 85, 25488, 2_795_610_137),
        (12, "data/aa.bin", 94, 25584, 2_391_808_042),
        (13, "data/al.bin", 103, 25680, 4_234_443_874),
        (14, "data/_x.bin", 20, 25792, 0),
        (15, "data/ab.cd_ef", 17, 25824, 0),
    ];
    let got: Vec<(usize, String, u32, u64, u32)> = es
        .iter()
        .map(|e| (e.index, e.label(), e.size, e.off, e.sum))
        .collect();
    let want: Vec<(usize, String, u32, u64, u32)> = want
        .iter()
        .map(|&(i, p, s, o, c)| (i, p.to_string(), s, o, c))
        .collect();
    assert_eq!(got, want);
}

#[test]
fn an_nro_without_the_table_is_not_an_error() {
    assert!(Index::new(STUB).unwrap().is_none());
    assert!(Index::new(b"MZ not an nro").unwrap().is_none());
    assert!(Index::new(&[]).unwrap().is_none());
    // an NRO header cut before its text size
    let mut short = vec![0u8; 0x20];
    short[0x10..0x14].copy_from_slice(b"NRO0");
    assert!(Index::new(&short).is_err());
    // a text size past the end of the file
    let mut long = vec![0u8; 0x40];
    long[0x10..0x14].copy_from_slice(b"NRO0");
    long[0x24..0x28].copy_from_slice(&0x1000u32.to_le_bytes());
    assert!(Index::new(&long).is_err());
}

#[test]
fn files_are_read_and_checked() {
    let es = Index::new(NRO).unwrap().unwrap().files().unwrap();
    for e in &es {
        let raw = &ARC[e.off as usize..][..span(e)];
        let plain = read(raw, e).unwrap();
        assert_eq!(plain.len(), e.size as usize, "{}", e.label());
        assert_eq!(head(raw, e), plain[..16], "{}", e.label());
    }
    let raw = &ARC[es[0].off as usize..][..span(&es[0])];
    assert_eq!(read(raw, &es[0]).unwrap(), NVN);
    let mut bad = raw.to_vec();
    bad[3] ^= 1;
    assert_eq!(
        read(&bad, &es[0]),
        Err(SplitError::Red(
            "Error: shader/font/font.bin: the checksum differs".into()
        ))
    );
    // a short read cannot pass the checksum either
    assert!(read(&raw[..raw.len() - 16], &es[0]).is_err());
}
