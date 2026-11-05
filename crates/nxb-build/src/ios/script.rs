//! What `build_ipa.sh` and `build_shell.sh` do with the shell's tools, done here: `ln -sfn`, `paste -sd,`, the chain
//! version `sed` reads from `dmsh_chain.h`, the fixed files both scripts `printf`, and the Apple tools they call
//! (`xcodegen`, `PlistBuddy`, `xcodebuild`, `git` for the commit line), each through [`Runner`].

use crate::host::Runner;
use std::path::{Path, PathBuf};

/// The entitlements both scripts write (Increased Memory Limit).
pub const ENTITLEMENTS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict><key>com.apple.developer.kernel.increased-memory-limit</key><true/></dict></plist>\n";

/// `Gen/Assets.xcassets/Contents.json`.
pub const ASSETS: &str = "{\"info\":{\"author\":\"xcode\",\"version\":1}}\n";

/// The default standard font (`STD_FONT`).
pub const STD_FONT: &str = "/System/Library/Fonts/Supplemental/Arial Unicode.ttf";

/// A failure as the scripts' `die` reports it (`FAIL: <what>`).
pub fn die(what: impl std::fmt::Display) -> String {
    format!("FAIL: {what}")
}

/// `mkdir -p`.
pub fn mkdir_p(p: &Path) -> Result<(), String> {
    std::fs::create_dir_all(p).map_err(|e| die(format!("mkdir {}: {e}", p.display())))
}

/// `rm -rf` of one path we made (a folder or a file, nothing when missing).
pub fn rm_rf(p: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() => std::fs::remove_dir_all(p),
        Ok(_) => std::fs::remove_file(p),
        Err(_) => Ok(()),
    }
    .map_err(|e| die(format!("rm {}: {e}", p.display())))
}

/// `ln -sfn target link`: a symlink or file at `link` is replaced, a real folder there gets the link inside it.
pub fn ln_sfn(target: &Path, link: &Path) -> Result<(), String> {
    let mut at = link.to_path_buf();
    match std::fs::symlink_metadata(link) {
        Ok(m) if m.is_dir() => {
            at = link.join(target.file_name().unwrap_or_default());
            let _ = std::fs::remove_file(&at);
        }
        Ok(_) => {
            std::fs::remove_file(link).map_err(|e| die(format!("link {}: {e}", link.display())))?
        }
        Err(_) => {}
    }
    std::os::unix::fs::symlink(target, &at).map_err(|e| die(format!("link {}: {e}", at.display())))
}

/// `$(cd dir && pwd)`: the absolute, normalised path without resolving symlinks.
pub fn pwd_of(dir: &Path) -> Result<PathBuf, String> {
    let s = dir
        .to_str()
        .ok_or_else(|| die(format!("{}: not UTF-8", dir.display())))?;
    if !dir.is_dir() {
        return Err(die(format!("cd {s}")));
    }
    Ok(PathBuf::from(super::abspath(s)?))
}

/// `$(paste -sd, file 2>/dev/null || true)`: the lines joined by commas, empty without the file.
pub fn paste_commas(file: &Path) -> String {
    std::fs::read_to_string(file)
        .map(|t| {
            let t = t.strip_suffix('\n').unwrap_or(&t);
            if t.is_empty() {
                String::new()
            } else {
                t.split('\n').collect::<Vec<_>>().join(",")
            }
        })
        .unwrap_or_default()
}

/// `$(sed -n 's/^#define DMSH_CHAIN "\(.*\)"/\1/p' dmsh_chain.h)`: every matching line's version (greedy, as `sed`).
pub fn chain_version(chain_h: &Path) -> String {
    let text = std::fs::read(chain_h).unwrap_or_default();
    let text = String::from_utf8_lossy(&text);
    let v: Vec<String> = text
        .split('\n')
        .filter_map(|l| {
            let r = l.strip_prefix("#define DMSH_CHAIN \"")?;
            let end = r.rfind('"')?;
            Some(format!("{}{}", &r[..end], &r[end + 1..]))
        })
        .collect();
    v.join("\n")
}

/// `${VAR-default}` tests in the scripts: unset and set-but-empty differ.
pub fn set_nonempty(v: &Option<String>) -> bool {
    v.as_deref().is_some_and(|s| !s.is_empty())
}

