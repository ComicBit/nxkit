//! Stages that overlap while the console events stay those of a run one after another. A stage either runs in its
//! turn on the calling thread (it writes `step`, `progress` and `say` events into its own stage) or starts early on a
//! thread of its own as soon as its dependencies succeeded (silent: it only returns its summary). The `stage` and
//! `stage_end` events are written in the graph's order whatever finishes first, so the front ends see the same stream
//! as without overlap (only `t` and `elapsed` differ). Hidden stages write no events: they are parts of a later stage
//! made ahead (the app's companion app, icon and settings).
//!
//! Errors keep their place too: a stage that failed early is reported in its turn, the stages before it still run
//! and report as they would have. A stage whose dependency failed does not run (it reports that dependency's error).

use std::sync::{Condvar, Mutex, PoisonError};

use crate::events::Progress;

/// A stage's outcome: its summary or its error.
pub type Outcome = Result<String, String>;

/// How a stage runs.
pub enum Work<'a> {
    /// In its turn, on the calling thread.
    Turn(Box<dyn FnOnce() -> Outcome + 'a>),
    /// On its own thread as soon as its dependencies succeeded.
    Early(Box<dyn FnOnce() -> Outcome + Send + 'a>),
    /// Done already: the summary.
    Reused(String),
}

/// One stage: its name (`None`: hidden, no events), the stages it waits for (earlier ones only) and its work.
pub struct Stage<'a> {
    /// The name in the events.
    pub name: Option<String>,
    /// Indices of earlier stages.
    pub deps: Vec<usize>,
    /// What it does.
    pub work: Work<'a>,
}

/// The outcomes so far.`abort` when the run stopped (stages waiting for a dependency that will never run give up).
struct Board {
    slots: Mutex<(Vec<Option<Outcome>>, bool)>,
    cv: Condvar,
}

impl Board {
    fn put(&self, i: usize, o: Outcome) {
        self.slots.lock().unwrap_or_else(PoisonError::into_inner).0[i] = Some(o);
        self.cv.notify_all();
    }

    fn abort(&self) {
        self.slots.lock().unwrap_or_else(PoisonError::into_inner).1 = true;
        self.cv.notify_all();
    }

