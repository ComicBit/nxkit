//! `nx_extract --manifest`: the RomFS is listed, not written. `romfs_manifest.tsv` has one line per file (path, size,
//! first 32 bytes in hex) and the NRO modules (code the game loads at run time) are copied under `romfs_nro/`.
//! Files are listed in path order. A file whose first block fails its hash check is listed with no bytes, named in
//! the returned list and not copied.

use std::fmt::Write as _;
use std::path::Path;

use crate::error::{ExtractError, Result};
use crate::storage::Storage;
use crate::version::VersionFs;

/// Writes the manifest of `fs` into `out_dir`. Returns the summary line and the paths that failed their hash check.
pub fn write(fs: &VersionFs, out_dir: &Path) -> Result<(String, Vec<String>)> {
    std::fs::create_dir_all(out_dir)
        .map_err(|e| ExtractError::io(format!("create {}", out_dir.display()), e))?;
    let path = out_dir.join("romfs_manifest.tsv");
    let mut files: Vec<&crate::romfs::RomEntry> = fs.entries.iter().filter(|e| !e.dir).collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let mut text = String::from("# path\tbytes\tfirst 32 bytes (hex)\n");
    let (mut n, mut bytes, mut damaged) = (0u64, 0u64, Vec::new());
    for e in files {
        let shown = String::from_utf8_lossy(&e.path).into_owned();
        let want = e.size.min(32) as usize;
        let mut head = vec![0u8; want];
        let ok = want == 0 || {
            match fs.fs.read_at(e.offset, &mut head) {
                Ok(()) => true,
                Err(ExtractError::Damaged(_)) => {
                    damaged.push(shown.clone());
                    head.clear();
                    false
                }
                Err(x) => return Err(x),
            }
        };
        let _ = writeln!(text, "{shown}\t{}\t{}", e.size, nxb_model::to_hex(&head));
        n += 1;
        bytes += e.size;
        if ok && shown.to_ascii_lowercase().ends_with(".nro") {
            let rel = crate::safe_path::rel("RomFS", &shown, false)?;
            let op = out_dir.join("romfs_nro").join(rel);
            match fs.fs.read_vec(e.offset, e.size as usize) {
                Ok(data) => {
                    if let Some(d) = op.parent() {
                        std::fs::create_dir_all(d)
                            .map_err(|x| ExtractError::io(format!("create {}", d.display()), x))?;
                    }
                    std::fs::write(&op, &data)
                        .map_err(|x| ExtractError::io(format!("write {}", op.display()), x))?;
                }
                Err(ExtractError::Damaged(_)) => damaged.push(shown),
                Err(x) => return Err(x),
            }
        }
    }
    std::fs::write(&path, text)
        .map_err(|e| ExtractError::io(format!("write {}", path.display()), e))?;
    let line = format!(
        "{}: RomFS {n} files {:.1} MB (manifest only)",
        path.display(),
        bytes as f64 / 1_048_576.0
    );
    Ok((line, damaged))
}
