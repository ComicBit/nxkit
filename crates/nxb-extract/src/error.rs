//! Extract errors. Texts name files, offsets and key names, never key values or game data.

use std::fmt;
use std::io;

/// Why an extract failed.
#[derive(Debug)]
pub enum ExtractError {
    /// A file operation failed.
    Io {
        /// What was being done (path and operation).
        context: String,
        /// The OS error.
        source: io::Error,
    },
    /// The package or one of its parts does not have the expected structure.
    Format(String),
    /// A key is missing (exit code 6 in `nx_extract`).
    Keys(String),
    /// Data failed its hash check (exit code 3 in `nx_extract`), the paths of the damaged files.
    Damaged(Vec<String>),
    /// A case `nx_extract` handles that this port does not yet (named, never silently different).
    Unsupported(String),
}

impl ExtractError {
    /// An I/O error with context.
    pub fn io(context: impl Into<String>, source: io::Error) -> ExtractError {
        ExtractError::Io {
            context: context.into(),
            source,
        }
    }

    /// A format error.
    pub fn format(msg: impl Into<String>) -> ExtractError {
        ExtractError::Format(msg.into())
    }

    /// The exit code of this class: 1 error, 3 damaged data, 4 not supported yet, 6 missing key (2 is bad
    /// arguments, 130 interrupted, see `docs/integration-requests.md`).
    pub fn exit_code(&self) -> i32 {
        match self {
            ExtractError::Keys(_) => 6,
            ExtractError::Damaged(_) => 3,
            ExtractError::Unsupported(_) => 4,
            _ => 1,
        }
    }
}

impl fmt::Display for ExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExtractError::Io { context, source } => write!(f, "{context}: {source}"),
            ExtractError::Format(m) => write!(f, "invalid package: {m}"),
            ExtractError::Keys(m) => write!(f, "{m}"),
            ExtractError::Damaged(p) => write!(f, "{} files fail the NCA's hash check", p.len()),
            ExtractError::Unsupported(m) => write!(f, "not supported yet: {m}"),
        }
    }
}

impl std::error::Error for ExtractError {}

/// Result alias.
pub type Result<T> = std::result::Result<T, ExtractError>;

#[cfg(test)]
mod tests {
    use super::ExtractError;

    #[test]
    fn exit_codes() {
        assert_eq!(ExtractError::format("x").exit_code(), 1);
        assert_eq!(ExtractError::Damaged(vec![]).exit_code(), 3);
        assert_eq!(ExtractError::Unsupported("x".into()).exit_code(), 4);
        assert_eq!(ExtractError::Keys("x".into()).exit_code(), 6);
        assert_eq!(
            ExtractError::Unsupported("sparse".into()).to_string(),
            "not supported yet: sparse"
        );
    }
}
