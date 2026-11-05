//! The error of the rewriter.

use std::fmt;

/// Why a rewrite did not produce its files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RewriteError {
    /// Outside the contract of this port: the caller runs the reference script (exit code 4).
    Unsupported(String),
    /// The module is damaged or a rule found something it cannot handle (the script's `[RED]` lines and
    /// assertion failures): exit code 1.
    Failed(String),
}

impl fmt::Display for RewriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RewriteError::Unsupported(m) => write!(f, "unsupported: {m}"),
            RewriteError::Failed(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for RewriteError {}

/// A failed assertion of the reference script.
pub(crate) fn fail<T>(m: impl Into<String>) -> Result<T, RewriteError> {
    Err(RewriteError::Failed(m.into()))
}
