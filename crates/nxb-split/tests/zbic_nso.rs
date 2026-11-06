//! An NSO whose text segment is a ZBIC stream (flag bit 7) is read into its memory image.

use nxb_model::sha256;

fn u(d: &[u8], o: usize) -> usize {
    u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]) as usize
}

#[test]
fn zbic_text_segment_is_decoded() {
    let cases = include_bytes!("../../nxb-codec/tests/data/zbic_cases.bin");
    let (clen, rlen) = (u(cases, 4), u(cases, 8));
    let sum = &cases[12..44];
    let stream = &cases[44..44 + clen];
    let raw = nxb_codec::zbic::decompress(stream, rlen).expect("decodes");
    assert_eq!(&sha256(&raw).0[..], sum);
    let ro_at = rlen.div_ceil(0x1000) * 0x1000;
    let (ro, data) = ([7u8; 0x10], [9u8; 0x10]);
    let mut d = vec![0u8; 0x100];
    d[..4].copy_from_slice(b"NSO0");
    // text compressed with a hash check, ro and data stored as they are
    let flags: u32 = 1 | (1 << 3) | (1 << 7);
    d[0xc..0x10].copy_from_slice(&flags.to_le_bytes());
    let segs = [
        (0x100usize, 0usize, rlen),
        (0x100 + clen, ro_at, 0x10),
        (0x110 + clen, ro_at + 0x1000, 0x10),
    ];
    for (i, (fo, mo, sz)) in segs.iter().enumerate() {
        for (k, v) in [fo, mo, sz].iter().enumerate() {
            d[0x10 + 16 * i + 4 * k..0x14 + 16 * i + 4 * k]
                .copy_from_slice(&(**v as u32).to_le_bytes());
        }
    }
    d[0x60..0x64].copy_from_slice(&(clen as u32).to_le_bytes());
    d[0xa0..0xc0].copy_from_slice(&sha256(&raw).0);
    d.extend_from_slice(stream);
    d.extend_from_slice(&ro);
    d.extend_from_slice(&data);
    let img = nxb_split::nso::image(&d).expect("image");
    assert_eq!(&img[..rlen], &raw[..]);
    assert_eq!(&img[ro_at..ro_at + 0x10], &ro);
    assert_eq!(&img[ro_at + 0x1000..ro_at + 0x1010], &data);
}

/// Real modules: `NXB_ZBIC_EXEFS` is a folder of NSO files (`main`, `sdk`, ...). Each segment's SHA-256 is checked by the
/// reader when the module asks for it, so a clean read is the proof. Skipped without the variable.
#[test]
fn real_zbic_modules_read() {
    let Some(dir) = std::env::var_os("NXB_ZBIC_EXEFS") else {
        return;
    };
    let mut n = 0;
    for e in std::fs::read_dir(dir).expect("dir").flatten() {
        let d = std::fs::read(e.path()).expect("file");
        if d.get(..4) != Some(b"NSO0") {
            continue;
        }
        let img = nxb_split::nso::image(&d).unwrap_or_else(|x| panic!("{:?}: {x}", e.file_name()));
        eprintln!(
            "{:?}: flags {:#x}, image {:#x} bytes",
            e.file_name(),
            u(&d, 0xc),
            img.len()
        );
        n += 1;
    }
    assert!(n > 0);
}
