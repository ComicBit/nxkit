//! Cache tests: key, export and import round trip, integrity on import, filter, size cap and LRU.

use crate::*;
use nxb_testkit::check;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime};

static N: AtomicU32 = AtomicU32::new(0);

fn scratch() -> PathBuf {
    let base = std::env::var_os("NXB_TEST_TMP")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let d = base.join(format!(
        "nxb-cache-test-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn key(i: u32, family: &str) -> CacheKey {
    CacheKey {
        shader_key: format!("{i:016x}"),
        translator: "xlate-1".into(),
        emitter: "emit-3".into(),
        device_family: family.into(),
        variant: "base".into(),
    }
}

fn fill(s: &Store, n: u32) {
    for i in 0..n {
        let payload = vec![i as u8; 100 + i as usize];
        let fam = if i % 2 == 0 { "apple9" } else { "apple7" };
        s.put_cache(
            &key(i, fam),
            &[("lib.metallib", &payload), ("same.bin", b"shared")],
        )
        .unwrap();
    }
}

fn export(s: &Store, f: &Filter) -> Vec<u8> {
    let mut v = Vec::new();
    s.export_cache(f, &mut v).unwrap();
    v
}

#[test]
fn key_text_round_trip_and_every_field_counts() {
    let k = key(1, "apple9");
    assert_eq!(CacheKey::parse(&k.to_text()), Some(k.clone()));
    let mut seen = vec![k.content_key()];
    for f in 0..5 {
        let mut o = k.clone();
        let t = match f {
            0 => &mut o.shader_key,
            1 => &mut o.translator,
            2 => &mut o.emitter,
            3 => &mut o.device_family,
            _ => &mut o.variant,
        };
        t.push('x');
        assert!(!seen.contains(&o.content_key()));
        seen.push(o.content_key());
    }
    let mut bad = k;
    bad.variant = "a\nb".into();
    assert!(bad.check().is_err());
}

#[test]
fn export_import_round_trip_is_deterministic() {
    let (a, b, c) = (scratch(), scratch(), scratch());
    let sa = Store::open(&a).unwrap();
    fill(&sa, 6);
    let bytes = export(&sa, &Filter::default());
    let sb = Store::open(&b).unwrap();
    let rep = sb.import_cache(&bytes[..]).unwrap();
    assert_eq!((rep.added, rep.present, rep.blobs), (6, 0, 13));
    for i in 0..6 {
        let fam = if i % 2 == 0 { "apple9" } else { "apple7" };
        let m = sb.get_cache(&key(i, fam)).unwrap().unwrap();
        assert_eq!(
            sb.verify(&key(i, fam).content_key()).unwrap(),
            Verified::Intact(m.clone())
        );
        let lib = m.files.iter().find(|f| f.name == "lib.metallib").unwrap();
        assert_eq!(
            sb.read_blob(&lib.digest.sha256).unwrap(),
            vec![i as u8; 100 + i as usize]
        );
    }
    assert_eq!(
        export(&sb, &Filter::default()),
        bytes,
        "same entries, same bytes"
    );
    assert_eq!(sb.import_cache(&bytes[..]).unwrap().present, 6);
    // an empty store exports and imports
    let sc = Store::open(&c).unwrap();
    let empty = export(&sc, &Filter::default());
    assert_eq!(sb.import_cache(&empty[..]).unwrap().added, 0);
}

#[test]
fn filter_selects_device_family() {
    let a = scratch();
    let s = Store::open(&a).unwrap();
    fill(&s, 6);
    let f = Filter {
        device_family: Some("apple9".into()),
        ..Default::default()
    };
    let list = s.list_cache(&f).unwrap();
    assert_eq!(list.len(), 3);
    assert!(list.iter().all(|e| e.key.device_family == "apple9"));
    let b = scratch();
    let sb = Store::open(&b).unwrap();
    assert_eq!(sb.import_cache(&export(&s, &f)[..]).unwrap().added, 3);
    assert_eq!(sb.list_cache(&Filter::default()).unwrap().len(), 3);
}

#[test]
fn import_rejects_damage_and_adds_nothing() {
    let (a, b) = (scratch(), scratch());
    let sa = Store::open(&a).unwrap();
    fill(&sa, 3);
    let bytes = export(&sa, &Filter::default());
    let sb = Store::open(&b).unwrap();
    check(77, 60, |r| {
        let mut bad = bytes.clone();
        let i = r.below(bad.len() as u64) as usize;
        bad[i] ^= 1 + r.below(255) as u8;
        assert!(sb.import_cache(&bad[..]).is_err(), "flip at {i} accepted");
        assert!(
            sb.entry_keys().unwrap().is_empty(),
            "flip at {i} left an entry"
        );
    });
    for cut in [0, 10, bytes.len() / 2, bytes.len() - 1] {
        assert!(sb.import_cache(&bytes[..cut]).is_err(), "cut at {cut}");
    }
    let mut more = bytes.clone();
    more.push(b'\n');
    assert!(sb.import_cache(&more[..]).is_err());
    assert!(sb.entry_keys().unwrap().is_empty());
    assert!(sb.import_cache(&bytes[..]).is_ok());
}

#[test]
fn prune_removes_least_recently_used_to_fit_the_cap() {
    let a = scratch();
    {
        let s = Store::open(&a).unwrap();
        fill(&s, 5);
        // age: entry 0 oldest ... entry 4 newest, then use entry 0 so it becomes the newest
        let now = SystemTime::now();
        for i in 0..5u32 {
            let fam = if i % 2 == 0 { "apple9" } else { "apple7" };
            let p = s.entry_path_for_test(&key(i, fam));
            let f = std::fs::OpenOptions::new().write(true).open(p).unwrap();
            f.set_modified(now - Duration::from_secs(1000 - 10 * i as u64))
                .unwrap();
        }
        assert!(s.get_cache(&key(0, "apple9")).unwrap().is_some());
        assert!(
            matches!(s.prune(0), Err(StoreError::Locked(_))),
            "needs the exclusive lock"
        );
    }
    let s = Store::open_exclusive(&a, false).unwrap();
    let before = s.list_cache(&Filter::default()).unwrap();
    let total: u64 = {
        let mut u = std::collections::BTreeMap::new();
        for e in &before {
            for f in &e.manifest.files {
                u.insert(f.digest.sha256, f.digest.len);
            }
        }
        u.values().sum()
    };
    assert_eq!(
        s.prune(total).unwrap(),
        PruneReport {
            kept_bytes: total,
            ..Default::default()
        }
    );
    // room for about three entries: the two oldest after the touch (1 and 2) go first
    let rep = s.prune(total - 150).unwrap();
    assert!(
        rep.entries >= 1 && rep.blobs >= 1 && rep.kept_bytes <= total - 150,
        "{rep:?}"
    );
    assert!(
        s.get_cache(&key(0, "apple9")).unwrap().is_some(),
        "the used entry stays"
    );
    assert!(
        s.get_cache(&key(1, "apple7")).unwrap().is_none(),
        "the oldest goes first"
    );
    let rep = s.prune(0).unwrap();
    assert!(rep.entries >= 1);
    assert!(s.list_cache(&Filter::default()).unwrap().is_empty());
    assert_eq!(s.prune(0).unwrap().kept_bytes, 0);
}
