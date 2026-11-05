//! Machine problems (`EnvError`) and the errno classification of design-v2 11.1 and 11.3.

use std::fmt;
use std::io;

/// The kind of machine problem.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum EnvKind {
    /// `ENOSPC`: the disk is full.
    NoSpace,
    /// `EDQUOT`: quota exceeded.
    Quota,
    /// `EMFILE` or `ENFILE`: too many open files.
    TooManyFiles,
    /// `EROFS`: read-only file system.
    ReadOnly,
    /// `EIO`: input/output error.
    Io,
    /// `EACCES`/`EPERM` on an output.
    Access,
    /// `EAGAIN` or `ENOMEM` at spawn: no process or memory resources.
    SpawnResources,
    /// The toolchain is gone (resolved path missing, `ENOENT`/`ENOEXEC` at spawn).
    ToolchainMissing,
    /// The translator runtime is missing.
    TranslatorMissing,
    /// The circuit breaker saw too many identical tool failures.
    RepeatedToolFailure,
}

impl EnvKind {
    /// Stable lower-case name.
    pub fn name(self) -> &'static str {
        match self {
            EnvKind::NoSpace => "no_space",
            EnvKind::Quota => "quota",
            EnvKind::TooManyFiles => "too_many_files",
            EnvKind::ReadOnly => "read_only",
            EnvKind::Io => "io",
            EnvKind::Access => "access",
            EnvKind::SpawnResources => "spawn_resources",
            EnvKind::ToolchainMissing => "toolchain_missing",
            EnvKind::TranslatorMissing => "translator_missing",
            EnvKind::RepeatedToolFailure => "repeated_tool_failure",
        }
    }

    /// How many occurrences of this kind on own I/O escalate to an abort (design-v2 11.3):
    /// 1 for disk full, quota, read-only and I/O errors, 3 for too many open files (with back-off in between).
    pub fn threshold(self) -> u32 {
        match self {
            EnvKind::TooManyFiles => 3,
            _ => 1,
        }
    }
}

/// What the failing call was doing, the same errno means different things for a write and a spawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IoOp {
    /// Reading an input.
    Read,
    /// Writing, creating or renaming an output or a store entry.
    Write,
    /// Starting a child process.
    Spawn,
}

/// A machine problem: abort the run, publish nothing, never retry.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EnvError {
    /// The kind.
    pub kind: EnvKind,
    /// What was being done, for the `[RED]` line (a path or a tool name, never file contents).
    pub context: String,
}

impl EnvError {
    /// An env error of this kind.
    pub fn new(kind: EnvKind, context: impl Into<String>) -> Self {
        EnvError {
            kind,
            context: context.into(),
        }
    }

    /// Classify an I/O error. `None` means it is not a machine problem (for example a missing input file on read), so the
    /// caller treats it as an error of its own scope.
    pub fn classify(err: &io::Error, op: IoOp, context: &str) -> Option<EnvError> {
        let kind = classify_kind(err, op)?;
        Some(EnvError::new(kind, context))
    }
}

fn classify_kind(err: &io::Error, op: IoOp) -> Option<EnvKind> {
    if let Some(code) = err.raw_os_error()
        && let Some(k) = classify_errno(code, op)
    {
        return Some(k);
    }
    match (err.kind(), op) {
        (io::ErrorKind::StorageFull, _) => Some(EnvKind::NoSpace),
        (io::ErrorKind::QuotaExceeded, _) => Some(EnvKind::Quota),
        (io::ErrorKind::ReadOnlyFilesystem, _) => Some(EnvKind::ReadOnly),
        (io::ErrorKind::PermissionDenied, IoOp::Write) => Some(EnvKind::Access),
        (io::ErrorKind::OutOfMemory, IoOp::Spawn) => Some(EnvKind::SpawnResources),
        (io::ErrorKind::NotFound, IoOp::Spawn) => Some(EnvKind::ToolchainMissing),
        _ => None,
    }
}

