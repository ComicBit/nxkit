//! Unit and property tests of the executor, joins and single-flight.

use super::*;
use nxb_testkit::{Rng, check};
use std::sync::atomic::{AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A random task tree: node i has children `kids[i]`, some children are spawned locally, some submitted.
struct Tree {
    kids: Vec<Vec<usize>>,
    submit: Vec<bool>,
}

fn random_tree(r: &mut Rng, max_nodes: usize) -> Tree {
    let mut kids = vec![Vec::new()];
    let mut submit = vec![false];
    let mut i = 0;
    while i < kids.len() && kids.len() < max_nodes {
        let n = r.below(5) as usize;
        for _ in 0..n {
            if kids.len() >= max_nodes {
                break;
            }
            let id = kids.len();
            kids.push(Vec::new());
            submit.push(r.chance(1, 3));
            kids[i].push(id);
        }
        i += 1;
    }
    Tree { kids, submit }
}

fn node_task(tree: Arc<Tree>, hits: Arc<Vec<AtomicU32>>, id: usize) -> Task {
    Task::new(move |ctx| {
        hits[id].fetch_add(1, Ordering::SeqCst);
        for &k in &tree.kids[id] {
            let t = node_task(tree.clone(), hits.clone(), k);
            if tree.submit[k] {
                ctx.submit(t.prio(k as i64 % 7));
            } else {
                ctx.spawn(t);
            }
        }
    })
}

#[test]
fn every_task_runs_exactly_once() {
    check(81, 30, |r| {
        let max = 1 + r.below(400) as usize;
        let tree = Arc::new(random_tree(r, max));
        let n = tree.kids.len();
        let hits: Arc<Vec<AtomicU32>> = Arc::new((0..n).map(|_| AtomicU32::new(0)).collect());
        let workers = 1 + r.below(8) as u32;
        let rep = Executor::new(Budget::fixed(workers)).run(vec![node_task(tree, hits.clone(), 0)]);
        assert_eq!(rep.tasks_run, n as u64);
        assert!(hits.iter().all(|h| h.load(Ordering::SeqCst) == 1));
        assert!(rep.panics.is_empty());
        assert_eq!(rep.cancelled, 0);
    });
}

#[test]
fn token_budget_is_never_exceeded() {
    check(82, 5, |r| {
        let tokens = 1 + r.below(4) as u32;
        let in_use = Arc::new(AtomicI64::new(0));
        let peak = Arc::new(AtomicI64::new(0));
        let roots: Vec<Task> = (0..40)
            .map(|_| {
                let cost = 1 + r.below(6) as u32;
                let (in_use, peak) = (in_use.clone(), peak.clone());
                let held = i64::from(cost.min(tokens));
                Task::new(move |_| {
                    let now = in_use.fetch_add(held, Ordering::SeqCst) + held;
                    peak.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_micros(300));
                    in_use.fetch_sub(held, Ordering::SeqCst);
                })
                .cost(cost)
            })
            .collect();
        let rep = Executor::new(Budget::fixed(tokens)).run(roots);
        assert_eq!(rep.tasks_run, 40);
        assert!(peak.load(Ordering::SeqCst) <= i64::from(tokens));
        assert!(rep.peak_tokens <= u64::from(tokens));
    });
}

/// Property: with random memory sizes (some above the whole budget), expected memory held at once never exceeds the
/// budget and every task runs.
#[test]
fn memory_budget_is_never_exceeded() {
    check(86, 5, |r| {
        let budget = 1000 + r.below(4000);
        let in_use = Arc::new(AtomicI64::new(0));
        let peak = Arc::new(AtomicI64::new(0));
        let roots: Vec<Task> = (0..60)
            .map(|_| {
                let mem = r.below(budget * 3 / 2);
                let held = mem.min(budget) as i64;
                let (in_use, peak) = (in_use.clone(), peak.clone());
                Task::new(move |_| {
                    let now = in_use.fetch_add(held, Ordering::SeqCst) + held;
                    peak.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_micros(200));
                    in_use.fetch_sub(held, Ordering::SeqCst);
                })
                .mem(mem)
            })
            .collect();
        let rep = Executor::new(Budget::fixed(8).with_memory(budget)).run(roots);
        assert_eq!(rep.tasks_run, 60);
        assert!(peak.load(Ordering::SeqCst) <= budget as i64);
        assert!(rep.peak_mem <= budget);
        assert!(rep.peak_mem > 0);
    });
}

