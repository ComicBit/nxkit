//! Event sinks and a rate-limited progress counter.

use crate::event::Event;
use std::io::{self, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// Receives events. Implementations must be cheap and must never panic: a broken sink must not stop a build.
pub trait Sink: Send + Sync {
    /// Handle one event.
    fn emit(&self, event: &Event);
}

/// Drops every event.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullSink;

impl Sink for NullSink {
    fn emit(&self, _event: &Event) {}
}

/// Keeps every event in memory (tests, `explain`).
#[derive(Debug, Default)]
pub struct MemorySink {
    events: Mutex<Vec<Event>>,
}

impl MemorySink {
    /// An empty sink.
    pub fn new() -> Self {
        Self::default()
    }

    /// A copy of the events so far.
    pub fn events(&self) -> Vec<Event> {
        self.events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Sink for MemorySink {
    fn emit(&self, event: &Event) {
        self.events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(event.clone());
    }
}

/// Writes one JSON line per event, each line written and flushed whole under a lock (lines never interleave).
///
/// A write error is kept (the first one) and later events are dropped, the build goes on and the caller checks
/// [`NdjsonSink::take_error`] at the end.
#[derive(Debug)]
pub struct NdjsonSink<W: Write + Send> {
    inner: Mutex<(W, Option<io::Error>)>,
}

impl<W: Write + Send> NdjsonSink<W> {
    /// A sink over a writer (a file, a pipe, `fd:3`).
    pub fn new(w: W) -> Self {
        NdjsonSink {
            inner: Mutex::new((w, None)),
        }
    }

    /// The first write error, if any.
    pub fn take_error(&self) -> Option<io::Error> {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .1
            .take()
    }

    /// The writer back.
    pub fn into_inner(self) -> W {
        self.inner
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner)
            .0
    }
}

impl<W: Write + Send> Sink for NdjsonSink<W> {
    fn emit(&self, event: &Event) {
        let mut line = event.to_ndjson();
        line.push('\n');
        let mut g = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if g.1.is_some() {
            return;
        }
        let res = g.0.write_all(line.as_bytes()).and_then(|_| g.0.flush());
        if let Err(e) = res {
            g.1 = Some(e);
        }
    }
}

/// Counts finished units from many threads and emits `progress` events at most once per interval, with `done`
/// never going backwards, and always once at [`ProgressCounter::finish`].
pub struct ProgressCounter {
    sink: Arc<dyn Sink>,
    step: String,
    unit: String,
    total: AtomicU64,
    done: AtomicU64,
    interval: Duration,
    last: Mutex<(Instant, Option<u64>)>,
}

impl std::fmt::Debug for ProgressCounter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgressCounter")
            .field("step", &self.step)
            .field("done", &self.done.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl ProgressCounter {
    /// A counter for `step` (for example `Mac shaders`) in `unit` (`shaders`).
    pub fn new(
        sink: Arc<dyn Sink>,
        step: &str,
        unit: &str,
        total: u64,
        interval: Duration,
    ) -> Self {
        ProgressCounter {
            sink,
            step: step.to_string(),
            unit: unit.to_string(),
            total: AtomicU64::new(total),
            done: AtomicU64::new(0),
            interval,
            last: Mutex::new((Instant::now(), None)),
        }
    }

    /// Change the total (streaming builds learn it late).
    pub fn set_total(&self, total: u64) {
        self.total.store(total, Ordering::Relaxed);
    }

    /// Units done so far.
    pub fn done(&self) -> u64 {
        self.done.load(Ordering::Relaxed)
    }

    /// Add finished units, may emit an event.
    pub fn add(&self, n: u64) {
        self.done.fetch_add(n, Ordering::Relaxed);
        // Never block a worker on the progress lock: if another thread is emitting, skip.
        if let Ok(mut g) = self.last.try_lock()
            && g.0.elapsed() >= self.interval
        {
            self.emit_locked(&mut g);
        }
    }

    /// Emit the final state (always, unless nothing changed since the last event).
    pub fn finish(&self) {
        let mut g = self.last.lock().unwrap_or_else(PoisonError::into_inner);
        self.emit_locked(&mut g);
    }

    fn emit_locked(&self, g: &mut (Instant, Option<u64>)) {
        let done = self.done.load(Ordering::Relaxed);
        if g.1.is_some_and(|prev| prev >= done) {
            return;
        }
        g.0 = Instant::now();
        g.1 = Some(done);
        self.sink.emit(&Event::Progress {
            step: self.step.clone(),
            done,
            total: self.total.load(Ordering::Relaxed),
            unit: self.unit.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ndjson_lines() {
        let sink = NdjsonSink::new(Vec::new());
        sink.emit(&Event::Phase { name: "a".into() });
        sink.emit(&Event::Phase { name: "b".into() });
        assert!(sink.take_error().is_none());
        let out = String::from_utf8(sink.into_inner()).unwrap();
        assert_eq!(
            out,
            "{\"ev\":\"phase\",\"name\":\"a\"}\n{\"ev\":\"phase\",\"name\":\"b\"}\n"
        );
    }

    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn broken_writer_is_reported_not_fatal() {
        let sink = NdjsonSink::new(Broken);
        sink.emit(&Event::Phase { name: "a".into() });
        sink.emit(&Event::Phase { name: "b".into() });
        assert_eq!(
            sink.take_error().map(|e| e.kind()),
            Some(io::ErrorKind::BrokenPipe)
        );
    }

    /// Property: from many threads, emitted `done` values never decrease and the last one equals the real total.
    #[test]
    fn progress_monotonic_and_final() {
        for threads in [1usize, 4, 8] {
            let mem = Arc::new(MemorySink::new());
            let pc = Arc::new(ProgressCounter::new(
                mem.clone(),
                "Mac shaders",
                "shaders",
                0,
                Duration::from_micros(50),
            ));
            let per = 2000u64;
            std::thread::scope(|s| {
                for _ in 0..threads {
                    let pc = pc.clone();
                    s.spawn(move || {
                        for _ in 0..per {
                            pc.add(1);
                        }
                    });
                }
            });
            pc.set_total(per * threads as u64);
            pc.finish();
            pc.finish();
            let dones: Vec<u64> = mem
                .events()
                .iter()
                .filter_map(|e| match e {
                    Event::Progress { done, .. } => Some(*done),
                    _ => None,
                })
                .collect();
            assert!(dones.windows(2).all(|w| w[0] < w[1]), "{dones:?}");
            assert_eq!(dones.last().copied(), Some(per * threads as u64));
        }
    }
}