    /// Waits for every stage in `ids`.`Err` with the first failed one's message (or "stopped" after an abort).
    fn wait(&self, ids: &[usize]) -> Result<(), String> {
        let mut g = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            if let Some(e) = ids.iter().find_map(|&d| match &g.0[d] {
                Some(Err(e)) => Some(e.clone()),
                _ => None,
            }) {
                return Err(e);
            }
            if ids.iter().all(|&d| g.0[d].is_some()) {
                return Ok(());
            }
            if g.1 {
                return Err("stopped".into());
            }
            g = self.cv.wait(g).unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn get(&self, i: usize) -> Outcome {
        self.wait(&[i])?;
        match &self.slots.lock().unwrap_or_else(PoisonError::into_inner).0[i] {
            Some(o) => o.clone(),
            None => Err("stopped".into()),
        }
    }
}

/// Runs the stages, early ones beside the others, and writes `stage` / `stage_end` in order. Stops at the first error
/// in that order (the caller reports it, as after a run one after another).
pub fn run(p: &Progress, stages: Vec<Stage<'_>>) -> Result<(), String> {
    let n = stages.len();
    let board = Board {
        slots: Mutex::new((vec![None; n], false)),
        cv: Condvar::new(),
    };
    let board = &board;
    std::thread::scope(|s| {
        let mut turn: Vec<(Option<String>, Vec<usize>, Work<'_>)> = Vec::with_capacity(n);
        for (i, st) in stages.into_iter().enumerate() {
            let work = match st.work {
                Work::Early(f) => {
                    let deps = st.deps.clone();
                    s.spawn(move || {
                        let o = board.wait(&deps).and_then(|()| f());
                        board.put(i, o);
                    });
                    Work::Reused(String::new())
                }
                Work::Reused(r) => {
                    board.put(i, Ok(r.clone()));
                    Work::Reused(r)
                }
                w => w,
            };
            turn.push((st.name, st.deps, work));
        }
        let r = (|| {
            for (i, (name, deps, work)) in turn.into_iter().enumerate() {
                if let Some(name) = &name {
                    p.stage(name, "");
                }
                let o = match work {
                    Work::Turn(f) => {
                        let o = board.wait(&deps).and_then(|()| f());
                        board.put(i, o.clone());
                        o
                    }
                    _ if name.is_none() => continue,
                    _ => board.get(i),
                };
                // a hidden stage's failure is reported by the stage that needs it, in that stage's turn
                if name.is_some() {
                    p.end(&o?, true);
                }
            }
            Ok(())
        })();
        board.abort();
        r
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::Mode;
    use std::io::Write;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    #[derive(Clone, Default)]
    struct Buf(Arc<Mutex<Vec<u8>>>);

    impl Write for Buf {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// The events without the times (`"t": ...`, `"elapsed": ...`).
    fn events(b: &Buf) -> Vec<String> {
        let text = String::from_utf8_lossy(&b.0.lock().unwrap_or_else(PoisonError::into_inner))
            .into_owned();
        text.lines()
            .map(|l| {
                let l = l.split(", \"elapsed\"").next().unwrap_or(l);
                l.split(", \"t\"").next().unwrap_or(l).to_string()
            })
            .collect()
    }

    /// The build's shape: extract (turn), split (reused), shaders (turn, says), host (early, no deps), hidden app parts
    /// (early, after extract and host), app (turn, after all). `early` picks overlap or a run one after another.
    /// `fail` makes one stage fail.
    fn work<'a>(early: bool, f: Box<dyn FnOnce() -> Outcome + Send + 'a>) -> Work<'a> {
        if early { Work::Early(f) } else { Work::Turn(f) }
    }

    fn shape(
        early: bool,
        fail: Option<usize>,
        host_started: &AtomicBool,
    ) -> (Vec<String>, Outcome) {
        let buf = Buf::default();
        let p = Progress::new(Mode::Json, Box::new(buf.clone()));
        let p = &p;
        let res = |i: usize, s: &str| -> Outcome {
            if fail == Some(i) {
                Err(format!("stage {i} failed"))
            } else {
                Ok(s.to_string())
            }
        };
        let stages = vec![
            Stage {
                name: Some("extract".into()),
                deps: vec![],
                work: Work::Turn(Box::new(move || {
                    std::thread::sleep(Duration::from_millis(60));
                    p.say("extract and shader split in one pass");
                    res(0, "2 images")
                })),
            },
            Stage {
                name: Some("shader split".into()),
                deps: vec![0],
                work: Work::Reused("in the extract's pass".into()),
            },
            Stage {
                name: Some("Mac shaders".into()),
                deps: vec![1],
                work: Work::Turn(Box::new(move || {
                    p.say("shaders done");
                    res(2, "3 of 3 shaders")
                })),
            },
            Stage {
                name: Some("Mac host".into()),
                deps: vec![],
                work: work(
                    early,
                    Box::new(move || {
                        host_started.store(true, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(5));
                        res(3, "tools/dmhost")
                    }),
                ),
            },
            Stage {
                name: None,
                deps: vec![0, 3],
                work: work(early, Box::new(move || res(4, ""))),
            },
            Stage {
                name: Some("Mac app".into()),
                deps: vec![2, 4],
                work: Work::Turn(Box::new(move || res(5, "4 files staged and signed"))),
            },
        ];
        let r = run(p, stages).map(|()| String::new());
        (events(&buf), r)
    }

    #[test]
    fn overlap_keeps_the_event_stream() {
        let started = AtomicBool::new(false);
        let (seq, r1) = shape(false, None, &started);
        let (ovl, r2) = shape(true, None, &started);
        assert!(r1.is_ok() && r2.is_ok());
        assert_eq!(seq.len(), 12);
        assert_eq!(seq, ovl);
    }

    #[test]
    fn early_stage_runs_during_an_earlier_turn() {
        let started = AtomicBool::new(false);
        let buf = Buf::default();
        let p = Progress::new(Mode::Off, Box::new(buf));
        let seen = AtomicBool::new(false);
        let stages = vec![
            Stage {
                name: Some("first".into()),
                deps: vec![],
                work: Work::Turn(Box::new(|| {
                    for _ in 0..200 {
                        if started.load(Ordering::SeqCst) {
                            seen.store(true, Ordering::SeqCst);
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Ok(String::new())
                })),
            },
            Stage {
                name: Some("second".into()),
                deps: vec![],
                work: Work::Early(Box::new(|| {
                    started.store(true, Ordering::SeqCst);
                    Ok(String::new())
                })),
            },
        ];
        assert!(run(&p, stages).is_ok());
        assert!(seen.load(Ordering::SeqCst));
    }

    #[test]
    fn errors_keep_their_place() {
        let started = AtomicBool::new(false);
        // the early host fails: extract and shaders still run and report, the failure ends the host's stage
        for fail in [0, 2, 3, 4, 5] {
            let (seq, r1) = shape(false, Some(fail), &started);
            let (ovl, r2) = shape(true, Some(fail), &started);
            assert_eq!(seq, ovl, "fail {fail}");
            assert_eq!(r1, r2, "fail {fail}");
            assert_eq!(r2, Err(format!("stage {fail} failed")));
        }
    }

    #[test]
    fn a_failed_turn_releases_waiting_early_stages() {
        let p = Progress::new(Mode::Off, Box::new(Buf::default()));
        let ran = AtomicBool::new(false);
        let stages = vec![
            Stage {
                name: Some("a".into()),
                deps: vec![],
                work: Work::Turn(Box::new(|| Err("a failed".into()))),
            },
            Stage {
                name: Some("b".into()),
                deps: vec![],
                work: Work::Turn(Box::new(|| Ok(String::new()))),
            },
            Stage {
                name: Some("c".into()),
                deps: vec![1],
                work: Work::Early(Box::new(|| {
                    ran.store(true, Ordering::SeqCst);
                    Ok(String::new())
                })),
            },
        ];
        assert_eq!(run(&p, stages), Err("a failed".into()));
        assert!(!ran.load(Ordering::SeqCst));
    }
}
