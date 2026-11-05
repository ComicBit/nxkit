//! The iOS stages of the engine's `dm/ios/tools` (the IPA flow of `build_ipa.sh`, the route C shell of
//! `build_shell.sh` and the generators they call), ported one by one with the reference scripts as the oracle.
//! Apple's tools (`xcodegen`, `xcodebuild`, `sips`, `PlistBuddy`, `plutil`) stay external and run through
//! [`crate::host::Runner`] (the [`nxb_tool::Tool`] trait).

pub mod embed;
pub mod etree;
pub mod extfont;
mod fontpen;
pub mod genbuild;
pub mod ipa;
mod ipafw;
mod ipagen;
pub mod ipapack;
mod ipaprog;
pub mod manifest;
pub mod nacp;
pub mod pso;
pub mod resources;
pub mod romfs;
pub mod script;
pub mod shell;
mod ttf;
mod ttf_read;
pub mod workspace;

/// `shlex.quote`.
pub fn shlex_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".into();
    }
    let safe = |c: char| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c);
    if s.chars().all(safe) {
        return s.into();
    }
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

/// `str(pathlib.Path(p).resolve())` (non-strict, Python 3.12): absolute, symlinks of the existing part resolved, the
/// rest joined as written (`..` drops the last component).
pub fn resolve(p: &str) -> Result<String, String> {
    fn walk(base: String, rest: &str, depth: u32) -> String {
        let mut path = base;
        for name in rest.split('/') {
            match name {
                "" | "." => continue,
                ".." => {
                    path = match path.rfind('/') {
                        Some(0) | None => "/".into(),
                        Some(i) => path[..i].to_string(),
                    };
                    continue;
                }
                _ => {}
            }
            let next = if path == "/" {
                format!("/{name}")
            } else {
                format!("{path}/{name}")
            };
            match std::fs::read_link(&next) {
                Ok(t) if depth < 40 => {
                    let t = t.to_string_lossy().into_owned();
                    path = if t.starts_with('/') {
                        walk("/".into(), &t, depth + 1)
                    } else {
                        walk(path, &t, depth + 1)
                    };
                }
                _ => path = next,
            }
        }
        path
    }
    let abs = abspath_raw(p)?;
    Ok(walk("/".into(), &abs, 0))
}

fn abspath_raw(p: &str) -> Result<String, String> {
    if p.starts_with('/') {
        return Ok(p.into());
    }
    let cwd = std::env::current_dir().map_err(|e| format!("current folder: {e}"))?;
    let cwd = cwd
        .to_str()
        .ok_or_else(|| format!("{}: not UTF-8", cwd.display()))?;
    Ok(format!("{cwd}/{p}"))
}

/// `os.path.normpath` of a POSIX path (text only: `..` is not resolved through symlinks).
pub fn normpath(p: &str) -> String {
    if p.is_empty() {
        return ".".into();
    }
    let lead = p.len() - p.trim_start_matches('/').len();
    let root = match lead {
        0 => "",
        2 => "//",
        _ => "/",
    };
    let mut out: Vec<&str> = Vec::new();
    for c in p.split('/') {
        match c {
            "" | "." => {}
            ".." if out.last().is_some_and(|l| *l != "..") => {
                out.pop();
            }
            ".." if !root.is_empty() => {}
            c => out.push(c),
        }
    }
    let s = format!("{root}{}", out.join("/"));
    if s.is_empty() { ".".into() } else { s }
}

/// `os.path.abspath`: joined to the current folder when relative, then [`normpath`].
pub fn abspath(p: &str) -> Result<String, String> {
    Ok(normpath(&abspath_raw(p)?))
}

#[cfg(test)]
pub(crate) mod oracle {
    //! The reference scripts for the oracle tests: `NXB_ORACLE_ENGINE` (the engine folder) and `NXB_ORACLE_PYTHON`
    //! (default `python3`). The tests skip without `NXB_ORACLE_ENGINE`.
    use std::path::{Path, PathBuf};
    use std::process::Command;

    /// The engine folder, or `None` (the test skips).
    pub(crate) fn engine() -> Option<PathBuf> {
        std::env::var_os("NXB_ORACLE_ENGINE").map(PathBuf::from)
    }

    /// A command running the Python of the oracle.
    pub(crate) fn python() -> Command {
        Command::new(std::env::var("NXB_ORACLE_PYTHON").unwrap_or_else(|_| "python3".into()))
    }

    /// Every file under `root` (symlinks as their targets) with the root's path
    /// (as given and resolved) replaced by `<root>`, sorted by relative path.
    pub(crate) fn tree(root: &Path) -> Vec<(String, Vec<u8>)> {
        let real = std::fs::canonicalize(root).unwrap();
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                let rel = p.strip_prefix(root).unwrap().to_string_lossy().into_owned();
                let m = std::fs::symlink_metadata(&p).unwrap();
                let data = if m.file_type().is_symlink() {
                    std::fs::read_link(&p)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned()
                        .into_bytes()
                } else if m.is_dir() {
                    stack.push(p);
                    continue;
                } else {
                    std::fs::read(&p).unwrap()
                };
                let text = String::from_utf8_lossy(&data)
                    .replace(real.to_str().unwrap(), "<root>")
                    .replace(root.to_str().unwrap(), "<root>");
                out.push((rel, text.into_bytes()));
            }
        }
        out.sort();
        out
    }

    /// A new empty scratch folder for one test.
    pub(crate) fn scratch(tag: &str) -> PathBuf {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("nxb-ios-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn normpath_is_pythons() {
        for (p, want) in [
            ("", "."),
            ("a/./b//c/", "a/b/c"),
            ("/../a/../../b", "/b"),
            ("../a/..", ".."),
            ("a/../..", ".."),
            ("//x/y", "//x/y"),
            ("///x/../y", "/y"),
            ("a/..", "."),
        ] {
            assert_eq!(super::normpath(p), want, "{p}");
        }
    }
}
