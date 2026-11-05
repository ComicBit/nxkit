//! Names from package data checked before they are joined to an output folder: a hostile package must not write
//! outside it (`..`, absolute paths) or name the folder itself.

use std::path::PathBuf;

use crate::error::{ExtractError, Result};

/// One relative path under an output folder from the name `s` read in a package (`what` names the table for the error).
/// Leading `/` are removed (RomFS paths start with one). Refused: an empty path, empty, `.` or `..` parts, NUL bytes,
/// and with `single` any `/` (ExeFS names are one file name).
pub fn rel(what: &str, s: &str, single: bool) -> Result<PathBuf> {
    let t = s.trim_start_matches('/');
    let bad = t.is_empty()
        || t.contains('\0')
        || (single && (t.len() != s.len() || t.contains('/')))
        || t.split('/').any(|p| p.is_empty() || p == "." || p == "..");
    if bad {
        return Err(ExtractError::format(format!("unsafe {what} name {s:?}")));
    }
    Ok(PathBuf::from(t))
}

#[cfg(test)]
mod tests {
    use super::rel;

    #[test]
    fn refuses_escapes() {
        for s in [
            "", "/", "..", ".", "../x", "a/../b", "a//b", "a/", "x\0y", "/../etc",
        ] {
            assert!(rel("t", s, false).is_err(), "{s:?}");
        }
        for s in ["a/b", "/main", "main/x"] {
            assert!(rel("t", s, true).is_err(), "{s:?}");
        }
        assert_eq!(
            rel("t", "/a/b.bin", false).unwrap().to_str(),
            Some("a/b.bin")
        );
        assert_eq!(
            rel("t", "main.npdm", true).unwrap().to_str(),
            Some("main.npdm")
        );
        assert_eq!(rel("t", ".hidden", true).unwrap().to_str(), Some(".hidden"));
    }
}
