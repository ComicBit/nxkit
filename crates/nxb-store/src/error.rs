//! Store errors.

use nxb_error::{EnvError, ErrorClass, IoOp};
use nxb_model::Digest;
use std::fmt;
use std::io;
use std::path::PathBuf;

/// A store operation failed.
#[derive(Debug)]
#[non_exhaustive]
pub enum StoreError {
    /// A file system call failed.
    Io {
        /// What was being done (`write blob`, `rename manifest`, …).
        op: &'static str,
        /// The path involved.
        path: PathBuf,
        /// The error.
        source: io::Error,
    },
    /// A file name in an entry is not allowed.
    BadName(String),
    /// An entry refers to a blob that is not in the store (or has another size).
    MissingBlob(Digest),
    /// The root holds another store format.
    WrongFormat(String),
    /// Another holder has a conflicting lock on this store root or output directory.
    Locked(PathBuf),
    /// A cache export file is damaged, truncated or not a cache export.
    BadExport(String),
}

impl StoreError {
    pub(crate) fn io(op: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        StoreError::Io {
            op,
            path: path.into(),
            source,
        }
    }

    /// The machine problem behind this error, if any (disk full, read-only volume, too many files).
    pub fn env(&self) -> Option<EnvError> {
        match self {
            StoreError::Io { path, source, .. } => {
                EnvError::classify(source, IoOp::Write, &path.display().to_string())
            }
            _ => None,
        }
    }

    /// The error class: `Env` for machine problems, `Build` for a foreign store format, `Internal` otherwise.
    pub fn class(&self) -> ErrorClass {
        match self {
            _ if self.env().is_some() => ErrorClass::Env,
            StoreError::WrongFormat(_) | StoreError::Locked(_) | StoreError::BadExport(_) => {
                ErrorClass::Build
            }
            _ => ErrorClass::Internal,
        }
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Io { op, path, source } => {
                write!(f, "store: {op} {}: {source}", path.display())
            }
            StoreError::BadName(n) => write!(f, "store: file name {n:?} is not allowed"),
            StoreError::MissingBlob(d) => write!(f, "store: blob {d} is missing"),
            StoreError::WrongFormat(s) => write!(f, "store: unknown format {s:?}"),
            StoreError::BadExport(s) => write!(f, "cache export: {s}"),
            StoreError::Locked(p) => write!(f, "another build is using {}", p.display()),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StoreError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
