//! `ToolError`: one external tool job failed.

use crate::env::EnvError;
use std::fmt;
use std::io;
use std::time::Duration;

/// How a tool job failed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ToolFailure {
    /// Non-zero exit code.
    Exit(i32),
    /// Killed by a signal (crash).
    Signal(i32),
    /// Killed by the watchdog after this long.
    Timeout(Duration),
    /// The process could not be started.
    Spawn(io::ErrorKind),
    /// Talking to the process failed (pipe error).
    Io(io::ErrorKind),
    /// The run was cancelled (interrupt or abort of the build).
    Cancelled,
}

impl ToolFailure {
    /// Short stable label used by the circuit breaker's classification.
    pub fn label(&self) -> String {
        match self {
            ToolFailure::Exit(c) => format!("exit {c}"),
            ToolFailure::Signal(s) => format!("signal {s}"),
            ToolFailure::Timeout(_) => "timeout".to_string(),
            ToolFailure::Spawn(k) => format!("spawn {k:?}"),
            ToolFailure::Io(k) => format!("io {k:?}"),
            ToolFailure::Cancelled => "cancelled".to_string(),
        }
    }
}

/// One tool job failed. Recovery decides whether it becomes a shader error or escalates to an `EnvError`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ToolError {
    /// Tool name (`metal`, `metallib`, `spirv-cross`, `dmsh_xlate`, …).
    pub tool: String,
    /// What happened.
    pub failure: ToolFailure,
    /// The first part of stderr, lossy UTF-8, at most a few KiB.
    pub stderr_head: String,
    /// Set when the failure is a machine problem (for example `EAGAIN` at spawn, the toolchain gone): the caller aborts
    /// instead of failing one shader.
    pub env: Option<EnvError>,
}

impl ToolError {
    /// A tool error without stderr.
    pub fn new(tool: impl Into<String>, failure: ToolFailure) -> Self {
        ToolError {
            tool: tool.into(),
            failure,
            stderr_head: String::new(),
            env: None,
        }
    }

    /// The same error marked as a machine problem.
    pub fn with_env(mut self, env: Option<EnvError>) -> Self {
        self.env = env;
        self
    }

    /// The same error with stderr text attached.
    pub fn with_stderr(mut self, stderr_head: impl Into<String>) -> Self {
        self.stderr_head = stderr_head.into();
        self
    }

    /// The first stderr line with digits replaced by `#` and paths cut: the circuit breaker's normalised class.
    pub fn first_line_normalised(&self) -> String {
        let line = self.stderr_head.lines().next().unwrap_or("");
        let mut out = String::with_capacity(line.len().min(200));
        let mut last_hash = false;
        for ch in line.chars().take(200) {
            if ch.is_ascii_digit() {
                if !last_hash {
                    out.push('#');
                }
                last_hash = true;
            } else {
                out.push(ch);
                last_hash = false;
            }
        }
        // Drop path-like tokens: they differ per shader.
        out.split_whitespace()
            .filter(|t| !t.contains('/'))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} failed: {}", self.tool, self.failure.label())?;
        if let Some(line) = self.stderr_head.lines().next() {
            write!(f, ": {line}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ToolError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalised_line_ignores_numbers_and_paths() {
        let mut a = ToolError::new("metal", ToolFailure::Exit(1));
        a.stderr_head = "/tmp/x/a.metal:12:3: error: use of undeclared identifier\nmore".into();
        let mut b = a.clone();
        b.stderr_head = "/tmp/y/b.metal:7:44: error: use of undeclared identifier".into();
        assert_eq!(a.first_line_normalised(), b.first_line_normalised());
        assert!(a.first_line_normalised().contains("undeclared identifier"));
    }

    #[test]
    fn display() {
        let mut e = ToolError::new("metallib", ToolFailure::Signal(9));
        e.stderr_head = "boom\nsecond".into();
        assert_eq!(e.to_string(), "metallib failed: signal 9: boom");
    }
}
