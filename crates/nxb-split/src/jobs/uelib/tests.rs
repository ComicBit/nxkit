use super::super::uejob::{tables, ue_job};
use super::super::uetoc::tests::{Fix, M, files, id, with_fake};
use super::*;
use crate::jobs::own;
use crate::oodle::set_local_decoder;

#[test]
fn city64_matches_the_reference() {
    let v = [
        (0, 0x9ae1_6a3b_2f90_404f),
        (1, 0x4271_b80e_2915_c76a),
        (3, 0x373a_2b2a_12e0_c581),
        (4, 0xcc1e_7781_37ae_6d69),
        (7, 0x9608_38fb_c6d9_c975),
        (8, 0x7d50_7025_ed4e_ecfd),
        (15, 0x834f_200c_fa7e_d95f),
        (16, 0xd836_4803_26a9_74d6),
        (17, 0x5cde_d6cb_65c9_dac6),
        (31, 0x52a4_67e8_7c92_d522),
        (32, 0xa033_00e1_b2be_6da7),
        (33, 0x6e14_215d_4c72_00d4),
        (63, 0xc9fc_77de_ad95_c3aa),
        (64, 0x2a7a_b541_551d_3235),
        (65, 0x5709_ecef_010a_51e4),
        (127, 0xecd6_ea5d_fa19_6e8c),
        (128, 0xa9e7_9ad8_f1c9_f5a7),
        (200, 0xc859_bc06_9682_37c1),
        (299, 0x053a_eac4_2241_9c98),
    ];
    for (n, h) in v {
        let s: Vec<u8> = (0..n).map(|i| ((i * 37 + 11) & 0xff) as u8).collect();
        assert_eq!(city64(&s), h, "length {n}");
    }
}

const NAME: &str = "ShaderArchive-Global-SF_GLSL_SWITCH-SP_GLSL_SWITCH";

/// The chunk ID of the library of [`NAME`].
fn lib_id() -> [u8; 12] {
    let mut x = id(0x5a, IO_SHADER_LIBRARY);
    x[..8].copy_from_slice(&[0xbe, 0x76, 0xfe, 0xab, 0x04, 0x15, 0xc1, 0x56]);
    x
}

#[test]
fn names_come_from_the_manifests() {
    let m = b"Engine/ShaderArchive-Global-SF_GLSL_SWITCH-SP_GLSL_SWITCH.ushaderbytecode\t1\n\
        x/ShaderArchive-Game-SF_X.ushaderbytecode ShaderArchive-.ushaderbytecode \
        ShaderArchive-Bad.ushaderbytecodes ShaderArchive-Game-SF_X.ushaderbytecode"
        .to_vec();
    let names = archive_names(&[m, b"ShaderArchive-A_b.ushaderbytecode".to_vec()]);
    assert_eq!(
        names,
        [
            "ShaderArchive-A_b",
            "ShaderArchive-Bad",
            "ShaderArchive-Game-SF_X",
            NAME
        ]
    );
    assert_eq!(io_library_name(&lib_id(), &names), NAME);
    let other = id(7, IO_SHADER_LIBRARY);
    assert_eq!(
        io_library_name(&other, &names),
        "ShaderArchive-IoStore-0707070707070707"
    );
}

const G0: [u8; 12] = [0x20; 12];
const G1: [u8; 12] = [0x21; 12];

/// A version 1 library of three shaders: 0 and 2 in group 0 (Oodle, `AB` repeated to 10 bytes), 1 in group 1
/// (stored, 3 bytes).
fn library(version: u32, gids: [[u8; 12]; 2], ents: [u64; 3]) -> Vec<u8> {
    let mut b = version.to_le_bytes().to_vec();
    let mut table = |n: u32, d: &[u8]| {
        b.extend(n.to_le_bytes());
        b.extend(d);
    };
    table(1, &[0x11; 20]);
    table(3, &[0x22; 60]);
    table(2, &gids.concat());
    table(1, &[0, 0, 0, 0, 3, 0, 0, 0]);
    table(
        3,
        &ents
            .iter()
            .flat_map(|e| e.to_le_bytes())
            .collect::<Vec<_>>(),
    );
    let groups: Vec<u8> = [0u32, 2, 10, 3, 2, 1, 3, 3]
        .iter()
        .flat_map(|x| x.to_le_bytes())
        .collect();
    table(2, &groups);
    table(3, &[0, 0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0]);
    b
}

const ENTS: [u64; 3] = [1, 1 << 4 | 2, 4 << 34 | 3];

fn group0() -> Vec<u8> {
    vec![0xbb, b'A', b'B']
}

/// The base container `Paks/global` with the library and both groups.
fn base(lib: Vec<u8>) -> Fix {
    let mut f = Fix::new(vec![
        (id(1, 1), b"package".to_vec(), M::Stored),
        (lib_id(), lib, M::Zlib),
        (G0, group0(), M::Stored),
        (G1, b"xyz".to_vec(), M::Lz4),
    ]);
    f.part = Some(160);
    f
}

fn romfs(fixes: &[(&str, &Fix)]) -> Vec<(String, Arc<dyn Src>)> {
    let mut v: Vec<(String, Arc<dyn Src>)> = vec![(
        "Manifest_NonUFSFiles_Switch.txt".into(),
        own(format!("Engine/Content/{NAME}.ushaderbytecode\n").into_bytes()),
    )];
    for (stem, f) in fixes {
        v.extend(files(stem, f));
    }
    v
}

