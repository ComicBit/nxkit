//! Process groups: kill a child's whole group, and wait for a child's exit without reaping it.
//!
//! Children are started in their own process group (`std::os::unix::process::CommandExt::process_group(0)`), so the
//! group id equals the child's pid. While the child is not reaped its pid (and so the group id) cannot be reused, which
//! makes [`kill_group`] safe between [`wait_exit_nowait`] returning and the final reap.

use std::io;

/// Signals used by the tool layer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Signal {
    /// `SIGKILL`: cannot be caught.
    Kill,
    /// `SIGTERM`: polite stop.
    Term,
}

impl Signal {
    #[cfg(unix)]
    fn number(self) -> i32 {
        match self {
            Signal::Kill => 9,
            Signal::Term => 15,
        }
    }
}

/// Send `sig` to every process in group `pgid`. A group that no longer exists (`ESRCH`) is not an error.
///
/// Not atomic against processes forking or execing at that instant: a caller that must end the group repeats the
/// kill until the leader has exited (and, for stragglers, until its pipes close).
pub fn kill_group(pgid: u32, sig: Signal) -> io::Result<()> {
    imp::kill_group(pgid, sig)
}

/// Set the nice value of every process in group `pgid` (`setpriority(PRIO_PGRP)`), from 0 (normal) to 20 (lowest).
/// Children the group starts later inherit it. A group that no longer exists (`ESRCH`) is not an error.
pub fn set_group_nice(pgid: u32, nice: i32) -> io::Result<()> {
    imp::set_group_nice(pgid, nice)
}

/// CPU and memory of one reaped child.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ChildUsage {
    /// User CPU time.
    pub user: std::time::Duration,
    /// System CPU time.
    pub system: std::time::Duration,
    /// Peak resident set size in bytes.
    pub max_rss: u64,
}

/// Reap the child `pid` (`wait4`) and return its raw wait status and resource usage. Retries on `EINTR`.
/// Use `std::os::unix::process::ExitStatusExt::from_raw` to read the status.
pub fn wait4(pid: u32) -> io::Result<(i32, ChildUsage)> {
    imp::wait4(pid)
}

/// Block until the child `pid` has exited, without reaping it (`waitid(P_PID, pid, WEXITED | WNOWAIT)`).
///
/// The caller reaps it afterwards (for example with `std::process::Child::wait`). Retries on `EINTR`.
pub fn wait_exit_nowait(pid: u32) -> io::Result<()> {
    imp::wait_exit_nowait(pid)
}

#[cfg(unix)]
mod imp {
    use super::Signal;
    use std::io;

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    const WNOWAIT: i32 = 0x20;
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    const WNOWAIT: i32 = 0x0100_0000;
    const WEXITED: i32 = 0x04;
    const P_PID: u32 = 1;
    const ESRCH: i32 = 3;
    const EINTR: i32 = 4;

    /// Large enough for `siginfo_t` on macOS (104 bytes) and Linux (128 bytes), contents are not read.
    #[repr(C, align(8))]
    struct SigInfoBuf([u8; 256]);

    #[repr(C)]
    #[derive(Default)]
    struct TimeVal {
        sec: i64,
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        usec: i32,
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        usec: i64,
    }

    #[repr(C)]
    #[derive(Default)]
    struct RUsage {
        utime: TimeVal,
        stime: TimeVal,
        maxrss: i64,
        rest: [i64; 13],
    }

    unsafe extern "C" {
        fn killpg(pgrp: i32, sig: i32) -> i32;
        fn setpriority(which: i32, who: u32, prio: i32) -> i32;
        fn waitid(idtype: u32, id: u32, infop: *mut SigInfoBuf, options: i32) -> i32;
        #[link_name = "wait4"]
        fn c_wait4(pid: i32, status: *mut i32, options: i32, rusage: *mut RUsage) -> i32;
    }

    fn duration(t: &TimeVal) -> std::time::Duration {
        let usec = u64::try_from(t.usec).unwrap_or(0);
        std::time::Duration::from_secs(u64::try_from(t.sec).unwrap_or(0))
            + std::time::Duration::from_micros(usec)
    }

    pub(super) fn wait4(pid: u32) -> io::Result<(i32, super::ChildUsage)> {
        let Ok(pid) = i32::try_from(pid) else {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        };
        let mut status = 0i32;
        let mut ru = RUsage::default();
        loop {
            // SAFETY: `status` and `ru` are valid, writable and live for the call, RUsage matches struct rusage.
            let rc = unsafe { c_wait4(pid, &mut status, 0, &mut ru) };
            if rc == pid {
                break;
            }
            let err = io::Error::last_os_error();
            if err.raw_os_error() != Some(EINTR) {
                return Err(err);
            }
        }
        // ru_maxrss is bytes on Apple systems, KiB on Linux.
        let rss = u64::try_from(ru.maxrss).unwrap_or(0);
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        let rss = rss * 1024;
        Ok((
            status,
            super::ChildUsage {
                user: duration(&ru.utime),
                system: duration(&ru.stime),
                max_rss: rss,
            },
        ))
    }

