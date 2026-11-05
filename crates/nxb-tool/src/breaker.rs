//! The circuit breaker (design-v2 11.3): repeated identical tool failures become one `EnvError` instead of thousands of
//! shader failures and a bad reuse key.

use nxb_error::{EnvError, EnvKind, ToolError};
use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

/// Thresholds, configurable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BreakerConfig {
    /// Identical classified failures in a row that escalate (default 8).
    pub same_in_a_row: u32,
    /// The first jobs of a tool watched for the failure share (default 200).
    pub window: u32,
}

impl Default for BreakerConfig {
    fn default() -> Self {
        BreakerConfig {
            same_in_a_row: 8,
            window: 200,
        }
    }
}

#[derive(Debug, Default)]
struct ToolStats {
    jobs: u32,
    failures_in_window: u32,
    last_class: Option<String>,
    run: u32,
    env_counts: BTreeMap<EnvKind, u32>,
}

/// Watches tool outcomes per tool name. Thread-safe.
#[derive(Debug, Default)]
pub struct CircuitBreaker {
    cfg: BreakerConfig,
    tools: Mutex<BTreeMap<String, ToolStats>>,
}

impl CircuitBreaker {
    /// A breaker with these thresholds.
    pub fn new(cfg: BreakerConfig) -> Self {
        CircuitBreaker {
            cfg,
            tools: Mutex::new(BTreeMap::new()),
        }
    }

    /// Record one job's outcome. `Err` means: abort the run with this `EnvError`.
    pub fn record(&self, tool: &str, outcome: Result<(), &ToolError>) -> Result<(), EnvError> {
        let mut tools = self.tools.lock().unwrap_or_else(PoisonError::into_inner);
        let st = tools.entry(tool.to_string()).or_default();
        st.jobs = st.jobs.saturating_add(1);
        let err = match outcome {
            Ok(()) => {
                st.run = 0;
                st.last_class = None;
                return Ok(());
            }
            Err(e) => e,
        };
        if let Some(env) = &err.env {
            let n = st.env_counts.entry(env.kind).or_default();
            *n += 1;
            if *n >= env.kind.threshold() {
                return Err(env.clone());
            }
        }
        if st.jobs <= self.cfg.window {
            st.failures_in_window += 1;
            if st.failures_in_window * 2 > self.cfg.window {
                return Err(escalate(
                    tool,
                    err,
                    "more than half of the first jobs failed",
                ));
            }
        }
        let class = format!("{} | {}", err.failure.label(), err.first_line_normalised());
        if st.last_class.as_deref() == Some(class.as_str()) {
            st.run += 1;
        } else {
            st.last_class = Some(class);
            st.run = 1;
        }
        if st.run >= self.cfg.same_in_a_row {
            return Err(escalate(tool, err, "the same failure repeats"));
        }
        Ok(())
    }
}

fn escalate(tool: &str, err: &ToolError, why: &str) -> EnvError {
    let sample = err.stderr_head.lines().next().unwrap_or("");
    EnvError::new(
        EnvKind::RepeatedToolFailure,
        format!("{tool}: {why} ({}: {sample})", err.failure.label()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use nxb_error::{IoOp, ToolFailure};
    use nxb_testkit::check;

    fn fail(line: &str) -> ToolError {
        ToolError::new("metal", ToolFailure::Exit(1)).with_stderr(line)
    }

    #[test]
    fn eight_identical_in_a_row() {
        let b = CircuitBreaker::new(BreakerConfig {
            same_in_a_row: 8,
            window: 0,
        });
        for _ in 0..7 {
            b.record("metal", Err(&fail("error: x"))).unwrap();
        }
        let e = b.record("metal", Err(&fail("error: x"))).unwrap_err();
        assert_eq!(e.kind, EnvKind::RepeatedToolFailure);
    }

    #[test]
    fn success_resets_the_run() {
        let b = CircuitBreaker::new(BreakerConfig {
            same_in_a_row: 8,
            window: 0,
        });
        for _ in 0..7 {
            b.record("metal", Err(&fail("error: x"))).unwrap();
        }
        b.record("metal", Ok(())).unwrap();
        for _ in 0..7 {
            b.record("metal", Err(&fail("error: x"))).unwrap();
        }
    }

    #[test]
    fn tools_are_independent() {
        let b = CircuitBreaker::new(BreakerConfig {
            same_in_a_row: 3,
            window: 0,
        });
        for _ in 0..2 {
            b.record("metal", Err(&fail("e"))).unwrap();
            b.record("metallib", Err(&fail("e"))).unwrap();
        }
    }

    #[test]
    fn more_than_half_of_window() {
        let b = CircuitBreaker::new(BreakerConfig {
            same_in_a_row: 1000,
            window: 200,
        });
        // 100 failures and 99 successes in the first 199 jobs: exactly half or less, no trip.
        for i in 0..199u32 {
            let r = if i % 2 == 0 {
                b.record("x", Err(&fail(&format!("error kind{}", i % 7))))
            } else {
                b.record("x", Ok(()))
            };
            assert!(r.is_ok(), "job {i}");
        }
        // The 101st failure inside the window trips.
        assert!(b.record("x", Err(&fail("error last"))).is_err());
        // After the window, sparse failures no longer count toward the share.
        let b = CircuitBreaker::new(BreakerConfig {
            same_in_a_row: 1000,
            window: 4,
        });
        for _ in 0..4 {
            b.record("y", Ok(())).unwrap();
        }
        for i in 0..50 {
            b.record("y", Err(&fail(&format!("e{}", i % 5)))).unwrap();
        }
    }

    #[test]
    fn env_errors_use_kind_thresholds() {
        let b = CircuitBreaker::default();
        let emfile = ToolError::new("metal", ToolFailure::Spawn(std::io::ErrorKind::Other))
            .with_env(Some(EnvError::new(EnvKind::TooManyFiles, "spawn")));
        b.record("metal", Err(&emfile)).unwrap();
        b.record("metal", Err(&emfile)).unwrap();
        assert_eq!(
            b.record("metal", Err(&emfile)).unwrap_err().kind,
            EnvKind::TooManyFiles
        );
        let nospace = ToolError::new("metal", ToolFailure::Exit(1)).with_env(EnvError::classify(
            &std::io::Error::from(std::io::ErrorKind::StorageFull),
            IoOp::Write,
            "x",
        ));
        assert_eq!(
            b.record("metal", Err(&nospace)).unwrap_err().kind,
            EnvKind::NoSpace
        );
    }

    /// Property: with distinct failure texts and fewer than half failing, the breaker never trips.
    #[test]
    fn distinct_sparse_failures_never_trip() {
        check(51, 50, |r| {
            let b = CircuitBreaker::default();
            for i in 0..1000u32 {
                let res = if r.chance(1, 3) {
                    b.record(
                        "t",
                        Err(&fail(&format!(
                            "error: case {}",
                            ["a", "b", "c", "d"][(i % 4) as usize]
                        ))),
                    )
                } else {
                    b.record("t", Ok(()))
                };
                assert!(res.is_ok());
            }
        });
    }
}
