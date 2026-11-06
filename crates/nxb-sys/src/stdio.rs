//! Standard output and interrupt signals of the `nxb` process.
//!
//! [`stdout_to_stderr`] makes file descriptor 1 a copy of descriptor 2 and returns the old standard output as a file:
//! whatever a library or a child process prints to standard output then goes to standard error, and only the caller
//! that holds the returned file writes to the original standard output. [`catch_interrupts`] installs handlers for
//! `SIGINT` and `SIGTERM` that only record the signal number, [`interrupted`] reads it.

use std::fs::File;
use std::io;
use std::sync::atomic::{AtomicI32, Ordering};

static CAUGHT: AtomicI32 = AtomicI32::new(0);

/// The number of the signal caught since [`catch_interrupts`] (`SIGINT` 2, `SIGTERM` 15), or `None`.
pub fn interrupted() -> Option<i32> {
    match CAUGHT.load(Ordering::SeqCst) {
        0 => None,
        n => Some(n),
    }
}

/// Descriptor 1 becomes a copy of descriptor 2. Returns the original standard output (a new descriptor).
pub fn stdout_to_stderr() -> io::Result<File> {
    imp::stdout_to_stderr()
}

/// Records `SIGINT` and `SIGTERM` in [`interrupted`] instead of ending the process.
pub fn catch_interrupts() -> io::Result<()> {
    imp::catch_interrupts()
}

#[cfg(unix)]
mod imp {
    use super::CAUGHT;
    use std::fs::File;
    use std::io;
    use std::os::fd::FromRawFd;
    use std::sync::atomic::Ordering;

    unsafe extern "C" {
        fn dup(fd: i32) -> i32;
        fn dup2(from: i32, to: i32) -> i32;
        fn signal(sig: i32, handler: usize) -> usize;
    }

    const SIGINT: i32 = 2;
    const SIGTERM: i32 = 15;
    const SIG_ERR: usize = usize::MAX;

    extern "C" fn on_signal(sig: i32) {
        CAUGHT.store(sig, Ordering::SeqCst);
    }

    pub(super) fn stdout_to_stderr() -> io::Result<File> {
        // SAFETY: dup and dup2 take integers, the new descriptor from dup is owned by the File made from it.
        unsafe {
            let saved = dup(1);
            if saved < 0 {
                return Err(io::Error::last_os_error());
            }
            let file = File::from_raw_fd(saved);
            if dup2(2, 1) < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(file)
        }
    }

    pub(super) fn catch_interrupts() -> io::Result<()> {
        for sig in [SIGINT, SIGTERM] {
            // SAFETY: the handler only stores into an atomic integer, which is allowed in a signal handler.
            let r = unsafe { signal(sig, on_signal as extern "C" fn(i32) as usize) };
            if r == SIG_ERR {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }
}

#[cfg(not(unix))]
mod imp {
    use std::fs::File;
    use std::io;

    pub(super) fn stdout_to_stderr() -> io::Result<File> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    pub(super) fn catch_interrupts() -> io::Result<()> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}
