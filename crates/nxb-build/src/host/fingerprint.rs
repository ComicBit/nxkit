//! The NSP script's `fingerprint`: SHA-256 over a stage's inputs, the reuse key of the Mac host (`<host>.key`) and its
//! decoder identity (`<host>.decoder.json`). Same bytes hashed in the same order, so the keys equal the script's and
//! either side reuses the other's build.

use super::Runner;
use super::pyfmt::{posix_path, repr_str};
use nxb_model::{Sha256, sha256};
use std::path::{Path, PathBuf};

/// One input of a fingerprint.
#[derive(Clone, Debug)]
pub enum Item {
    /// `('files', paths)`: each file's name and the SHA-256 of its content (`missing` when unreadable).
    Files(Vec<PathBuf>),
    /// `('tree', folder)`: relative path and size of every file under it (symlinks not followed).
    Tree(PathBuf),
    /// `('cmd', argv)`: the tool's stdout + stderr (`missing` when it cannot start).
    Cmd(Vec<String>),
    /// Anything else: its Python `repr` (the caller writes it, see [`repr_path`], [`repr_list`]).
    Repr(String),
}

/// `repr(Path(p))` (`PosixPath('...')`).
pub fn repr_path(p: &Path) -> String {
    format!("PosixPath({})", repr_str(&posix_path(&p.to_string_lossy())))
}

/// `repr(list)` of already written element reprs.
pub fn repr_list(items: &[String]) -> String {
    format!("[{}]", items.join(", "))
}

/// `files_under(d)`: (relative path, size) of every entry under `d` that is not a folder (symlinks not followed),
/// sorted by path, nothing when `d` cannot be read.
pub fn files_under(d: &Path) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let mut todo = vec![d.to_path_buf()];
    while let Some(dir) = todo.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let Ok(md) = std::fs::symlink_metadata(e.path()) else {
                continue;
            };
            if md.is_dir() {
                todo.push(e.path());
            } else {
                let rel = e
                    .path()
                    .strip_prefix(d)
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                out.push((rel, md.len()));
            }
        }
    }
    out.sort();
    out
}

/// The hex SHA-256 over `items`, run tools with `r`'s environment.
pub fn fingerprint(items: &[Item], r: &Runner) -> String {
    let mut h = Sha256::new();
    for it in items {
        let kind = match it {
            Item::Files(_) => "files",
            Item::Tree(_) => "tree",
            Item::Cmd(_) => "cmd",
            Item::Repr(_) => "repr",
        };
        h.update(format!("\0{kind}\0").as_bytes());
        match it {
            Item::Files(paths) => {
                for p in paths {
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    h.update(name.as_bytes());
                    h.update(b"\0");
                    match std::fs::read(p) {
                        Ok(b) => h.update(sha256(&b).as_bytes()),
                        Err(_) => h.update(b"missing"),
                    }
                }
            }
            Item::Tree(d) => {
                for (rel, size) in files_under(d) {
                    h.update(format!("{rel}\0{size}\0\n").as_bytes());
                }
            }
            Item::Cmd(argv) => {
                let out = argv.split_first().and_then(|(prog, args)| {
                    r.which(prog).ok()?;
                    let c = r.run(prog, args, None).ok()?;
                    let mut t = String::from_utf8_lossy(&c.stdout).into_owned();
                    t.push_str(&String::from_utf8_lossy(&c.stderr));
                    Some(t.replace("\r\n", "\n").replace('\r', "\n"))
                });
                match out {
                    Some(t) => h.update(t.as_bytes()),
                    None => h.update(b"missing"),
                }
            }
            Item::Repr(text) => h.update(text.as_bytes()),
        }
    }
    h.finalize().to_hex()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nxb_tool::EnvPolicy;

    /// Expected digests come from the NSP script's `fingerprint` on the same inputs (the script imported, run once on the same files).
    #[test]
    fn same_bytes_as_the_script() {
        let r = Runner::new(EnvPolicy::empty().set("PATH", "/usr/bin:/bin").unwrap());
        let d = std::env::temp_dir().join(format!("nxb-fp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("t/sub")).unwrap();
        std::fs::write(d.join("a.c"), b"int a;\n").unwrap();
        std::fs::write(d.join("t/x"), b"12345").unwrap();
        std::fs::write(d.join("t/sub/y"), b"").unwrap();
        assert_eq!(
            files_under(&d.join("t")),
            vec![("sub/y".to_string(), 0), ("x".to_string(), 5)]
        );
        let items = [
            Item::Repr(repr_list(&[
                repr_str("clang"),
                repr_path(Path::new("/w/tools/dmhost")),
            ])),
            Item::Files(vec![d.join("a.c"), d.join("missing.h")]),
            Item::Tree(d.join("t")),
            Item::Cmd(vec!["echo".into(), "v1".into()]),
            Item::Cmd(vec!["no-such-tool-nxb".into()]),
            Item::Repr("None".into()),
        ];
        assert_eq!(
            fingerprint(&items, &r),
            "5381ccb5246a6b2e651c81f0d7cb7623a6dc5b3f6c3576a10d3c69ac53180ca6"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }
}