fn want(code1: &[u8]) -> Vec<u8> {
    let mut w = Vec::new();
    let mut u = |x: u32| w.extend(x.to_le_bytes());
    u(2);
    u(1);
    w.extend([0x11; 20]);
    w.extend(3u32.to_le_bytes());
    w.extend([0x22; 60]);
    for x in [1u32, 0, 3, 0, 0, 3] {
        w.extend(x.to_le_bytes());
    }
    for (off, len, f) in [(0u64, 4u32, 1u8), (4, 3, 2), (7, 6, 3)] {
        w.extend(off.to_le_bytes());
        w.extend(len.to_le_bytes());
        w.extend(len.to_le_bytes());
        w.push(f);
    }
    for x in [0u32, 3, 0, 2, 1] {
        w.extend(x.to_le_bytes());
    }
    w.extend(b"ABAB");
    w.extend(code1);
    w.extend(b"ABABAB");
    w
}

fn no_key() -> impl FnMut() -> R<Option<[u8; 32]>> {
    || panic!("no container is encrypted")
}

#[test]
fn libraries_decode_to_archives() {
    with_fake();
    let f = base(library(1, [G0, G1], ENTS));
    let fs = romfs(&[("Game/Content/Paks/global", &f)]);
    assert!(fs.iter().any(|x| x.0.ends_with("global_s1.ucas")));
    let libs = io_libraries(&fs, &mut no_key()).expect("libraries");
    assert_eq!(libs.len(), 1);
    let l = &libs[0];
    assert_eq!(l.name, format!("{NAME}.ushaderbytecode"));
    assert_eq!(l.rel, "Game/Content/Paks/global.utoc");
    assert_eq!(l.path, format!("chunk {}", hex(&lib_id())));
    assert_eq!(l.shaders, 3);
    assert_eq!(l.bytes, want(b"xyz"));
    let (sh, _, _) = tables(&l.bytes, "x").expect("tables");
    assert_eq!(sh, [(0, 4, 4, 1), (4, 3, 3, 2), (7, 6, 6, 3)]);
    let job = ue_job(&fs, &[]).expect("job");
    assert_eq!(job.archives.len(), 1);
    let a = &job.archives[0];
    assert_eq!(
        (a.compression, a.compressed, a.shaders),
        ("IoStore, decoded", 0, 3)
    );
    assert_eq!(a.pak, l.rel);
    // no library: no decoder is needed
    set_local_decoder(Some(None));
    let plain = Fix::new(vec![(id(1, 1), b"p".to_vec(), M::Stored)]);
    assert!(
        io_libraries(&files("a", &plain), &mut no_key())
            .expect("none")
            .is_empty()
    );
    set_local_decoder(None);
}

#[test]
fn the_newest_container_wins() {
    with_fake();
    let f = base(library(1, [G0, G1], ENTS));
    let patch = Fix::new(vec![
        (G1, b"uvw".to_vec(), M::Oodle),
        (lib_id(), library(1, [G0, G1], ENTS), M::Stored),
    ]);
    let fs = romfs(&[("Paks/global_0_P", &patch), ("Paks/global", &f)]);
    let libs = io_libraries(&fs, &mut no_key()).expect("libraries");
    assert_eq!(libs.len(), 1);
    assert_eq!(libs[0].rel, "Paks/global_0_P.utoc");
    assert_eq!(libs[0].bytes, want(b"uvw"));
    set_local_decoder(None);
}

#[test]
fn an_encrypted_container_asks_for_the_key_once() {
    with_fake();
    let f = base(library(1, [G0, G1], ENTS));
    let mut enc = Fix::new(vec![(id(9, 1), vec![0; 16], M::Stored)]);
    enc.encrypted = true;
    let fs = romfs(&[
        ("Paks/global", &f),
        ("Paks/other", &enc),
        ("Paks/more", &enc),
    ]);
    let mut asked = 0;
    let libs = io_libraries(&fs, &mut || {
        asked += 1;
        Ok(Some([7; 32]))
    })
    .expect("libraries");
    assert_eq!((libs.len(), asked), (1, 1));
    let e = io_libraries(&fs, &mut || Err(crate::SplitError::Red("no key".into())));
    assert!(e.is_err());
    set_local_decoder(None);
}

fn fails(lib: Vec<u8>, drop_g0: bool) -> String {
    let mut f = base(lib);
    if drop_g0 {
        f.chunks.retain(|c| c.0 != G0);
    }
    match io_libraries(&romfs(&[("Paks/global", &f)]), &mut no_key()) {
        Ok(_) => panic!("no error"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn damaged_libraries_are_errors() {
    with_fake();
    assert!(fails(library(2, [G0, G1], ENTS), false).contains("is not version 1"));
    let mut extra = library(1, [G0, G1], ENTS);
    extra.push(0);
    assert!(fails(extra, false).contains("does not parse"));
    let cut = library(1, [G0, G1], ENTS);
    assert!(fails(cut[..cut.len() - 2].to_vec(), false).contains("shader library chunk"));
    assert!(fails(library(1, [G0, G1], ENTS), true).contains("is in no container"));
    assert!(
        fails(library(1, [id(1, 1), G1], ENTS), false).contains("is 7 bytes, its entry says 3")
    );
    let wrong = [ENTS[0], ENTS[1], 4 << 34 | 1 << 4 | 3];
    assert!(fails(library(1, [G0, G1], wrong), false).contains("names another"));
    let mut huge = library(1, [G0, G1], ENTS);
    let at = huge.len() - 16 - 32 + 8;
    huge[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(fails(huge, false).contains("shader group 0 of"));
    set_local_decoder(Some(None));
    let e = fails(library(1, [G0, G1], ENTS), false);
    assert!(
        e.contains("Paks/global.utoc: IoStore shader libraries need the game's decoder"),
        "{e}"
    );
    set_local_decoder(None);
}