/// The commit line (`DMCommit`): `git -C <dm> rev-parse --short=12 HEAD`, `+dirty` with uncommitted changes to tracked
/// files.`None` outside a checkout.
pub fn commit(r: &Runner, dm: &Path) -> Option<String> {
    let dm = dm.to_str()?;
    let c = r
        .stdout("git", ["-C", dm, "rev-parse", "--short=12", "HEAD"], None)
        .ok()?;
    let c = c.trim_end_matches('\n').to_string();
    let dirty = r
        .stdout(
            "git",
            ["-C", dm, "status", "--porcelain", "--untracked-files=no"],
            None,
        )
        .unwrap_or_default();
    Some(if dirty.trim_end_matches('\n').is_empty() {
        c
    } else {
        format!("{c}+dirty")
    })
}

/// `/usr/libexec/PlistBuddy -c "Add :<key> <type> <value>" <plist>`.
pub fn plist_add(r: &Runner, plist: &Path, key: &str, ty: &str, value: &str) -> Result<(), String> {
    let cmd = format!("Add :{key} {ty} {value}");
    r.run(
        "/usr/libexec/PlistBuddy",
        ["-c".as_ref(), cmd.as_ref(), plist.as_os_str()],
        None,
    )
    .map(|_| ())
    .map_err(|_| die(key))
}

/// `xcodegen generate -q [--spec <spec>] > /dev/null` in `dir`.
pub fn xcodegen(r: &Runner, dir: &Path, spec: Option<&str>) -> Result<(), String> {
    let mut a = vec!["generate", "-q"];
    if let Some(s) = spec {
        a.extend(["--spec", s]);
    }
    r.run("xcodegen", a, Some(dir))
        .map(|_| ())
        .map_err(|e| die(format!("xcodegen ({e})")))
}

/// `xcodebuild <args> > log 2>&1`, on failure the log's first 20 `error:` lines and `die "xcodebuild, log <log>"`.
pub fn xcodebuild(
    r: &Runner,
    dir: &Path,
    args: &[String],
    log: &Path,
) -> Result<(), (String, Vec<String>)> {
    match run_logged(r, "xcodebuild", args, Some(dir), log) {
        Ok(()) => Ok(()),
        Err(_) => {
            let text = std::fs::read(log).unwrap_or_default();
            let errors: Vec<String> = String::from_utf8_lossy(&text)
                .lines()
                .filter(|l| l.contains("error:"))
                .take(20)
                .map(str::to_string)
                .collect();
            Err((die(format!("xcodebuild, log {}", log.display())), errors))
        }
    }
}

/// `<program> <args> > log 2>&1` (through `/bin/sh`, so both streams interleave in the log as in the scripts).
pub fn run_logged(
    r: &Runner,
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
    log: &Path,
) -> Result<(), String> {
    let log = log
        .to_str()
        .ok_or_else(|| die(format!("{}: not UTF-8", log.display())))?;
    let mut a: Vec<&str> = vec!["-c", "exec \"$@\" > \"$0\" 2>&1", log, program];
    a.extend(args.iter().map(String::as_str));
    r.run("/bin/sh", a, cwd).map(|_| ())
}

/// `$(du -sh path | cut -f1)`.
pub fn du_sh(r: &Runner, p: &Path) -> String {
    r.stdout("du", ["-sh".as_ref(), p.as_os_str()], None)
        .ok()
        .and_then(|o| o.split('\t').next().map(str::to_string))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ios::oracle;

    #[test]
    fn shell_helpers() {
        let d = oracle::scratch("script");
        std::fs::write(d.join("f.txt"), "a\nb\n\nc\n").unwrap();
        assert_eq!(paste_commas(&d.join("f.txt")), "a,b,,c");
        assert_eq!(paste_commas(&d.join("none")), "");
        std::fs::write(
            d.join("c.h"),
            "#define DMSH_CHAIN \"0ad5\" x\"\n#define DMSH_CHAIN 1\n",
        )
        .unwrap();
        assert_eq!(chain_version(&d.join("c.h")), "0ad5\" x");
        std::fs::write(d.join("c2.h"), "#define DMSH_CHAIN \"ab\" // c\n").unwrap();
        assert_eq!(chain_version(&d.join("c2.h")), "ab // c");
        std::fs::create_dir_all(d.join("t1")).unwrap();
        std::fs::create_dir_all(d.join("t2")).unwrap();
        ln_sfn(&d.join("t1"), &d.join("l")).unwrap();
        ln_sfn(&d.join("t2"), &d.join("l")).unwrap();
        assert_eq!(std::fs::read_link(d.join("l")).unwrap(), d.join("t2"));
        assert!(set_nonempty(&Some("x".into())) && !set_nonempty(&Some(String::new())));
        let _ = std::fs::remove_dir_all(&d);
    }
}
