//! Parity on real compressed packages (skipped unless `NXB_NSZ_REAL` names an `.nsz` and `NXB_NSZ_REF` the `.nsp`
//! that `nsz -D` made of it): every NCZ shows exactly the bytes of the reference NCA (SHA-256 over both, streamed).
//! `NXB_TMP` names the folder for the decoded solid streams (they are as big as the NCA).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use nxb_extract::nca::read_pfs0;
use nxb_extract::ncz::NczStorage;
use nxb_extract::storage::{FileStorage, Storage, SubStorage};
use nxb_model::Sha256;

fn digest(s: &dyn Storage) -> (u64, String) {
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 8 << 20];
    let mut at = 0u64;
    while at < s.size() {
        let n = (s.size() - at).min(buf.len() as u64) as usize;
        s.read_at(at, &mut buf[..n]).unwrap();
        h.update(&buf[..n]);
        at += n as u64;
    }
    (s.size(), h.finalize().to_hex())
}

#[test]
fn real_nsz_shows_the_ncas_nsz_d_wrote() {
    let (Some(nsz), Some(reference)) = (
        std::env::var_os("NXB_NSZ_REAL"),
        std::env::var_os("NXB_NSZ_REF"),
    ) else {
        return;
    };
    if let Some(t) = std::env::var_os("NXB_TMP") {
        nxb_extract::ncz::set_spill_dir(std::path::Path::new(&t));
    }
    let a: Arc<dyn Storage> = Arc::new(FileStorage::open(std::path::Path::new(&nsz)).unwrap());
    let b: Arc<dyn Storage> =
        Arc::new(FileStorage::open(std::path::Path::new(&reference)).unwrap());
    let (ea, eb) = (
        read_pfs0(a.as_ref()).unwrap(),
        read_pfs0(b.as_ref()).unwrap(),
    );
    let mut checked = 0;
    for e in ea.iter().filter(|e| e.name.ends_with(".ncz")) {
        let stem = e.name.trim_end_matches(".ncz");
        let r = eb
            .iter()
            .find(|r| r.name == format!("{stem}.nca"))
            .expect("the reference has the NCA");
        let raw: Arc<dyn Storage> = Arc::new(SubStorage::new(a.clone(), e.offset, e.size).unwrap());
        let ncz = NczStorage::open(raw, stem).unwrap();
        let refs = SubStorage::new(b.clone(), r.offset, r.size).unwrap();
        let (x, y) = (digest(&ncz), digest(&refs));
        assert_eq!(x, y, "NCA {stem}");
        checked += 1;
        eprintln!("NCZ {stem}: {} bytes identical", x.0);
    }
    assert!(checked > 0, "no NCZ in the package");
}
