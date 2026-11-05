//! Advisory file locks (`flock`): exclusive on an output directory, shared or exclusive on a store root.
//!
//! A lock belongs to the open file, so two `FileLock`s on the same path conflict even inside one process, and the lock
//! is released when the `FileLock` is dropped or the process dies (no stale lock after a crash).

use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

/// Lock mode.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LockMode {
    /// Many holders (builds sharing a store).
    Shared,
    /// One holder (a build's output directory, prune, migration).
    Exclusive,
}

/// A held lock on a lock file (created if missing).
#[derive(Debug)]
pub struct FileLock {
    file: File,
    path: PathBuf,
    mode: LockMode,
}

impl FileLock {
    /// Take the lock. With `wait` false, `Ok(None)` means another holder has a conflicting lock.
    pub fn acquire(path: &Path, mode: LockMode, wait: bool) -> io::Result<Option<FileLock>> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        if !imp::lock(&file, mode, wait)? {
            return Ok(None);
        }
        Ok(Some(FileLock {
            file,
            path: path.to_path_buf(),
            mode,
        }))
    }

    /// The lock file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The mode held.
    pub fn mode(&self) -> LockMode {
        self.mode
    }

    /// The open lock file (for writing diagnostics such as the owner's pid).
    pub fn file(&self) -> &File {
        &self.file
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        imp::unlock(&self.file);
    }
}

#[cfg(unix)]
mod imp {
    use super::LockMode;
    use std::fs::File;
    use std::io;
    use std::os::fd::AsRawFd;

    const LOCK_SH: i32 = 1;
    const LOCK_EX: i32 = 2;
    const LOCK_NB: i32 = 4;
    const LOCK_UN: i32 = 8;
    const EINTR: i32 = 4;

    unsafe extern "C" {
        fn flock(fd: i32, operation: i32) -> i32;
    }

    pub(super) fn lock(f: &File, mode: LockMode, wait: bool) -> io::Result<bool> {
        let mut op = match mode {
            LockMode::Shared => LOCK_SH,
            LockMode::Exclusive => LOCK_EX,
        };
        if !wait {
            op |= LOCK_NB;
        }
        loop {
            // SAFETY: flock takes a file descriptor owned by `f`, alive for the call, and an integer.
            if unsafe { flock(f.as_raw_fd(), op) } == 0 {
                return Ok(true);
            }
            let e = io::Error::last_os_error();
            match e.raw_os_error() {
                Some(EINTR) => continue,
                _ if e.kind() == io::ErrorKind::WouldBlock => return Ok(false),
                _ => return Err(e),
            }
        }
    }

    pub(super) fn unlock(f: &File) {
        // SAFETY: as above, closing the file would release the lock too.
        let _ = unsafe { flock(f.as_raw_fd(), LOCK_UN) };
    }
}

#[cfg(not(unix))]
mod imp {
    use super::LockMode;
    use std::fs::File;
    use std::io;

    // Windows uses LockFileEx (design-v2 10.4), refuse until it exists rather than pretend to lock.
    pub(super) fn lock(_f: &File, _mode: LockMode, _wait: bool) -> io::Result<bool> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    pub(super) fn unlock(_f: &File) {}
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn exclusive_and_shared_conflicts() {
        let p = std::env::temp_dir().join(format!("nxb-lock-test-{}", std::process::id()));
        let a = FileLock::acquire(&p, LockMode::Shared, false)
            .unwrap()
            .unwrap();
        let b = FileLock::acquire(&p, LockMode::Shared, false).unwrap();
        assert!(b.is_some(), "shared locks coexist");
        assert!(
            FileLock::acquire(&p, LockMode::Exclusive, false)
                .unwrap()
                .is_none()
        );
        drop((a, b));
        let x = FileLock::acquire(&p, LockMode::Exclusive, false)
            .unwrap()
            .unwrap();
        assert!(
            FileLock::acquire(&p, LockMode::Shared, false)
                .unwrap()
                .is_none()
        );
        assert_eq!(x.mode(), LockMode::Exclusive);
        drop(x);
        assert!(
            FileLock::acquire(&p, LockMode::Exclusive, true)
                .unwrap()
                .is_some()
        );
        let _ = std::fs::remove_file(p);
    }
}
