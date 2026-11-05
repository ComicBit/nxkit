//! DAG edges: a task that runs when its predecessors are done.

use crate::executor::{Ctx, Task};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

/// A countdown: after `n` calls of [`Join::done`] the continuation is spawned on the worker that made the last call.
///
/// ```
/// use nxb_sched::{Budget, Executor, Join, Task};
/// use std::sync::{Arc, atomic::{AtomicU32, Ordering}};
/// let hits = Arc::new(AtomicU32::new(0));
/// let h = hits.clone();
/// let root = Task::new(move |ctx| {
///     let h2 = h.clone();
///     let join = Join::new(3, Task::new(move |_| assert_eq!(h2.load(Ordering::SeqCst), 3))).unwrap();
///     for _ in 0..3 {
///         let (h, join) = (h.clone(), join.clone());
///         ctx.spawn(Task::new(move |ctx| { h.fetch_add(1, Ordering::SeqCst); join.done(ctx); }));
///     }
/// });
/// let report = Executor::new(Budget::fixed(4)).run(vec![root]);
/// assert_eq!(report.tasks_run, 5);
/// ```
pub struct Join {
    remaining: AtomicUsize,
    then: Mutex<Option<Task>>,
}

impl std::fmt::Debug for Join {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Join")
            .field("remaining", &self.remaining.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl Join {
    /// A join over `n` predecessors. With `n == 0` there is nothing to wait for: the task comes back as `Err` and the
    /// caller spawns it directly.
    pub fn new(n: usize, then: Task) -> Result<Arc<Join>, Task> {
        if n == 0 {
            return Err(then);
        }
        Ok(Arc::new(Join {
            remaining: AtomicUsize::new(n),
            then: Mutex::new(Some(then)),
        }))
    }

    /// One predecessor is done. Extra calls after the count reached zero do nothing.
    pub fn done(&self, ctx: &Ctx<'_>) {
        let prev = self
            .remaining
            .try_update(Ordering::AcqRel, Ordering::Acquire, |r| r.checked_sub(1));
        if prev == Ok(1) {
            let task = self
                .then
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take();
            if let Some(t) = task {
                ctx.spawn(t);
            }
        }
    }

    /// Predecessors still outstanding.
    pub fn remaining(&self) -> usize {
        self.remaining.load(Ordering::Acquire)
    }
}
