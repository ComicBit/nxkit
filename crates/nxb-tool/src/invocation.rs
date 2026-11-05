//! What to run (`Invocation`) and what came back (`Completed`).

use crate::env::EnvPolicy;
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

/// Default stderr kept per job (the rest is drained and dropped).
pub const DEFAULT_MAX_STDERR: usize = 64 * 1024;

/// One child process run: argument vector only (never a shell), exact environment, optional stdin bytes, a deadline.
#[derive(Clone, Debug)]
pub struct Invocation {
    /// Absolute path of the executable (resolved once, never looked up through `PATH`).
    pub program: PathBuf,
    /// Arguments after the program name.
    pub args: Vec<OsString>,
    /// The child's whole environment.
    pub env: EnvPolicy,
    /// Working directory.`None` keeps the caller's.
    pub cwd: Option<PathBuf>,
    /// Bytes for stdin.`None` connects stdin to `/dev/null`.
    pub stdin: Option<Vec<u8>>,
    /// Kill the whole process group after this long.
    pub timeout: Duration,
    /// Stdout bytes kept, more is a failure (`ToolFailure::Io(FileTooLarge)`), never silent truncation.
    pub max_stdout: usize,
    /// Stderr bytes kept, more is drained and dropped.
    pub max_stderr: usize,
}

impl Invocation {
    /// An invocation with an empty environment, no stdin, a 30 minute timeout (design-v2 9.4 for `metal`), 1 GiB stdout
    /// and 64 KiB stderr limits.
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Invocation {
            program: program.into(),
            args: Vec::new(),
            env: EnvPolicy::empty(),
            cwd: None,
            stdin: None,
            timeout: Duration::from_secs(30 * 60),
            max_stdout: 1 << 30,
            max_stderr: DEFAULT_MAX_STDERR,
        }
    }

    /// Add one argument.
    pub fn arg(mut self, a: impl Into<OsString>) -> Self {
        self.args.push(a.into());
        self
    }

    /// Add arguments.
    pub fn args<I, S>(mut self, it: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.args.extend(it.into_iter().map(Into::into));
        self
    }

    /// Set the environment.
    pub fn env(mut self, env: EnvPolicy) -> Self {
        self.env = env;
        self
    }

    /// Set stdin bytes.
    pub fn stdin(mut self, bytes: Vec<u8>) -> Self {
        self.stdin = Some(bytes);
        self
    }

    /// Set the timeout.
    pub fn timeout(mut self, t: Duration) -> Self {
        self.timeout = t;
        self
    }

    /// Set the working directory.
    pub fn cwd(mut self, dir: impl Into<PathBuf>) -> Self {
        self.cwd = Some(dir.into());
        self
    }
}

/// A child that exited with status 0.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completed {
    /// Everything it wrote to stdout.
    pub stdout: Vec<u8>,
    /// The first `max_stderr` bytes of stderr (warnings).
    pub stderr: Vec<u8>,
    /// Wall time from spawn to exit.
    pub wall: Duration,
    /// The child's CPU time and peak RSS (`wait4`).
    pub usage: nxb_sys::process::ChildUsage,
}
