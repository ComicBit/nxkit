//! Tests that need no game data.

use nxb_rewrite::{Options, RewriteError, patches::PatchTable, rewrite};

#[test]
fn patch_table_parses_rows() {
    let t = PatchTable::parse(
        "file\ta.py\tab\ncuts\tb1\t16\t00000000\t1f2003d5\tnop out\nframe\tb2\ncutsempty\tb3\nrule\tsdk\tsdk\tnode\t0\t0\t240\taa,bb\nrcut\tsdk\tsdk\tnode\t0\t0\t4\t00000000\t1f2003d5\twhy\n",
    )
    .unwrap_or_default();
    assert_eq!(t.cuts["b1"][0].off, 16);
    assert!(t.cuts["b3"].is_empty());
    assert!(t.frame.contains("b2"));
    assert_eq!(t.rules["sdk"]["sdk"][0].fns[0].fps, ["aa", "bb"]);
}

#[test]
fn garbage_is_refused_not_panicked() {
    let r = rewrite(&[0u8; 64], &Options::default(), &PatchTable::default());
    assert!(matches!(r, Err(RewriteError::Failed(m)) if m.contains("not an NSO")));
}
