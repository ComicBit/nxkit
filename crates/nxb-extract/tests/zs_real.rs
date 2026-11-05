//! Real zstd dictionaries (a title that ships them): skipped unless `NXB_ZS_KEYS`, `NXB_ZS_BASE` (base NSP) and
//! `NXB_ZS_UPDATE` (update NSP, optional) are set. Files of the update's RomFS that name a dictionary decode with
//! `Pack/ZsDic.pack.zs` to the bytes the `zstd` command gives with the same dictionary file.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use nxb_extract::cache::BlockCache;
use nxb_extract::keys::KeySet;
use nxb_extract::nca::open_package;
use nxb_extract::storage::{FileStorage, Storage};
use nxb_extract::version::{base_data, open_romfs, pick_programs};

#[test]
fn dictionary_files_equal_the_zstd_command() {
    let (Some(keys), Some(base)) = (
        std::env::var_os("NXB_ZS_KEYS"),
        std::env::var_os("NXB_ZS_BASE"),
    ) else {
        return;
    };
    let mut ks = KeySet::parse(&std::fs::read_to_string(keys).unwrap());
    let cache = Arc::new(BlockCache::new(256));
    let open = |ks: &mut KeySet, p: &std::ffi::OsStr| {
        let s: Arc<dyn Storage> = Arc::new(FileStorage::open(std::path::Path::new(p)).unwrap());
        open_package(ks, s, "pkg").unwrap()
    };
    let b = open(&mut ks, &base);
    let u = std::env::var_os("NXB_ZS_UPDATE").map(|p| open(&mut ks, &p));
    let (bp, up, _) = pick_programs(&b, u.as_deref()).unwrap();
    let fs = open_romfs(&ks, base_data(&ks, bp, &cache).unwrap(), up, &cache).unwrap();
    let pack = nxb_extract::romfs::read_file(fs.fs.as_ref(), &fs.entries, "/Pack/ZsDic.pack.zs")
        .unwrap()
        .expect("Pack/ZsDic.pack.zs");
    let sarc = nxb_split::zstd::decompress(&pack).unwrap();
    let members = nxb_split::shaders::sarc_members(&sarc, "pack").unwrap();
    let mut dicts = nxb_split::zstd::Dicts::new();
    let dir = std::env::temp_dir().join(format!("nxb-zs-real-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut files = std::collections::HashMap::new();
    for (_, m) in members
        .iter()
        .filter(|(_, m)| m.starts_with(&[0x37, 0xa4, 0x30, 0xec]))
    {
        let id = dicts.add(m).unwrap();
        std::fs::write(dir.join(format!("d{id}")), m).unwrap();
        files.insert(id, 0usize);
    }
    assert!(!files.is_empty());
    let mut checked = 0;
    for e in fs.entries.iter().filter(|e| !e.dir && e.size < (8 << 20)) {
        let raw = fs.fs.read_vec(e.offset, e.size as usize).unwrap();
        let id = nxb_split::zstd::dict_id(&raw);
        if id == 0 || files.get(&id).is_none_or(|n| *n >= 3) {
            continue;
        }
        *files.get_mut(&id).unwrap() += 1;
        std::fs::write(dir.join("in.zs"), &raw).unwrap();
        let want = std::process::Command::new("zstd")
            .current_dir(&dir)
            .args(["-d", "-q", "-c", "-D", &format!("d{id}"), "in.zs"])
            .output()
            .unwrap();
        assert!(
            want.status.success(),
            "zstd failed on {}",
            String::from_utf8_lossy(&e.path)
        );
        let got = nxb_split::zstd::decompress_dict(&raw, &dicts).unwrap();
        assert!(got == want.stdout, "{}", String::from_utf8_lossy(&e.path));
        checked += 1;
    }
    eprintln!("dictionaries {:?}, files checked {checked}", files);
    assert!(checked > 0);
    let _ = std::fs::remove_dir_all(&dir);
}
