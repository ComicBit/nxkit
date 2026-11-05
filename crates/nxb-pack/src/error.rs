//! The crate's error type.

use std::fmt;
use std::path::PathBuf;

/// Result with the crate's [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// Everything that can go wrong while signing.
#[derive(Debug)]
pub enum Error {
    /// An I/O operation on `path` failed.
    Io {
        /// The file or directory involved.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// The input is malformed (bad Mach-O, bad plist, ...).
    Format(String),
    /// The input is valid but uses a feature this signer does not implement, use `codesign`.
    Unsupported(String),
    /// The bundle content is something `codesign` would refuse as well.
    Refused(String),
}

impl Error {
    /// Wraps an I/O error with its path.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Error::Format(m) => write!(f, "malformed input: {m}"),
            Error::Unsupported(m) => write!(f, "not supported by nxb-codesign (use codesign): {m}"),
            Error::Refused(m) => write!(f, "refused: {m}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Shorthand for a [`Error::Format`] error.
pub(crate) fn format_err(msg: impl Into<String>) -> Error {
    Error::Format(msg.into())
}
