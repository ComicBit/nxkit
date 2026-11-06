//! A BND3 archive (format 0x5c, little-endian) whose entry names are Shift JIS.

use super::*;

fn bnd3(entries: &[(&[u8], &[u8])]) -> Vec<u8> {
    let names_at = 0x20 + 28 * entries.len();
    let mut names = Vec::new();
    let mut name_off = Vec::new();
    for (n, _) in entries {
        name_off.push(names_at + names.len());
        names.extend_from_slice(n);
        names.push(0);
    }
    let mut data_at = names_at + names.len();
    let mut d = vec![0u8; 0x20];
    d[..4].copy_from_slice(b"BND3");
    d[12] = 0x5c;
    d[16..20].copy_from_slice(&(entries.len() as u32).to_le_bytes());
    for (i, (_, b)) in entries.iter().enumerate() {
        d.extend_from_slice(&0u32.to_le_bytes());
        d.extend_from_slice(&(b.len() as u32).to_le_bytes());
        d.extend_from_slice(&(data_at as u64).to_le_bytes());
        d.extend_from_slice(&(i as u32).to_le_bytes());
        d.extend_from_slice(&(name_off[i] as u32).to_le_bytes());
        d.extend_from_slice(&(b.len() as u32).to_le_bytes());
        data_at += b.len();
    }
    d.extend_from_slice(&names);
    for (_, b) in entries {
        d.extend_from_slice(b);
    }
    d
}

#[test]
fn double_byte_names_are_decoded() {
    // N:\FRPG\Source\Shader\表ソース.vpo and 日本語.fpo
    let a = [
        0x4e, 0x3a, 0x5c, 0x46, 0x52, 0x50, 0x47, 0x5c, 0x53, 0x6f, 0x75, 0x72, 0x63, 0x65, 0x5c,
        0x53, 0x68, 0x61, 0x64, 0x65, 0x72, 0x5c, 0x95, 0x5c, 0x83, 0x5c, 0x81, 0x5b, 0x83, 0x58,
        0x2e, 0x76, 0x70, 0x6f,
    ];
    let b = [0x93, 0xfa, 0x96, 0x7b, 0x8c, 0xea, 0x2e, 0x66, 0x70, 0x6f];
    let d = bnd3(&[(&a, b"one"), (&b, b"two")]);
    let m = bnd3_members(&d, "x").unwrap();
    assert_eq!(m[0], ("Shader/表ソース.vpo".to_string(), b"one".to_vec()));
    assert_eq!(m[1], ("日本語.fpo".to_string(), b"two".to_vec()));
}
