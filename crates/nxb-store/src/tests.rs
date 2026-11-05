//! Store tests: round trips, damage, quarantine, concurrency, locks, crash recovery.

use crate::*;
use nxb_model::{ContentKey, FileDigest};
use nxb_testkit::{Rng, check};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

static DIR_N: AtomicU32 = AtomicU32::new(0);

/// A fresh directory under the build target's tmp area (never the repository).
fn scratch() -> PathBuf {
    let base = std::env::var_os("NXB_TEST_TMP")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let d = base.join(format!(
        "nxb-store-test-{}-{}",
        std::process::id(),
        DIR_N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn key(s: &str) -> ContentKey {
    ContentKey::builder("nxb.test.v1")
        .field_str("k", s)
        .finish()
}

fn count_files(dir: &Path) -> usize {
    let mut n = 0;
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                n += count_files(&p);
            } else {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn blob_round_trip_and_dedupe() {
    let root = scratch();
    let s = Store::open(&root).unwrap();
    let a = s.put_bytes(b"hello").unwrap();
    let b = s.put_bytes(b"hello").unwrap();
    let c = s.put_reader(&b"hello"[..]).unwrap();
    assert_eq!(a, b);
    assert_eq!(a, c);
    assert_eq!(s.read_blob(&a.sha256).unwrap(), b"hello");
    assert_eq!(count_files(&root.join("blobs")), 1);
    assert_eq!(count_files(&root.join("tmp")), 0);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn entry_publish_lookup_restore() {
    let root = scratch();
    let s = Store::open(&root).unwrap();
    let mut w = s.entry(key("e"));
    w.add_bytes("a.metallib", b"lib").unwrap();
    w.add_bytes("dir/b.json", b"{}").unwrap();
    let m = w.commit().unwrap();
    assert_eq!(s.lookup(&key("e")).unwrap(), Some(m.clone()));
    assert_eq!(s.lookup(&key("other")).unwrap(), None);
    let out = root.join("out");
    s.restore(&m, &out).unwrap();
    assert_eq!(fs::read(out.join("a.metallib")).unwrap(), b"lib");
    assert_eq!(fs::read(out.join("dir/b.json")).unwrap(), b"{}");
    assert!(matches!(s.verify(&key("e")).unwrap(), Verified::Intact(_)));
    // Reopen: same format, same content.
    let s2 = Store::open(&root).unwrap();
    assert_eq!(s2.lookup(&key("e")).unwrap(), Some(m));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn add_refuses_unknown_blobs_and_bad_names() {
    let root = scratch();
    let s = Store::open(&root).unwrap();
    let mut w = s.entry(key("x"));
    assert!(matches!(
        w.add("a", FileDigest::of(b"never stored")),
        Err(StoreError::MissingBlob(_))
    ));
    assert!(matches!(
        w.add_bytes("../escape", b"x"),
        Err(StoreError::BadName(_))
    ));
    w.add_bytes("a", b"1").unwrap();
    assert!(w.add_bytes("a", b"2").is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn truncated_blob_is_quarantined() {
    let root = scratch();
    let s = Store::open(&root).unwrap();
    let mut w = s.entry(key("t"));
    let d = w.add_bytes("f", b"0123456789").unwrap();
    w.commit().unwrap();
    fs::write(s.blob_path(&d.sha256), b"01234").unwrap();
    assert_eq!(s.lookup(&key("t")).unwrap(), None);
    assert_eq!(s.quarantined(), 2);
    assert_eq!(s.lookup(&key("t")).unwrap(), None);
    // Evidence kept: blob and manifest in quarantine, nothing deleted.
    assert_eq!(count_files(&root.join("quarantine")), 4);
    // Publishing again repairs it.
    let mut w = s.entry(key("t"));
    w.add_bytes("f", b"0123456789").unwrap();
    w.commit().unwrap();
    assert!(s.lookup(&key("t")).unwrap().is_some());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn flipped_byte_found_by_verify_only() {
    let root = scratch();
    let s = Store::open(&root).unwrap();
    let mut w = s.entry(key("f"));
    let d = w.add_bytes("f", b"0123456789").unwrap();
    w.commit().unwrap();
    fs::write(s.blob_path(&d.sha256), b"0123456780").unwrap();
    assert!(
        s.lookup(&key("f")).unwrap().is_some(),
        "same size passes the cheap check"
    );
    assert!(matches!(
        s.verify(&key("f")).unwrap(),
        Verified::Quarantined(_)
    ));
    assert_eq!(s.lookup(&key("f")).unwrap(), None);
    assert_eq!(s.verify(&key("f")).unwrap(), Verified::Missing);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn damaged_manifest_is_quarantined() {
    let root = scratch();
    let s = Store::open(&root).unwrap();
    let mut w = s.entry(key("m"));
    w.add_bytes("f", b"x").unwrap();
    w.commit().unwrap();
    let p = s.entry_path(&key("m"));
    let mut b = fs::read(&p).unwrap();
    let i = b.len() / 2;
    b[i] ^= 0x20;
    fs::write(&p, b).unwrap();
    assert_eq!(s.lookup(&key("m")).unwrap(), None);
    assert_eq!(s.quarantined(), 1);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn foreign_format_refused() {
    let root = scratch();
    fs::write(root.join("FORMAT"), "something else\n").unwrap();
    assert!(matches!(
        Store::open(&root),
        Err(StoreError::WrongFormat(_))
    ));
    fs::remove_dir_all(root).unwrap();
}

/// Property: many threads storing overlapping random blobs and entries, every result is readable, the blob count
/// equals the number of distinct payloads, no temp file is left.
#[test]
fn concurrent_writers() {
    check(71, 2, |r| {
        let root = scratch();
        let s = Store::open(&root).unwrap();
        let payloads: Vec<Vec<u8>> = (0..20)
            .map(|_| {
                let n = r.below(3000) as usize;
                r.bytes(n)
            })
            .collect();
        let seed = r.next_u64();
        let used: Vec<usize> = std::thread::scope(|sc| {
            let handles: Vec<_> = (0..8u64)
                .map(|t| {
                    let (s, payloads) = (&s, &payloads);
                    sc.spawn(move || {
                        let mut rr = Rng::new(seed ^ t);
                        let mut used = Vec::new();
                        for _ in 0..40 {
                            let i = rr.below(payloads.len() as u64) as usize;
                            let d = s.put_bytes(&payloads[i]).unwrap();
                            assert_eq!(s.read_blob(&d.sha256).unwrap(), payloads[i]);
                            let mut w = s.entry(key(&format!("p{i}")));
                            w.add("data", d).unwrap();
                            w.commit().unwrap();
                            used.push(i);
                        }
                        used
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap())
                .collect()
        });
        let mut distinct: Vec<&Vec<u8>> = used.iter().map(|&i| &payloads[i]).collect();
        distinct.sort();
        distinct.dedup();
        assert_eq!(count_files(&root.join("blobs")), distinct.len());
        for (i, p) in payloads.iter().enumerate() {
            if let Some(m) = s.lookup(&key(&format!("p{i}"))).unwrap() {
                assert_eq!(&s.read_blob(&m.files[0].digest.sha256).unwrap(), p);
            }
        }
        assert_eq!(count_files(&root.join("tmp")), 0);
        assert_eq!(s.quarantined(), 0);
        fs::remove_dir_all(root).unwrap();
    });
}

#[test]
fn root_lock_shared_and_exclusive() {
    let root = scratch();
    let a = Store::open(&root).unwrap();
    let b = Store::open(&root).unwrap();
    assert!(matches!(
        Store::open_exclusive(&root, false),
        Err(StoreError::Locked(_))
    ));
    drop((a, b));
    let x = Store::open_exclusive(&root, false).unwrap();
    assert_eq!(x.recovered(), 0, "clean drops leave no journal");
    drop(x);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn output_dir_lock_is_exclusive() {
    let root = scratch();
    let out = root.join("b-1234");
    let g = lock_output_dir(&out, ".shaderbuild.lock", false).unwrap();
    let e = lock_output_dir(&out, ".shaderbuild.lock", false).unwrap_err();
    assert!(matches!(e, StoreError::Locked(_)));
    assert_eq!(e.class(), nxb_error::ErrorClass::Build);
    assert!(e.to_string().starts_with("another build is using "));
    drop(g);
    assert!(lock_output_dir(&out, ".shaderbuild.lock", false).is_ok());
    fs::remove_dir_all(root).unwrap();
}

/// A dead writer's journal: its damaged blob and entry go to quarantine, its temp files go, the journal goes.
/// a live store's journal is left alone.
#[test]
fn recovery_after_unclean_end() {
    let root = scratch();
    let (blob, live_journals) = {
        let s = Store::open(&root).unwrap();
        let mut w = s.entry(key("crash"));
        let d = w.add_bytes("f", b"0123456789").unwrap();
        w.commit().unwrap();
        // Same size, other bytes: only a re-hash finds it.
        fs::write(s.blob_path(&d.sha256), b"9876543210").unwrap();
        (d, fs::read_dir(root.join("dirty")).unwrap().count())
    };
    assert_eq!(live_journals, 1);
    // Fake the dead process: a journal nobody locks, and a temp file with its token.
    let token = "99999-1";
    fs::write(
        root.join("dirty").join(format!("{token}-j0.journal")),
        format!("blob {}\nentry {}\n", blob.sha256, key("crash").to_hex()),
    )
    .unwrap();
    fs::write(root.join("tmp").join(format!("{token}-7.tmp")), b"partial").unwrap();
    let live = Store::open(&root).unwrap();
    let s = Store::open(&root).unwrap();
    assert_eq!(s.recovered() + live.recovered(), 1);
    assert_eq!(s.lookup(&key("crash")).unwrap(), None);
    assert!(!s.blob_path(&blob.sha256).exists());
    assert_eq!(count_files(&root.join("tmp")), 0);
    // Only the two live journals remain.
    assert_eq!(fs::read_dir(root.join("dirty")).unwrap().count(), 2);
    drop((s, live));
    assert_eq!(fs::read_dir(root.join("dirty")).unwrap().count(), 0);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn relaxed_durability_round_trip() {
    let root = scratch();
    let mut s = Store::open(&root).unwrap();
    s.set_durability(Durability::Relaxed);
    let mut w = s.entry(key("relaxed"));
    w.add_bytes("f", b"data").unwrap();
    let m = w.commit().unwrap();
    let r = s.put_reader(&b"stream"[..]).unwrap();
    assert_eq!(s.lookup(&key("relaxed")).unwrap(), Some(m));
    assert_eq!(s.read_blob(&r.sha256).unwrap(), b"stream");
    drop(s);
    fs::remove_dir_all(root).unwrap();
}
