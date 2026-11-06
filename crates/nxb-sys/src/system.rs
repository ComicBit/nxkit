//! Machine facts: physical memory, core counts by performance level, and the current CPU and memory load.

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

/// The machine's CPU time counters, in ticks of all cores together. They are 32 bits wide and wrap around.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CpuTicks {
    /// Ticks running user code.
    pub user: u32,
    /// Ticks running kernel code.
    pub system: u32,
    /// Idle ticks.
    pub idle: u32,
    /// Ticks running niced user code.
    pub nice: u32,
}

impl CpuTicks {
    /// The busy share between `earlier` and `self`, from 0 to 1. `None` when no tick passed.
    pub fn busy_since(&self, earlier: &CpuTicks) -> Option<f64> {
        let d = |a: u32, b: u32| u64::from(a.wrapping_sub(b));
        let busy = d(self.user, earlier.user)
            + d(self.system, earlier.system)
            + d(self.nice, earlier.nice);
        let total = busy + d(self.idle, earlier.idle);
        // tick counts stay far below 2^52, the conversion is exact
        #[allow(clippy::cast_precision_loss)]
        (total > 0).then(|| (busy as f64 / total as f64).clamp(0.0, 1.0))
    }
}

/// The machine's CPU counters (`host_statistics` `HOST_CPU_LOAD_INFO`), `None` when unknown.
pub fn cpu_ticks() -> Option<CpuTicks> {
    imp::cpu_ticks()
}

/// Memory in use in bytes, as Activity Monitor's "Memory Used": app memory (anonymous pages that are not purgeable),
/// wired memory and the compressor's pages. File cache and purgeable pages count as free. `None` when unknown.
pub fn memory_used() -> Option<u64> {
    imp::memory_used()
}

/// The kernel's memory pressure level (`kern.memorystatus_vm_pressure_level`): 1 normal, 2 warning, 4 critical.
/// `None` when unknown.
pub fn memory_pressure() -> Option<u32> {
    imp::memory_pressure()
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

    pub(super) fn memory_used() -> Option<u64> {
        let page = read_u64(c"hw.pagesize")?;
        let internal = read_u64(c"vm.page_pageable_internal_count")?;
        let purgeable = read_u64(c"vm.page_purgeable_count").unwrap_or(0);
        let wired = read_u64(c"vm.page_wired_count")?;
        let compressor = read_u64(c"vm.compressor_bytes_used").unwrap_or(0);
        Some(
            (internal.saturating_sub(purgeable) + wired)
                .saturating_mul(page)
                .saturating_add(compressor),
        )
    }

    pub(super) fn memory_pressure() -> Option<u32> {
        read_u64(c"kern.memorystatus_vm_pressure_level").and_then(|v| u32::try_from(v).ok())
    }

    /// `HOST_CPU_LOAD_INFO` and its count in `natural_t` (user, system, idle, nice).
    const HOST_CPU_LOAD_INFO: i32 = 3;
    const CPU_STATE_MAX: usize = 4;

    unsafe extern "C" {
        fn mach_host_self() -> u32;
        fn host_statistics(host: u32, flavor: i32, info: *mut u32, count: *mut u32) -> i32;
    }

    /// The host port, taken once (every `mach_host_self` call adds a send right).
    fn host() -> u32 {
        static HOST: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
        // SAFETY: `mach_host_self` takes no arguments and only returns a port name.
        *HOST.get_or_init(|| unsafe { mach_host_self() })
    }

    pub(super) fn cpu_ticks() -> Option<super::CpuTicks> {
        let mut t = [0u32; CPU_STATE_MAX];
        let mut count = CPU_STATE_MAX as u32;
        // SAFETY: `t` is writable for `count` natural_t values, the kernel writes at most `count` and updates it.
        let rc = unsafe { host_statistics(host(), HOST_CPU_LOAD_INFO, t.as_mut_ptr(), &mut count) };
        if rc != 0 || count as usize != CPU_STATE_MAX {
            return None;
        }
        let [user, system, idle, nice] = t;
        Some(super::CpuTicks {
            user,
            system,
            idle,
            nice,
        })
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

    pub(super) fn cpu_ticks() -> Option<super::CpuTicks> {
        None
    }

    pub(super) fn memory_used() -> Option<u64> {
        None
    }

    pub(super) fn memory_pressure() -> Option<u32> {
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

    #[cfg(target_os = "macos")]
    #[test]
    fn load_on_mac() {
        let a = cpu_ticks().unwrap();
        let mut x = 0u64;
        for i in 0..20_000_000u64 {
            x = x.wrapping_mul(31).wrapping_add(i);
        }
        std::hint::black_box(x);
        std::thread::sleep(std::time::Duration::from_millis(100));
        let b = cpu_ticks().unwrap();
        let share = b.busy_since(&a).unwrap();
        assert!((0.0..=1.0).contains(&share));
        let used = memory_used().unwrap();
        assert!(used > 0 && used < memsize().unwrap());
        assert!(memory_pressure().unwrap() >= 1);
    }

    #[test]
    fn busy_share() {
        let t = |user, system, idle, nice| CpuTicks {
            user,
            system,
            idle,
            nice,
        };
        let a = t(10, 5, 80, 5);
        assert_eq!(t(40, 15, 130, 15).busy_since(&a), Some(0.5));
        assert_eq!(a.busy_since(&a), None);
        // a counter that wrapped since
        let w = t(u32::MAX - 9, 0, 0, 0);
        assert_eq!(t(10, 0, 20, 0).busy_since(&w), Some(0.5));
    }
}
