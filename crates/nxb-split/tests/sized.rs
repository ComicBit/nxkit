//! Size-prefixed frames against the reference `unsized` (the `.exp` rows: kind, length, SHA-256 prefix, the note
//! the reference prints). The LZ4 frames use independent and linked blocks of 64 KiB and 4 MiB.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use nxb_model::sha256;
use nxb_split::sized::{Sized, unsized_frame};
use nxb_split::zstd::Dicts;

fn row(d: &[u8]) -> (String, String) {
    let (kind, bytes, note) = match unsized_frame(d, &Dicts::new()).unwrap() {
        Sized::Opened(v) => ("opened", v, String::new()),
        Sized::AsIs(why) => ("asis", d.to_vec(), why.unwrap_or_default()),
    };
    (
        format!(
            "{kind}\t{}\t{}",
            bytes.len(),
            &sha256(&bytes).to_hex()[..16]
        ),
        note,
    )
}

macro_rules! case {
    ($name:ident, $file:literal, exact_note) => {
        #[test]
        fn $name() {
            let d = include_bytes!(concat!("data/sz_", $file, ".bin"));
            let exp = include_str!(concat!("data/sz_", $file, ".exp"));
            let (head, note) = row(d);
            let (want, wnote) = exp
                .trim_end()
                .split_once("\t[RED] /x: a size-prefixed frame that did not open (")
                .map_or((exp.trim_end().trim_end_matches('\t'), ""), |(a, b)| {
                    (a, b.trim_end_matches("): the file is read as it is"))
                });
            assert_eq!(head, want);
            assert_eq!(note, wnote);
        }
    };
    ($name:ident, $file:literal) => {
        #[test]
        fn $name() {
            let d = include_bytes!(concat!("data/sz_", $file, ".bin"));
            let exp = include_str!(concat!("data/sz_", $file, ".exp"));
            let (head, _) = row(d);
            assert_eq!(head, exp.split('\t').take(3).collect::<Vec<_>>().join("\t"));
        }
    };
}

case!(zstd_with_a_recorded_size, "zstd_sized");
case!(zstd_without_a_size, "zstd_nosize");
case!(zstd_bad_prefix, "zstd_nosize_badprefix", exact_note);
case!(zstd_other_size_is_not_a_container, "zstd_size_mismatch");
case!(lz4_independent_blocks, "lz4_independent");
case!(lz4_linked_blocks, "lz4_linked");
case!(lz4_linked_4m_with_checksums, "lz4_linked_4m");
case!(lz4_truncated_is_read_as_is, "lz4_trunc");
case!(not_a_frame, "plain");
