//! The survey's input from a RomFS image: `romfs_manifest.tsv` (path, size, first 32 bytes of every file, in path
//! order) and the NRO modules under `romfs_nro/`, as `nx_extract --manifest` leaves them (`manifest::write` of
//! `nxb-extract`). A NUP package has no manifest mode, so its `RomFS.img` is listed.

use nxb_extract::romfs::read_tables;
use nxb_extract::storage::{FileStorage, Storage};
use std::path::Path;

/// The files of a `DMROMFS1` image (`RomFS.img`, `dm/tools/romfs.py` `Image`): `(path, size, offset in the image)`.
fn dm_image_files(img: &Path) -> Result<Vec<nxb_extract::romfs::RomEntry>, String> {
    use std::io::{Read, Seek, SeekFrom};
    let bad = |m: &str| format!("invalid RomFS image {m}: {}", img.display());
    let mut f = std::fs::File::open(img).map_err(|e| format!("{}: {e}", img.display()))?;
    let total = f.metadata().map_err(|e| e.to_string())?.len();
    let mut h = [0u8; 32];
    f.read_exact(&mut h).map_err(|_| bad("header"))?;
    let u64at = |b: &[u8], o: usize| u64::from_le_bytes(b[o..o + 8].try_into().unwrap_or([0; 8]));
    let (off, size, count) = (u64at(&h, 8), u64at(&h, 16), u64at(&h, 24) as usize);
    if &h[..8] != b"DMROMFS1"
        || count == 0
        || off < 32
        || off > total
        || size != total - off
        || count as u64 > size / 24
    {
        return Err(bad("header"));
    }
    f.seek(SeekFrom::Start(off)).map_err(|e| e.to_string())?;
    let mut idx = vec![0u8; size as usize];
    f.read_exact(&mut idx).map_err(|_| bad("index"))?;
    let pool = count * 24;
    let ent = |k: usize| {
        let e = &idx[k * 24..k * 24 + 24];
        (
            u64at(e, 0),
            u64at(e, 8),
            u32::from_le_bytes(e[16..20].try_into().unwrap_or([0; 4])) as usize,
            u16::from_le_bytes([e[20], e[21]]) as usize,
            e[22],
        )
    };
    let mut paths: Vec<Vec<u8>> = vec![Vec::new(); count];
    let mut out = Vec::new();
    for k in 0..count {
        let (a, b, no, nl, t) = ent(k);
        if t == 0 {
            let (a, b) = (a as usize, b as usize);
            if a > count || b > count - a {
                return Err(bad("tree"));
            }
            for j in a..a + b {
                let (_, _, jo, jl, _) = ent(j);
                let name = idx
                    .get(pool + jo..pool + jo + jl)
                    .ok_or_else(|| bad("name"))?;
                let mut p = paths[k].clone();
                p.push(b'/');
                p.extend_from_slice(name);
                paths[j] = p;
            }
        } else {
            let _ = (no, nl);
            out.push(nxb_extract::romfs::RomEntry {
                path: paths[k].clone(),
                dir: false,
                size: b,
                offset: a,
            });
        }
    }
    Ok(out)
}

/// Lists the RomFS image `img` (a `DMROMFS1` image or a RomFS partition) into `out`. Returns the number of files.
pub fn manifest_of_image(img: &Path, out: &Path) -> Result<usize, String> {
    let s = FileStorage::open(img).map_err(|e| e.to_string())?;
    let mut magic = [0u8; 8];
    let dm = s.size() >= 8 && s.read_at(0, &mut magic).is_ok() && &magic == b"DMROMFS1";
    let mut files: Vec<_> = if dm {
        dm_image_files(img)?
    } else {
        read_tables(&s)
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|e| !e.dir)
            .collect()
    };
    files.sort_by(|a, b| a.path.cmp(&b.path));
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut text = String::from("# path\tbytes\tfirst 32 bytes (hex)\n");
    for e in &files {
        let shown = String::from_utf8_lossy(&e.path).into_owned();
        let mut head = vec![0u8; e.size.min(32) as usize];
        s.read_at(e.offset, &mut head).map_err(|x| x.to_string())?;
        text.push_str(&format!(
            "{shown}\t{}\t{}\n",
            e.size,
            nxb_model::to_hex(&head)
        ));
        if shown.to_ascii_lowercase().ends_with(".nro") {
            let op = out.join("romfs_nro").join(shown.trim_start_matches('/'));
            if let Some(d) = op.parent() {
                std::fs::create_dir_all(d).map_err(|x| format!("{}: {x}", d.display()))?;
            }
            let data = s
                .read_vec(e.offset, e.size as usize)
                .map_err(|x| x.to_string())?;
            std::fs::write(&op, data).map_err(|x| format!("{}: {x}", op.display()))?;
        }
    }
    std::fs::write(out.join("romfs_manifest.tsv"), text).map_err(|e| e.to_string())?;
    Ok(files.len())
}