#[test]
fn no_memory_budget_ignores_task_memory() {
    let roots: Vec<Task> = (0..10).map(|_| Task::new(|_| {}).mem(u64::MAX)).collect();
    let rep = Executor::new(Budget::fixed(4)).run(roots);
    assert_eq!(rep.tasks_run, 10);
    assert_eq!(rep.peak_mem, 0);
}

#[test]
fn priority_order_on_one_worker() {
    let order = Arc::new(Mutex::new(Vec::new()));
    let roots: Vec<Task> = [(3, 'a'), (1, 'b'), (5, 'c'), (3, 'd'), (4, 'e'), (1, 'f')]
        .into_iter()
        .map(|(p, name)| {
            let order = order.clone();
            Task::new(move |_| order.lock().unwrap().push(name)).prio(p)
        })
        .collect();
    Executor::new(Budget::fixed(1)).run(roots);
    // Highest first, equal priorities in submission order.
    assert_eq!(*order.lock().unwrap(), ['c', 'e', 'a', 'd', 'b', 'f']);
}

#[test]
fn idle_workers_steal_local_work() {
    let workers_seen = Arc::new(Mutex::new(std::collections::BTreeSet::new()));
    let ws = workers_seen.clone();
    let root = Task::new(move |ctx| {
        for _ in 0..64 {
            let ws = ws.clone();
            ctx.spawn(Task::new(move |ctx| {
                ws.lock().unwrap().insert(ctx.worker());
                std::thread::sleep(Duration::from_millis(2));
            }));
        }
    });
    let rep = Executor::new(Budget::fixed(4)).run(vec![root]);
    assert_eq!(rep.tasks_run, 65);
    assert!(rep.steals > 0);
    assert!(workers_seen.lock().unwrap().len() >= 2);
    assert!(rep.utilisation() > 0.3, "{}", rep.utilisation());
}

#[test]
fn panics_are_isolated() {
    let ok = Arc::new(AtomicU32::new(0));
    let mut roots = vec![Task::new(|_| panic!("boom in task"))];
    for _ in 0..10 {
        let ok = ok.clone();
        roots.push(Task::new(move |_| {
            ok.fetch_add(1, Ordering::SeqCst);
        }));
    }
    let rep = Executor::new(Budget::fixed(3)).run(roots);
    assert_eq!(ok.load(Ordering::SeqCst), 10);
    assert_eq!(rep.tasks_run, 11);
    assert_eq!(rep.panics, ["boom in task"]);
}

#[test]
fn cancel_drops_queued_tasks() {
    let ex = Executor::new(Budget::fixed(2));
    let cancel = ex.cancel_handle();
    let ran = Arc::new(AtomicU32::new(0));
    let r2 = ran.clone();
    let root = Task::new(move |ctx| {
        cancel.cancel();
        assert!(ctx.is_cancelled());
        for _ in 0..100 {
            let r = r2.clone();
            ctx.spawn(Task::new(move |_| {
                r.fetch_add(1, Ordering::SeqCst);
            }));
        }
    });
    let rep = ex.run(vec![root]);
    assert_eq!(rep.tasks_run + rep.cancelled, 101);
    assert_eq!(ran.load(Ordering::SeqCst), (rep.tasks_run - 1) as u32);
    assert!(rep.cancelled >= 90, "{}", rep.cancelled);
}

#[test]
fn empty_run_returns() {
    let rep = Executor::new(Budget::fixed(4)).run(Vec::new());
    assert_eq!(rep.tasks_run, 0);
}

/// `n == 0` hands the task back, calls after the count reached zero do nothing.
#[test]
fn join_edge_cases() {
    assert!(Join::new(0, Task::new(|_| {})).is_err());
    let after = Arc::new(AtomicU32::new(0));
    let a = after.clone();
    let root = Task::new(move |ctx| {
        let a2 = a.clone();
        let Ok(join) = Join::new(
            2,
            Task::new(move |_| {
                a2.fetch_add(1, Ordering::SeqCst);
            }),
        ) else {
            return;
        };
        for _ in 0..5 {
            join.done(ctx);
        }
        assert_eq!(join.remaining(), 0);
    });
    Executor::new(Budget::fixed(2)).run(vec![root]);
    assert_eq!(after.load(Ordering::SeqCst), 1);
}

