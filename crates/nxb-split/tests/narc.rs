//! NARC members of every packing kind: `narc.bin` is a synthetic container, `narc.exp` the rows (member, length,
//! SHA-256 prefix) the reference `cra.undo` gives for its members.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use nxb_model::sha256;
use nxb_split::narc::{narc_members, undo};

#[test]
fn members_unpack_as_the_reference_does() {
    let d = include_bytes!("data/narc.bin");
    let want = include_str!("data/narc.exp");
    let got: String = narc_members(d, "t")
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let u = undo(m).unwrap();
            format!("{i}\t{}\t{}\n", u.len(), &sha256(&u).to_hex()[..16])
        })
        .collect();
    assert_eq!(got, want);
}

#[test]
fn damaged_containers_are_errors() {
    assert!(narc_members(b"NARX", "t").is_err());
    let d = include_bytes!("data/narc.bin");
    assert!(narc_members(&d[..40], "t").is_err());
}
