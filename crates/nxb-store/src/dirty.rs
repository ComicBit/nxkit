//! The dirty marker (design-v2 10.3): each open store writes a journal of the blobs and entries it is about to write.
//!
//! `dirty/<process token>-<n>.journal` is created at open and held under an exclusive `flock` for the store's life, a
//! clean drop deletes it. A journal whose lock can be taken belongs to a dead process: at the next open its blobs are
//! re-hashed, its entries verified (damage goes to quarantine), its temp files removed, then the journal is deleted.

use crate::error::StoreError;
use crate::fsutil::{ensure_dir, process_token};
use nxb_sys::lock::{FileLock, LockMode};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

static JOURNAL_N: AtomicU64 = AtomicU64::new(0);

/// One store instance's journal.
#[derive(Debug)]
pub(crate) struct Journal {
    path: PathBuf,
    out: Mutex<File>,
    _lock: FileLock,
}

impl Journal {
    pub(crate) fn create(dirty_dir: &Path) -> Result<Journal, StoreError> {
        ensure_dir(dirty_dir)?;
        let n = JOURNAL_N.fetch_add(1, Ordering::Relaxed);
        let path = dirty_dir.join(format!("{}-j{n}.journal", process_token()));
        let lock = FileLock::acquire(&path, LockMode::Exclusive, false)
            .map_err(|e| StoreError::io("lock journal", &path, e))?
            .ok_or_else(|| StoreError::Locked(path.clone()))?;
        let out = OpenOptions::new()
            .append(true)
            .open(&path)
            .map_err(|e| StoreError::io("open journal", &path, e))?;
        Ok(Journal {
            path,
            out: Mutex::new(out),
            _lock: lock,
        })
    }

    /// Record that a blob or entry is about to be written. One small append, no fsync: the store's own fsync order
    /// keeps every visible file whole, the journal tells the next run what to check after a crash.
    pub(crate) fn note(&self, kind: &str, hex: &str) -> Result<(), StoreError> {
        let mut f = self.out.lock().unwrap_or_else(PoisonError::into_inner);
        f.write_all(format!("{kind} {hex}\n").as_bytes())
            .map_err(|e| StoreError::io("write journal", &self.path, e))
    }
}

impl Drop for Journal {
    fn drop(&mut self) {
        // Clean end: nothing to check next time. The lock is released after the file is gone.
        let _ = fs::remove_file(&self.path);
    }
}

/// What a dead writer's journal asks to check.
#[derive(Debug, Default)]
pub(crate) struct Orphan {
    pub(crate) blobs: Vec<String>,
    pub(crate) entries: Vec<String>,
    /// Temp-file prefix of the dead process.
    pub(crate) token: String,
    pub(crate) path: PathBuf,
    pub(crate) lock: Option<FileLock>,
}

/// Journals in `dirty_dir` whose owner is gone (their lock is free). Each comes with its lock held, so two recovering
/// processes never handle the same journal.
pub(crate) fn orphans(dirty_dir: &Path) -> Result<Vec<Orphan>, StoreError> {
    let mut out = Vec::new();
    let rd = match fs::read_dir(dirty_dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(StoreError::io("list journals", dirty_dir, e)),
    };
    for e in rd.flatten() {
        let path = e.path();
        let Some(stem) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".journal"))
        else {
            continue;
        };
        let Some((token, _)) = stem.rsplit_once("-j") else {
            continue;
        };
        let lock = match FileLock::acquire(&path, LockMode::Exclusive, false) {
            Ok(Some(l)) => l,
            Ok(None) => continue, // owner alive
            Err(e) => return Err(StoreError::io("lock journal", &path, e)),
        };
        let text = fs::read_to_string(&path).unwrap_or_default();
        let mut o = Orphan {
            token: token.to_string(),
            path: path.clone(),
            lock: Some(lock),
            ..Orphan::default()
        };
        for line in text.lines() {
            match line.split_once(' ') {
                Some(("blob", h)) => o.blobs.push(h.to_string()),
                Some(("entry", h)) => o.entries.push(h.to_string()),
                _ => {}
            }
        }
        o.blobs.sort();
        o.blobs.dedup();
        o.entries.sort();
        o.entries.dedup();
        out.push(o);
    }
    Ok(out)
}

/// Remove the dead process's temp files and its journal.
pub(crate) fn finish(tmp_dir: &Path, mut o: Orphan) -> Result<(), StoreError> {
    let prefix = format!("{}-", o.token);
    if let Ok(rd) = fs::read_dir(tmp_dir) {
        for e in rd.flatten() {
            let name = e.file_name();
            if name.to_str().is_some_and(|n| n.starts_with(&prefix)) {
                let _ = fs::remove_file(e.path());
            }
        }
    }
    fs::remove_file(&o.path).map_err(|e| StoreError::io("remove journal", &o.path, e))?;
    o.lock.take();
    Ok(())
}