#[test]
fn join_exact_fan_in() {
    check(84, 20, |r| {
        let n = 1 + r.below(64) as usize;
        let done = Arc::new(AtomicU32::new(0));
        let after = Arc::new(AtomicU32::new(0));
        let (d, a) = (done.clone(), after.clone());
        let root = Task::new(move |ctx| {
            let (d2, a2) = (d.clone(), a.clone());
            let Ok(join) = Join::new(
                n,
                Task::new(move |_| {
                    assert_eq!(d2.load(Ordering::SeqCst), n as u32);
                    a2.fetch_add(1, Ordering::SeqCst);
                }),
            ) else {
                return;
            };
            for _ in 0..n {
                let (d, join) = (d.clone(), join.clone());
                ctx.submit(Task::new(move |ctx| {
                    d.fetch_add(1, Ordering::SeqCst);
                    join.done(ctx);
                }));
            }
        });
        let rep = Executor::new(Budget::fixed(4)).run(vec![root]);
        assert_eq!(after.load(Ordering::SeqCst), 1);
        assert_eq!(rep.tasks_run, n as u64 + 2);
        assert!(rep.panics.is_empty());
    });
}

/// Property: inside the executor, many tasks claim a few keys, each key's work runs once and every claimant gets
/// the leader's value through its continuation.
#[test]
fn single_flight_in_executor() {
    check(85, 10, |r| {
        let keys = 1 + r.below(6);
        let claims = 10 + r.below(100);
        let sf: Arc<SingleFlight<u64, u64>> = Arc::new(SingleFlight::new());
        let work = Arc::new((0..keys).map(|_| AtomicU32::new(0)).collect::<Vec<_>>());
        let got = Arc::new(AtomicU64::new(0));
        let picks: Vec<u64> = (0..claims).map(|_| r.below(keys)).collect();
        let roots: Vec<Task> = picks
            .iter()
            .map(|&k| {
                let (sf, work, got) = (sf.clone(), work.clone(), got.clone());
                Task::new(move |_| {
                    let g = got.clone();
                    match sf.claim(k, move |v| {
                        assert_eq!(v, Some(&(k * 10)));
                        g.fetch_add(1, Ordering::SeqCst);
                    }) {
                        Claim::Leader(l) => {
                            work[k as usize].fetch_add(1, Ordering::SeqCst);
                            std::thread::sleep(Duration::from_micros(200));
                            l.complete(k * 10);
                            got.fetch_add(1, Ordering::SeqCst);
                        }
                        Claim::Joined => {}
                    }
                })
            })
            .collect();
        Executor::new(Budget::fixed(6)).run(roots);
        assert_eq!(got.load(Ordering::SeqCst), claims);
        for (k, w) in work.iter().enumerate() {
            let used = picks.contains(&(k as u64));
            assert_eq!(w.load(Ordering::SeqCst), u32::from(used));
        }
    });
}

#[test]
fn get_or_run_blocks_followers() {
    let sf: SingleFlight<&'static str, u32> = SingleFlight::new();
    let runs = AtomicU32::new(0);
    std::thread::scope(|s| {
        for _ in 0..8 {
            s.spawn(|| {
                let v = sf.get_or_run("k", || {
                    runs.fetch_add(1, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(20));
                    7
                });
                assert_eq!(*v, 7);
            });
        }
    });
    assert_eq!(runs.load(Ordering::SeqCst), 1);
    assert_eq!(sf.get(&"k").as_deref(), Some(&7));
    sf.forget(&"k");
    assert_eq!(sf.get(&"k"), None);
}

#[test]
fn abandoned_leader_releases_key() {
    let sf: SingleFlight<u8, u8> = SingleFlight::new();
    let saw_none = Arc::new(AtomicU32::new(0));
    let Claim::Leader(leader) = sf.claim(1, |_| {}) else {
        panic!("first claim must lead");
    };
    let s = saw_none.clone();
    assert!(matches!(
        sf.claim(1, move |v| {
            if v.is_none() {
                s.fetch_add(1, Ordering::SeqCst);
            }
        }),
        Claim::Joined
    ));
    drop(leader);
    assert_eq!(saw_none.load(Ordering::SeqCst), 1);
    assert_eq!(*sf.get_or_run(1, || 9), 9);
}
