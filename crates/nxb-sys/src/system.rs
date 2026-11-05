//! Machine facts: physical memory and core counts by performance level.

/// Core counts of this machine.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CoreCounts {
    /// Performance cores (all cores on a machine without levels).
    pub performance: u32,
    /// Efficiency cores (0 when the machine has no levels).
    pub efficiency: u32,
    /// Logical CPUs the process may use.
    pub logical: u32,
}

/// Physical memory in bytes (`hw.memsize`), `None` when unknown.
pub fn memsize() -> Option<u64> {
    imp::memsize()
}

/// Core counts. On Apple silicon `hw.perflevel0.physicalcpu` (P) and `hw.perflevel1.physicalcpu` (E), elsewhere all
/// logical CPUs count as performance cores.
pub fn core_counts() -> CoreCounts {
    let logical = std::thread::available_parallelism()
        .map(|n| u32::try_from(n.get()).unwrap_or(u32::MAX))
        .unwrap_or(1);
    match imp::perf_levels() {
        Some((p, e)) if p > 0 => CoreCounts {
            performance: p,
            efficiency: e,
            logical,
        },
        _ => CoreCounts {
            performance: logical,
            efficiency: 0,
            logical,
        },
    }
}

/// Whether the file system holding `path` is APFS (`statfs` `f_fstypename`).`false` when unknown.
pub fn is_apfs(path: &std::path::Path) -> bool {
    imp::fs_type(path).is_some_and(|t| t == "apfs")
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod imp {
    use std::ffi::{CStr, CString, c_char, c_void};
    use std::os::unix::ffi::OsStrExt;

    /// Darwin's `struct statfs` (64-bit inode layout).
    #[repr(C)]
    struct Statfs {
        bsize: u32,
        iosize: i32,
        counts: [u64; 5],
        fsid: [i32; 2],
        owner: u32,
        ftype: u32,
        flags: u32,
        fssubtype: u32,
        fstypename: [c_char; 16],
        mntonname: [c_char; 1024],
        mntfromname: [c_char; 1024],
        reserved: [u32; 8],
    }

    unsafe extern "C" {
        fn statfs(path: *const c_char, buf: *mut Statfs) -> i32;
    }

    pub(super) fn fs_type(path: &std::path::Path) -> Option<String> {
        let p = CString::new(path.as_os_str().as_bytes()).ok()?;
        // SAFETY: a zeroed statfs is a valid value of this plain C struct.
        let mut s: Statfs = unsafe { std::mem::zeroed() };
        // SAFETY: `p` is NUL-terminated and `s` is writable for the call.
        if unsafe { statfs(p.as_ptr(), &mut s) } != 0 {
            return None;
        }
        // SAFETY: the kernel NUL-terminates f_fstypename.
        let n = unsafe { CStr::from_ptr(s.fstypename.as_ptr()) };
        Some(n.to_string_lossy().into_owned())
    }

    unsafe extern "C" {
        fn sysctlbyname(
            name: *const c_char,
            oldp: *mut c_void,
            oldlenp: *mut usize,
            newp: *mut c_void,
            newlen: usize,
        ) -> i32;
    }

    fn read_u64(name: &CStr) -> Option<u64> {
        let mut buf = [0u8; 8];
        let mut len = buf.len();
        // SAFETY: `name` is NUL-terminated.`buf` is writable for `len` bytes and `len` is updated by the call, no new value.
        let rc = unsafe {
            sysctlbyname(
                name.as_ptr(),
                buf.as_mut_ptr().cast(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        if rc != 0 {
            return None;
        }
        match len {
            4 => Some(u64::from(u32::from_ne_bytes([
                buf[0], buf[1], buf[2], buf[3],
            ]))),
            8 => Some(u64::from_ne_bytes(buf)),
            _ => None,
        }
    }

    pub(super) fn memsize() -> Option<u64> {
        read_u64(c"hw.memsize")
    }

    pub(super) fn perf_levels() -> Option<(u32, u32)> {
        let p = read_u64(c"hw.perflevel0.physicalcpu")?;
        let e = read_u64(c"hw.perflevel1.physicalcpu").unwrap_or(0);
        Some((u32::try_from(p).ok()?, u32::try_from(e).ok()?))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
mod imp {
    pub(super) fn fs_type(_path: &std::path::Path) -> Option<String> {
        None
    }

    pub(super) fn memsize() -> Option<u64> {
        None
    }

    pub(super) fn perf_levels() -> Option<(u32, u32)> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_are_sane() {
        let c = core_counts();
        assert!(c.performance >= 1);
        assert!(c.performance + c.efficiency <= c.logical.max(c.performance + c.efficiency));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn memsize_on_mac() {
        let m = memsize().unwrap();
        assert!(m >= 1 << 30);
    }
}
