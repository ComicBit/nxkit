//! Copy-on-write clones (`clonefile` on APFS) with a plain copy as fallback.

use std::fs;
use std::io;
use std::path::Path;

/// How a path was materialised.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CloneKind {
    /// A copy-on-write clone (no data copied).
    Cloned,
    /// A byte copy (other volume or file system).
    Copied,
}

/// Clone `src` (a file, or a whole directory tree) to `dst`, which must not exist. Falls back to a copy when the file
/// system cannot clone (other volume, not APFS). Symlinks inside a tree are copied as links.
pub fn clone_or_copy(src: &Path, dst: &Path) -> io::Result<CloneKind> {
    if imp::clone(src, dst)? {
        return Ok(CloneKind::Cloned);
    }
    copy_tree(src, dst)?;
    Ok(CloneKind::Copied)
}

fn copy_tree(src: &Path, dst: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(src)?;
    if meta.is_dir() {
        fs::create_dir(dst)?;
        for e in fs::read_dir(src)? {
            let e = e?;
            copy_tree(&e.path(), &dst.join(e.file_name()))?;
        }
        Ok(())
    } else if meta.file_type().is_symlink() {
        copy_link(src, dst)
    } else {
        fs::copy(src, dst).map(|_| ())
    }
}

#[cfg(unix)]
fn copy_link(src: &Path, dst: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(fs::read_link(src)?, dst)
}

#[cfg(not(unix))]
fn copy_link(src: &Path, dst: &Path) -> io::Result<()> {
    fs::copy(src, dst).map(|_| ())
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod imp {
    use std::ffi::{CString, c_char};
    use std::io;
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    const CLONE_NOFOLLOW: u32 = 1;
    const EXDEV: i32 = 18;
    const ENOTSUP: i32 = 45;

    unsafe extern "C" {
        fn clonefile(src: *const c_char, dst: *const c_char, flags: u32) -> i32;
    }

    /// `Ok(false)`: this file system cannot clone, copy instead.
    pub(super) fn clone(src: &Path, dst: &Path) -> io::Result<bool> {
        let s = CString::new(src.as_os_str().as_bytes())?;
        let d = CString::new(dst.as_os_str().as_bytes())?;
        // SAFETY: both pointers are NUL-terminated strings alive for the call.
        if unsafe { clonefile(s.as_ptr(), d.as_ptr(), CLONE_NOFOLLOW) } == 0 {
            return Ok(true);
        }
        let e = io::Error::last_os_error();
        match e.raw_os_error() {
            Some(EXDEV | ENOTSUP) => Ok(false),
            _ => Err(e),
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
mod imp {
    use std::io;
    use std::path::Path;

    pub(super) fn clone(_src: &Path, _dst: &Path) -> io::Result<bool> {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_file_and_tree() {
        let base = std::env::temp_dir().join(format!("nxb-clone-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("src/sub")).unwrap();
        fs::write(base.join("src/a"), b"alpha").unwrap();
        fs::write(base.join("src/sub/b"), b"beta").unwrap();
        let k = clone_or_copy(&base.join("src/a"), &base.join("a2")).unwrap();
        assert_eq!(fs::read(base.join("a2")).unwrap(), b"alpha");
        clone_or_copy(&base.join("src"), &base.join("dst")).unwrap();
        assert_eq!(fs::read(base.join("dst/sub/b")).unwrap(), b"beta");
        // The destination must not exist.
        assert!(clone_or_copy(&base.join("src/a"), &base.join("a2")).is_err());
        // Writing the clone leaves the source alone.
        fs::write(base.join("a2"), b"changed").unwrap();
        assert_eq!(fs::read(base.join("src/a")).unwrap(), b"alpha");
        #[cfg(target_os = "macos")]
        assert_eq!(k, CloneKind::Cloned);
        let _ = k;
        fs::remove_dir_all(base).unwrap();
    }
}
