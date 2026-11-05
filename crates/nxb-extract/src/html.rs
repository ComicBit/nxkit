//! The offline HTML pages (`nx_extract --html`): the data of the program's HtmlDocument (Manual) NCAs, each written as a
//! folder `html<n>/` (LibHac's `Copy`, file by file). The update's NCAs come first: read alone, or as a patch of one of
//! the base's, then every base NCA of the program's title.

use std::path::Path;
use std::sync::Arc;

use crate::cache::BlockCache;
use crate::error::{ExtractError, Result};
use crate::keys::KeySet;
use crate::nca::{ContentType, Nca};
use crate::romfs::RomEntry;
use crate::storage::Storage;
use crate::version::{base_data, open_romfs};

fn copy(fs: &dyn Storage, entries: &[RomEntry], dst: &Path) -> Result<String> {
    let (mut n, mut bytes) = (0u64, 0u64);
    std::fs::create_dir_all(dst)
        .map_err(|e| ExtractError::io(format!("create {}", dst.display()), e))?;
    for e in entries {
        let op = dst.join(crate::safe_path::rel(
            "RomFS",
            &String::from_utf8_lossy(&e.path),
            false,
        )?);
        if e.dir {
            std::fs::create_dir_all(&op)
                .map_err(|x| ExtractError::io(format!("create {}", op.display()), x))?;
            continue;
        }
        if let Some(p) = op.parent() {
            std::fs::create_dir_all(p)
                .map_err(|x| ExtractError::io(format!("create {}", p.display()), x))?;
        }
        let data = fs.read_vec(e.offset, e.size as usize)?;
        std::fs::write(&op, &data)
            .map_err(|x| ExtractError::io(format!("write {}", op.display()), x))?;
        n += 1;
        bytes += e.size;
    }
    let mb = bytes as f64 / 1_048_576.0;
    Ok(format!(
        "{}: {n} files {mb:.1} MB, 0 identical to the compare dir (cloned), {mb:.1} MB newly written\n",
        dst.display()
    ))
}

fn try_copy(
    ks: &KeySet,
    base: Option<&Nca>,
    nca: &Nca,
    dst: &Path,
    cache: &Arc<BlockCache>,
) -> Result<String> {
    let r = (|| {
        let v = match base {
            None => open_romfs(ks, base_data(ks, nca, cache)?, None, cache)?,
            Some(b) => open_romfs(ks, base_data(ks, b, cache)?, Some(nca), cache)?,
        };
        copy(v.fs.as_ref(), &v.entries, dst)
    })();
    if r.is_err() && dst.exists() {
        let _ = std::fs::remove_dir_all(dst);
    }
    r
}

/// Writes `out/html<n>/` for the program `title`, returns the log lines.
pub fn write_html(
    ks: &KeySet,
    base: &[Nca],
    update: Option<&[Nca]>,
    title: u64,
    out: &Path,
    cache: &Arc<BlockCache>,
) -> Result<String> {
    let manual = |n: &&Nca| n.content_type == ContentType::Manual && n.title_id == title;
    let bases: Vec<&Nca> = base.iter().filter(manual).collect();
    let mut log = String::new();
    let mut k = 0;
    for u in update.unwrap_or(&[]).iter().filter(manual) {
        let dst = out.join(format!("html{k}"));
        let mut ok = try_copy(ks, None, u, &dst, cache);
        for b in &bases {
            if ok.is_ok() {
                break;
            }
            if matches!(ok, Err(ExtractError::Keys(_))) {
                return ok;
            }
            ok = try_copy(ks, Some(b), u, &dst, cache);
        }
        match ok {
            Ok(l) => {
                log.push_str(&l);
                k += 1;
            }
            Err(ExtractError::Keys(m)) => return Err(ExtractError::Keys(m)),
            Err(_) => log.push_str("[RED] an HtmlDocument NCA of the update could not be read\n"),
        }
    }
    for b in bases {
        let dst = out.join(format!("html{k}"));
        let v = open_romfs(ks, base_data(ks, b, cache)?, None, cache)?;
        log.push_str(&copy(v.fs.as_ref(), &v.entries, &dst)?);
        k += 1;
    }
    log.push_str(&format!("{k} HtmlDocument NCAs\n"));
    Ok(log)
}