    pub(super) fn kill_group(pgid: u32, sig: Signal) -> io::Result<()> {
        let Ok(pgid) = i32::try_from(pgid) else {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        };
        if pgid <= 1 {
            // Never signal group 0 (our own) or 1 (launchd/init).
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }
        // SAFETY: killpg takes two integers and has no memory effects.
        let rc = unsafe { killpg(pgid, sig.number()) };
        if rc == 0 {
            return Ok(());
        }
        let err = io::Error::last_os_error();
        if err.raw_os_error() == Some(ESRCH) {
            Ok(())
        } else {
            Err(err)
        }
    }

    pub(super) fn set_group_nice(pgid: u32, nice: i32) -> io::Result<()> {
        const PRIO_PGRP: i32 = 1;
        if pgid <= 1 {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }
        // SAFETY: setpriority takes three integers and has no memory effects.
        let rc = unsafe { setpriority(PRIO_PGRP, pgid, nice.clamp(0, 20)) };
        if rc == 0 {
            return Ok(());
        }
        let err = io::Error::last_os_error();
        if err.raw_os_error() == Some(ESRCH) {
            Ok(())
        } else {
            Err(err)
        }
    }

    pub(super) fn wait_exit_nowait(pid: u32) -> io::Result<()> {
        let mut info = SigInfoBuf([0; 256]);
        loop {
            // SAFETY: `info` is a valid, writable, suitably aligned buffer larger than siginfo_t and lives for the call.
            let rc = unsafe { waitid(P_PID, pid, &mut info, WEXITED | WNOWAIT) };
            if rc == 0 {
                return Ok(());
            }
            let err = io::Error::last_os_error();
            if err.raw_os_error() != Some(EINTR) {
                return Err(err);
            }
        }
    }
}

#[cfg(not(unix))]
mod imp {
    use super::Signal;
    use std::io;

    // Windows uses Job Objects (design-v2 9.4), they come with the Windows implementation.
    pub(super) fn kill_group(_pgid: u32, _sig: Signal) -> io::Result<()> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    pub(super) fn wait_exit_nowait(_pid: u32) -> io::Result<()> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    pub(super) fn set_group_nice(_pgid: u32, _nice: i32) -> io::Result<()> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    pub(super) fn wait4(_pid: u32) -> io::Result<(i32, super::ChildUsage)> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::process::CommandExt;
    use std::process::Command;
    use std::time::{Duration, Instant};

    #[test]
    fn refuses_own_and_init_group() {
        assert!(kill_group(0, Signal::Kill).is_err());
        assert!(kill_group(1, Signal::Kill).is_err());
        assert!(set_group_nice(1, 10).is_err());
    }

    #[test]
    fn lowers_a_groups_priority() {
        let mut child = Command::new("/bin/sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .unwrap();
        let pid = child.id();
        set_group_nice(pid, 10).unwrap();
        let ps = Command::new("/bin/ps")
            .args(["-o", "nice=", "-p", &pid.to_string()])
            .output()
            .unwrap();
        kill_group(pid, Signal::Kill).unwrap();
        let _ = child.wait();
        assert_eq!(String::from_utf8_lossy(&ps.stdout).trim(), "10");
    }

    #[test]
    fn kills_whole_group_including_grandchild() {
        // The shell starts a grandchild sleeper in the same group, then sleeps itself.
        let mut child = Command::new("/bin/sh")
            .arg("-c")
            .arg("/bin/sleep 30 & /bin/sleep 30; wait")
            .process_group(0)
            .spawn()
            .unwrap();
        let pid = child.id();
        std::thread::sleep(Duration::from_millis(100));
        let t = Instant::now();
        // One killpg can miss a process that is forking or execing at that instant (seen on macOS: 3 in 20 runs),
        // so callers repeat it until the leader has exited, as nxb-tool does.
        let waiter = std::thread::spawn(move || wait_exit_nowait(pid));
        while !waiter.is_finished() {
            kill_group(pid, Signal::Kill).unwrap();
            std::thread::sleep(Duration::from_millis(20));
        }
        waiter.join().unwrap().unwrap();
        // The status is not asserted: when the kill reaches the two sleepers before the shell, the shell sees them
        // die and exits 0 itself (seen under load). What matters is that the group is gone quickly.
        let _ = child.wait().unwrap();
        assert!(t.elapsed() < Duration::from_secs(25));
        // The group is gone: signalling it again is fine (ESRCH is not an error) once the sleeper has died too.
        std::thread::sleep(Duration::from_millis(100));
        kill_group(pid, Signal::Kill).unwrap();
    }

    #[test]
    #[allow(clippy::zombie_processes, reason = "reaped by wait4, not Child::wait")]
    fn wait4_reports_status_and_usage() {
        use std::os::unix::process::ExitStatusExt;
        // Burn a little CPU in the child so user time is visible.
        let child = Command::new("/bin/sh")
            .arg("-c")
            .arg("i=0; while [ $i -lt 20000 ]; do i=$((i+1)); done; exit 7")
            .process_group(0)
            .spawn()
            .unwrap();
        let (status, usage) = wait4(child.id()).unwrap();
        assert_eq!(std::process::ExitStatus::from_raw(status).code(), Some(7));
        assert!(usage.user + usage.system > Duration::ZERO);
        assert!(usage.max_rss > 100_000, "{}", usage.max_rss);
    }

    #[test]
    fn wait_nowait_leaves_child_reapable() {
        let mut child = Command::new("/usr/bin/true")
            .process_group(0)
            .spawn()
            .unwrap();
        wait_exit_nowait(child.id()).unwrap();
        assert!(child.wait().unwrap().success());
    }
}
