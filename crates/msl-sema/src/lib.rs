//! Semantic analysis for the own MSL front end.
//!
//! Input: the typed syntax tree of `msl-parse` (an accepted file). Output: a [`hir::Module`] in
//! which every name is resolved (locals, parameters, constant arrays, function constants, user
//! function overloads), every implicit conversion of C++/MSL is an explicit node, struct layouts
//! follow the MSL rules, and builtin calls carry the argument types of their overload. The
//! engine's template helpers (`dm_bs`, `spvFAdd`, `spvDepthCast`, `spvUnsafeArray`) are expanded
//! here.
//!
//! Constructs outside the supported subset produce [`Error::Unsupported`] with a stable key, which
//! the coverage tools count, the build falls back to Apple's compiler for such a file.

#![deny(missing_docs)]

pub mod builtins;
pub mod hir;
mod lower;
pub mod types;

use std::fmt;

pub use lower::analyze;

use hir::{Expr, ExprKind};
use types::Type;

/// Why analysis stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// A construct outside the supported subset, the key names the construct family.
    Unsupported(String),
    /// The input breaks a rule the checker should have caught (a bug or an unexpected form).
    Invalid(String),
}

impl Error {
    /// An unsupported construct.
    pub fn unsupported(key: impl Into<String>) -> Error {
        Error::Unsupported(key.into())
    }

    /// An invalid input.
    pub fn invalid(msg: impl Into<String>) -> Error {
        Error::Invalid(msg.into())
    }

    /// The key for coverage reports: `unsupported:<key>` or `invalid:<msg>`.
    pub fn key(&self) -> String {
        match self {
            Error::Unsupported(k) => format!("unsupported:{k}"),
            Error::Invalid(m) => format!("invalid:{m}"),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.key())
    }
}

impl std::error::Error for Error {}

/// Converts a value to type `to` with the implicit conversions MSL allows: scalar to scalar,
/// scalar to vector (broadcast), vector to vector of the same width, packed and unpacked.
pub fn conv(e: Expr, to: &Type) -> Result<Expr, Error> {
    let to = to.unpacked();
    if e.ty == to {
        return Ok(e);
    }
    match (&e.ty, &to) {
        (Type::Scalar(_), Type::Scalar(_)) => Ok(Expr::new(ExprKind::Convert(Box::new(e)), to)),
        (Type::Scalar(_), Type::Vector(s, _)) => {
            let el = conv(e, &Type::Scalar(*s))?;
            Ok(Expr::new(ExprKind::Splat(Box::new(el)), to))
        }
        (Type::Vector(_, a), Type::Vector(_, b)) if a == b => {
            Ok(Expr::new(ExprKind::Convert(Box::new(e)), to))
        }
        (Type::Texture(a), Type::Texture(b)) if a.kind == b.kind && a.elem == b.elem => Ok(e),
        (a, b) => Err(Error::invalid(format!("conversion {a} -> {b}"))),
    }
}
