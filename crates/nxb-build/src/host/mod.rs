//! The Mac stages around the app that the NSP script runs as tools: the app icon (`sips`, `iconutil`), the companion
//! app (`clang`, `swiftc`), the app settings (`dm/games/settings.py`, ported) and the Mac host (`make`, `clang`).
//! Every child runs through [`nxb_tool::Tool`] (own process group, exact environment, timeout).

pub mod app;
pub mod build_sources;
pub mod companion;
pub mod fingerprint;
pub mod gpu;
pub mod guess;
pub mod guess_check;
pub mod guess_dump;
pub mod guess_facts;
pub mod guess_src;
#[cfg(test)]
mod guess_tests;
pub mod header;
pub mod header_multi;
pub mod icon;
pub mod machost;
pub mod nsofacts;
pub mod profile;
pub mod profile_cli;
pub mod profile_tsv;
pub mod pyfmt;
pub mod rom;
pub mod settings;
#[cfg(test)]
mod settings_tests;
pub mod survey;
pub mod survey_extract;
pub mod survey_profile;
#[cfg(test)]
mod survey_tests;
pub mod ue_paks;

use nxb_tool::{Completed, EnvPolicy, Invocation, ProcessRegistry, ProcessTool, Tool, ToolId};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// The folders the NSP script puts in front of `PATH` (`TOOL_PATHS`: GUI apps do not get the shell's `PATH`).
pub const TOOL_PATHS: &[&str] = &[
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/usr/local/share/dotnet",
    "~/.dotnet",
];

/// Runs the stages' tools: one registry (cancellation) and one environment for all of them.
#[derive(Clone, Debug)]
pub struct Runner {
    reg: Arc<ProcessRegistry>,
    env: EnvPolicy,
    path: String,
}

static GAMES: std::sync::OnceLock<(PathBuf, bool)> = std::sync::OnceLock::new();

/// The folder holding a provisional profile and whether it goes before the engine's own: every [`Runner::from_parent`]
/// after this passes them on as `DM_GAMES_DIR` and `DM_GAMES_FIRST` (the process environment is left alone: the
/// rewrite refuses an environment that names profiles outside the engine).
pub fn set_games(dir: PathBuf, first: bool) {
    let _ = GAMES.set((dir, first));
}

/// What [`set_games`] set.
pub(crate) fn games() -> Option<&'static (PathBuf, bool)> {
    GAMES.get()
}

