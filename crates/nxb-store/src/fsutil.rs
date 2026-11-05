//! Durable file writes: temp name with `O_EXCL`, `fsync`, rename, directory `fsync`.

use crate::error::StoreError;
use nxb_model::{FileDigest, HashingWriter};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// This process's token, `<pid>-<start nanos>`: prefix of every temp file and journal it creates, so leftovers of a
/// dead process can be told apart from a live one that reuses its pid.
pub(crate) fn process_token() -> &'static str {
    static TOKEN: OnceLock<String> = OnceLock::new();
    TOKEN.get_or_init(|| {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("{}-{nanos}", std::process::id())
    })
}

/// A unique temp path in `dir`: `<process token>-<counter>.tmp`.
pub(crate) fn temp_path(dir: &Path) -> PathBuf {
    let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    dir.join(format!("{}-{n}.tmp", process_token()))
}

/// `fsync` a directory so a rename in it is durable. No-op where directories cannot be opened (Windows).
pub(crate) fn sync_dir(dir: &Path) -> Result<(), StoreError> {
    #[cfg(unix)]
    {
        File::open(dir)
            .and_then(|f| f.sync_all())
            .map_err(|e| StoreError::io("sync directory", dir, e))?;
    }
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

/// Create `dir` and its parents.
pub(crate) fn ensure_dir(dir: &Path) -> Result<(), StoreError> {
    fs::create_dir_all(dir).map_err(|e| StoreError::io("create directory", dir, e))
}

/// Stream `src` into a new temp file in `tmp_dir`, hashing on the way.`fsync` it when `sync`. Returns the temp path and digest.
pub(crate) fn write_temp_hashed(
    tmp_dir: &Path,
    mut src: impl Read,
    sync: bool,
) -> Result<(PathBuf, FileDigest), StoreError> {
    let tmp = temp_path(tmp_dir);
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|e| StoreError::io("create temp", &tmp, e))?;
    let mut w = HashingWriter::new(io::BufWriter::with_capacity(256 * 1024, file));
    let res = io::copy(&mut src, &mut w).and_then(|_| w.flush());
    if let Err(e) = res {
        let _ = fs::remove_file(&tmp);
        return Err(StoreError::io("write temp", &tmp, e));
    }
    let (buf, digest) = w.finish();
    let file = buf
        .into_inner()
        .map_err(|e| StoreError::io("flush temp", &tmp, e.into_error()))?;
    if let Err(e) = if sync { file.sync_all() } else { Ok(()) } {
        let _ = fs::remove_file(&tmp);
        return Err(StoreError::io("fsync temp", &tmp, e));
    }
    Ok((tmp, digest))
}

/// Write bytes whose digest is already known to a new temp file in `tmp_dir`, `fsync` it when `sync` (no second hash).
pub(crate) fn write_temp_plain(
    tmp_dir: &Path,
    bytes: &[u8],
    sync: bool,
) -> Result<PathBuf, StoreError> {
    let tmp = temp_path(tmp_dir);
    let res = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .and_then(|mut f| {
            f.write_all(bytes)?;
            if sync { f.sync_all() } else { Ok(()) }
        });
    if let Err(e) = res {
        let _ = fs::remove_file(&tmp);
        return Err(StoreError::io("write temp", &tmp, e));
    }
    Ok(tmp)
}

/// Rename `tmp` to `dst` (creating `dst`'s directory) and `fsync` the directory when `sync`.
pub(crate) fn commit_rename(tmp: &Path, dst: &Path, sync: bool) -> Result<(), StoreError> {
    let dir = dst.parent().unwrap_or(Path::new("."));
    ensure_dir(dir)?;
    if let Err(e) = fs::rename(tmp, dst) {
        let _ = fs::remove_file(tmp);
        return Err(StoreError::io("rename", dst, e));
    }
    if sync { sync_dir(dir) } else { Ok(()) }
}

/// Write `bytes` to `dst` through a temp file in `tmp_dir` and a rename (durably when `sync`).
pub(crate) fn write_durable(
    tmp_dir: &Path,
    dst: &Path,
    bytes: &[u8],
    sync: bool,
) -> Result<(), StoreError> {
    let tmp = write_temp_plain(tmp_dir, bytes, sync)?;
    commit_rename(&tmp, dst, sync)
}
