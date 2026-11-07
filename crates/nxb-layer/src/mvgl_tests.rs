#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::*;

#[test]
fn doboz_stored_stream() {
    assert_eq!(doboz(&[0x80, 3, 3, b'a', b'b', b'c']).unwrap(), b"abc");
}

#[test]
fn doboz_literals_and_a_match() {
    // three literals, a match of 3 bytes at distance 3, then the 8 trailing literals
    let ctl = (1u32 << 3) | (1 << 31);
    let mut b = vec![0x00, 14, 16];
    b.extend_from_slice(&ctl.to_le_bytes());
    b.extend_from_slice(b"abc");
    b.push(3 << 2);
    b.extend_from_slice(b"12345678");
    assert_eq!(doboz(&b).unwrap(), b"abcabc12345678");
}

#[test]
fn mvgl_rejects_other_files() {
    assert!(mvgl_entries(|_, n| Ok(vec![0; n])).is_err());
}

#[test]
fn mvgl_lists_its_files() {
    // one tree entry with data index 0, its name entry, and one data entry
    let mut a = b"MDB1".to_vec();
    a.extend_from_slice(&1u16.to_le_bytes());
    a.extend_from_slice(&1u16.to_le_bytes());
    a.extend_from_slice(&1u32.to_le_bytes());
    a.extend_from_slice(&0x100u32.to_le_bytes());
    a.extend_from_slice(&0x200u32.to_le_bytes());
    a.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]);
    let mut name = b"shad".to_vec();
    name.extend_from_slice(b"x\\y_vp");
    name.resize(0x40, 0);
    a.extend_from_slice(&name);
    for v in [0x10u32, 5, 5] {
        a.extend_from_slice(&v.to_le_bytes());
    }
    let e = mvgl_entries(|o, n| Ok(a[o as usize..o as usize + n].to_vec())).unwrap();
    assert_eq!(e, vec![MvglEntry { name: "x\\y_vp.shad".into(), offset: 0x110, len: 5, stored: 5 }]);
}

#[test]
fn nso_glsl_finds_sources_at_string_starts() {
    let ro = b"\0#version 310 es\nvoid main(){}\0x#version 100\0#version: no\0";
    assert_eq!(nso_glsl(ro), vec![b"#version 310 es\nvoid main(){}".to_vec()]);
}