impl Runner {
    /// A runner with this environment (`PATH` in it is the search path for programs given by name).
    pub fn new(env: EnvPolicy) -> Self {
        let path = env
            .get("PATH")
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "/usr/bin:/bin".into());
        Runner {
            reg: Arc::new(ProcessRegistry::new()),
            env,
            path,
        }
    }

    /// The NSP script's environment for its tools: `PATH` = [`TOOL_PATHS`] + the caller's `PATH` (else
    /// `/usr/bin:/bin:/usr/sbin:/sbin`), and the caller's `HOME`, `TMPDIR`, `USER`, `LANG`, `DM_GAMES_DIR`,
    /// `DM_GAMES_FIRST` when set. The deny list of [`EnvPolicy`] still applies (`DEVELOPER_DIR` never reaches a child).
    pub fn from_parent() -> Result<Self, String> {
        let home = std::env::var("HOME").unwrap_or_default();
        let mut dirs: Vec<String> = TOOL_PATHS
            .iter()
            .map(|d| match d.strip_prefix("~/") {
                Some(rest) if !home.is_empty() => format!("{home}/{rest}"),
                _ => (*d).to_string(),
            })
            .collect();
        dirs.push(std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin:/usr/sbin:/sbin".into()));
        let mut env = EnvPolicy::empty()
            .set("PATH", dirs.join(":"))
            .map_err(|e| e.to_string())?;
        for name in [
            "HOME",
            "TMPDIR",
            "USER",
            "LANG",
            "DM_GAMES_DIR",
            "DM_GAMES_FIRST",
        ] {
            if let Some(v) = std::env::var_os(name) {
                env = env.set(name, v).map_err(|e| e.to_string())?;
            }
        }
        if let Some((dir, first)) = GAMES.get() {
            env = env.set("DM_GAMES_DIR", dir).map_err(|e| e.to_string())?;
            if *first {
                env = env.set("DM_GAMES_FIRST", "1").map_err(|e| e.to_string())?;
            }
        }
        Ok(Runner::new(env))
    }

    /// The same runner with one more variable.
    pub fn with_var(&self, name: &str, value: impl Into<OsString>) -> Result<Self, String> {
        let mut r = self.clone();
        r.env = r.env.set(name, value).map_err(|e| e.to_string())?;
        Ok(r)
    }

    /// The environment the children get.
    pub fn env(&self) -> &EnvPolicy {
        &self.env
    }

    /// The program's absolute path: as given when it has a `/`, else the first match on `PATH`.
    pub fn which(&self, program: &str) -> Result<PathBuf, String> {
        if program.contains('/') {
            return Ok(PathBuf::from(program));
        }
        self.path
            .split(':')
            .filter(|d| !d.is_empty())
            .map(|d| Path::new(d).join(program))
            .find(|p| p.is_file())
            .ok_or_else(|| format!("{program}: not found on PATH"))
    }

    /// Run one program to completion, a non-zero exit is an error with the end of its output (as `pack_app.py`'s
    /// `run`: `<tool> failed (<code>): <stderr or stdout>`).
    pub fn run<I, S>(&self, program: &str, args: I, cwd: Option<&Path>) -> Result<Completed, String>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let path = self.which(program)?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| program.to_string());
        let tool = ProcessTool::new(
            ToolId {
                name: name.clone(),
                path,
                version: String::new(),
            },
            self.reg.clone(),
        );
        let mut inv: Invocation = tool
            .invocation()
            .args(args)
            .env(self.env.clone())
            .timeout(Duration::from_secs(60 * 60));
        if let Some(d) = cwd {
            inv = inv.cwd(d);
        }
        tool.run(&inv).map_err(|e| {
            let text = if e.stderr_head.trim().is_empty() {
                e.failure.label()
            } else {
                tail(e.stderr_head.trim(), 1500).to_string()
            };
            format!("{name} failed ({}): {text}", e.failure.label())
        })
    }

    /// [`Runner::run`] and its stdout as text.
    pub fn stdout<I, S>(&self, program: &str, args: I, cwd: Option<&Path>) -> Result<String, String>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.run(program, args, cwd)
            .map(|c| String::from_utf8_lossy(&c.stdout).into_owned())
    }
}

/// The last `n` bytes of `s` (on a character boundary).
fn tail(s: &str, n: usize) -> &str {
    let mut i = s.len().saturating_sub(n);
    while !s.is_char_boundary(i) {
        i += 1;
    }
    &s[i..]
}

/// A temporary folder inside `parent` (as Python's `tempfile.TemporaryDirectory(dir=parent)`), removed on drop.
pub struct TempDir(PathBuf);

impl TempDir {
    /// Make a new, empty folder in `parent`.
    pub fn new_in(parent: &Path) -> Result<Self, String> {
        static N: AtomicU64 = AtomicU64::new(0);
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        for _ in 0..100 {
            let n = N.fetch_add(1, Ordering::Relaxed);
            let p = parent.join(format!("tmp-nxb-{}-{n}", std::process::id()));
            match std::fs::create_dir(&p) {
                Ok(()) => return Ok(TempDir(p)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(format!("{}: {e}", p.display())),
            }
        }
        Err(format!("no temporary folder in {}", parent.display()))
    }

    /// Its path.
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_and_reports_failures() {
        let r = Runner::new(EnvPolicy::empty().set("PATH", "/usr/bin:/bin").unwrap());
        assert_eq!(r.stdout("echo", ["hi"], None).unwrap(), "hi\n");
        let e = r
            .run("sh", ["-c", "echo bad >&2; exit 3"], None)
            .unwrap_err();
        assert_eq!(e, "sh failed (exit 3): bad");
        assert!(r.which("no-such-tool-nxb").is_err());
    }

    #[test]
    fn temp_dir_is_removed() {
        let base = std::env::temp_dir().join(format!("nxb-host-tmp-{}", std::process::id()));
        let p = {
            let t = TempDir::new_in(&base).unwrap();
            std::fs::write(t.path().join("x"), b"1").unwrap();
            t.path().to_path_buf()
        };
        assert!(!p.exists());
        let _ = std::fs::remove_dir(&base);
    }
}