#[cfg(unix)]
mod errno {
    pub(super) const EPERM: i32 = 1;
    pub(super) const ENOENT: i32 = 2;
    pub(super) const EIO: i32 = 5;
    pub(super) const ENOEXEC: i32 = 8;
    pub(super) const ENOMEM: i32 = 12;
    pub(super) const EACCES: i32 = 13;
    pub(super) const ENFILE: i32 = 23;
    pub(super) const EMFILE: i32 = 24;
    pub(super) const ENOSPC: i32 = 28;
    pub(super) const EROFS: i32 = 30;
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub(super) const EAGAIN: i32 = 35;
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub(super) const EDQUOT: i32 = 69;
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    pub(super) const EAGAIN: i32 = 11;
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    pub(super) const EDQUOT: i32 = 122;
}

#[cfg(unix)]
fn classify_errno(code: i32, op: IoOp) -> Option<EnvKind> {
    use errno::*;
    match (code, op) {
        (ENOSPC, _) => Some(EnvKind::NoSpace),
        (EDQUOT, _) => Some(EnvKind::Quota),
        (EMFILE | ENFILE, _) => Some(EnvKind::TooManyFiles),
        (EROFS, _) => Some(EnvKind::ReadOnly),
        (EIO, _) => Some(EnvKind::Io),
        (EACCES | EPERM, IoOp::Write) => Some(EnvKind::Access),
        (EAGAIN | ENOMEM, IoOp::Spawn) => Some(EnvKind::SpawnResources),
        (ENOENT | ENOEXEC, IoOp::Spawn) => Some(EnvKind::ToolchainMissing),
        _ => None,
    }
}

#[cfg(not(unix))]
fn classify_errno(_code: i32, _op: IoOp) -> Option<EnvKind> {
    // Windows codes are mapped through io::ErrorKind, the Windows table comes with the nxb-sys Windows impl.
    None
}

impl fmt::Display for EnvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = match self.kind {
            EnvKind::NoSpace => "the disk is full",
            EnvKind::Quota => "the disk quota is exceeded",
            EnvKind::TooManyFiles => "too many open files",
            EnvKind::ReadOnly => "the volume is read-only",
            EnvKind::Io => "an input/output error",
            EnvKind::Access => "no permission to write",
            EnvKind::SpawnResources => "cannot start more processes",
            EnvKind::ToolchainMissing => "the toolchain is missing",
            EnvKind::TranslatorMissing => "the shader translator does not run",
            EnvKind::RepeatedToolFailure => "a tool fails the same way again and again",
        };
        write!(f, "{what}: {}", self.context)
    }
}

impl std::error::Error for EnvError {}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn os(code: i32) -> io::Error {
        io::Error::from_raw_os_error(code)
    }

    #[test]
    fn errno_table() {
        use errno::*;
        let c = |code, op| EnvError::classify(&os(code), op, "x").map(|e| e.kind);
        assert_eq!(c(ENOSPC, IoOp::Write), Some(EnvKind::NoSpace));
        assert_eq!(c(EDQUOT, IoOp::Write), Some(EnvKind::Quota));
        assert_eq!(c(EMFILE, IoOp::Read), Some(EnvKind::TooManyFiles));
        assert_eq!(c(ENFILE, IoOp::Spawn), Some(EnvKind::TooManyFiles));
        assert_eq!(c(EROFS, IoOp::Write), Some(EnvKind::ReadOnly));
        assert_eq!(c(EIO, IoOp::Read), Some(EnvKind::Io));
        assert_eq!(c(EACCES, IoOp::Write), Some(EnvKind::Access));
        assert_eq!(c(EACCES, IoOp::Read), None);
        assert_eq!(c(ENOENT, IoOp::Read), None);
        assert_eq!(c(ENOENT, IoOp::Spawn), Some(EnvKind::ToolchainMissing));
        assert_eq!(c(EAGAIN, IoOp::Spawn), Some(EnvKind::SpawnResources));
    }

    #[test]
    fn thresholds() {
        assert_eq!(EnvKind::NoSpace.threshold(), 1);
        assert_eq!(EnvKind::TooManyFiles.threshold(), 3);
    }

    #[test]
    fn classifier_text_kept() {
        // A front-end classifier key (design-v2 13.2) must stay inside this message.
        let e = EnvError::new(EnvKind::TranslatorMissing, "dmsh_xlate");
        assert!(e.to_string().contains("the shader translator does not run"));
    }
}
