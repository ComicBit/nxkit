//! Strict parser and type checker for the Metal Shading Language that the nxnative engine
//! generates, a port of the phase 1 prototype `msl_parse.py`.
//!
//! The checker accepts only the subset the corpus survey found and rejects everything else
//! with a reason key, which the real tool answers with a fallback to Apple's `metal`. The
//! decisions, reason keys, details and construct counters are the prototype's (see
//! [`parser`] for the rules that keep them identical). On top of the prototype it builds a
//! typed syntax tree ([`ast`]) for the IR lowering.
//!
//! ```
//! let r = msl_parse::check_source("#include <metal_stdlib>\nusing namespace metal;\n");
//! assert!(r.accepted());
//! ```

pub mod ast;
pub mod parser;
pub mod prelude;
pub mod stats;
pub mod ty;

use std::path::Path;

pub use parser::{Checked, Parser};
pub use stats::Counter;
pub use ty::{PyExc, Ty};

/// The prototype's `Reject` exception: a reason key and a free-form detail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reject {
    pub key: String,
    pub detail: String,
}

/// Why a checker method stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fail {
    /// The item is rejected. The parser goes on with the next item.
    Reject(Box<Reject>),
    /// The prototype would have raised a Python exception: the whole file fails.
    Crash(PyExc),
}

/// The outcome for one file, in the shape of the prototype's `parse_file` result.
#[derive(Debug)]
pub enum Outcome {
    /// The file was checked. Accepted when `reasons` is empty.
    Checked(Checked),
    /// The prototype's `RecursionError`: `parse_file` reports `{'recursion': 1}`.
    Recursion,
    /// The tokenizer failed (the prototype raises `LexError` and does not catch it).
    LexError(msl_lex::LexError),
    /// Another Python exception the prototype does not catch (it would end its run).
    Crash(PyExc),
}

impl Outcome {
    pub fn accepted(&self) -> bool {
        matches!(self, Outcome::Checked(c) if c.reasons.is_empty())
    }

    /// Reason keys with counts, as `parse_file` reports them. A lex error or another crash,
    /// which the prototype does not report, gets the key `lex-error` or `crash:<Exception>`.
    pub fn reasons(&self) -> Vec<(String, u64)> {
        match self {
            Outcome::Checked(c) => c.reasons.iter().map(|(k, v)| (k.to_string(), v)).collect(),
            Outcome::Recursion => vec![("recursion".into(), 1)],
            Outcome::LexError(_) => vec![("lex-error".into(), 1)],
            Outcome::Crash(e) => vec![(format!("crash:{}", e.name()), 1)],
        }
    }

    /// `parse_file`'s `detail` (the last rejection's detail).
    pub fn detail(&self) -> String {
        match self {
            Outcome::Checked(c) => c.last_detail.clone(),
            Outcome::LexError(e) => e.message.clone(),
            _ => String::new(),
        }
    }
}

/// Checks MSL source text (universal newlines are applied here).
pub fn check_source(src: &str) -> Outcome {
    let src = if src.contains('\r') {
        msl_lex::normalize_newlines(src.to_string())
    } else {
        src.to_string()
    };
    check_normalized(&src)
}

/// Checks source text that already has universal newlines.
pub fn check_normalized(src: &str) -> Outcome {
    check_with(src, None)
}

/// Checks source text (universal newlines applied) with an optional replacement helper table.
pub fn check_with(src: &str, table: Option<std::rc::Rc<prelude::Table>>) -> Outcome {
    let toks = match msl_lex::tokenize(src) {
        Ok(t) => t,
        Err(e) => return Outcome::LexError(e),
    };
    let mut parser = Parser::new(toks, src.len());
    if let Some(t) = table {
        parser = parser.with_prelude_table(t);
    }
    match parser.run() {
        Ok(c) => Outcome::Checked(c),
        Err(PyExc::RecursionError) => Outcome::Recursion,
        Err(e) => Outcome::Crash(e),
    }
}

/// Hashes of the template items in `src` (as the prototype's `--make-prelude` sees them).
pub fn template_hashes(src: &str) -> Result<Vec<String>, msl_lex::LexError> {
    let toks = msl_lex::tokenize(src)?;
    Ok(Parser::new(toks, src.len()).template_hashes())
}

/// Reads a file the way the prototype does: UTF-8 with replacement characters, universal newlines.
pub fn read_source(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    let text = match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => String::from_utf8_lossy(e.as_bytes()).into_owned(),
    };
    Ok(msl_lex::normalize_newlines(text))
}

/// Checks a file.
pub fn check_file(path: &Path) -> std::io::Result<Outcome> {
    let src = read_source(path)?;
    Ok(check_normalized(&src))
}
