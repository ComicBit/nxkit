//! The pipeline archive while the shader step runs (design-pipeline-rest 10.4, overlap): the pairs are cut into groups
//! by the content of their identity (the two shader keys), so a group is known before any library exists and an
//! insertion changes only its own group. A group's batches start as soon as every shader it names is finished (the
//! shader step reports each one), on a few worker threads beside the shader step. Within a group the bases are planned
//! (`plan::plan`), hashed by their libraries' bytes and cut by content ([`cut`]) as in [`crate::build`], so the batch
//! keys in the store stay content keys. At the end the pieces are merged in key order.
//!
//! Width: while the shader step runs, a few workers ([`StreamInputs::workers`]) leave it the cores; when it ends
//! ([`Stream::finish`]) the stream widens to [`StreamInputs::workers_after`] (one run per slot, within memory), so the
//! groups still waiting are not compiled three at a time (the reference title, wave 9: a 232 s tail after the shader step).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Condvar, Mutex, PoisonError};
use std::time::Instant;

use nxb_model::{ContentKey, sha256};
use nxb_store::Store;
use nxb_tool::ProcessRegistry;

use crate::cut::cut;
use crate::plan::{self, PlannedBase};
use crate::script::Script;
use crate::{BatchReport, Config, Report};

/// The pairs of one group and how many of its shaders are still running.
struct Group {
    pairs: Vec<(String, String)>,
    waiting: usize,
}

/// What a streamed archive needs besides [`Config`] (whose `script` is not used).
#[derive(Clone, Debug)]
pub struct StreamInputs {
    /// The library folder the shader step writes (`b-<chain>`).
    pub lib_dir: PathBuf,
    /// The chain version.
    pub chain: String,
    /// The pairs, in `pso_offline.py` order (profile pairs, then the split's `pso_pairs.tsv`).
    pub pairs: Vec<(String, String)>,
    /// `depth_mode zero`.
    pub zero: bool,
    /// Pairs per group (about, a group's identity hash cuts it, at most 4x).
    pub group_target: usize,
    /// Worker threads (concurrent `metal-tt`) while the shader step runs.
    pub workers: usize,
    /// Worker threads after [`Stream::finish`] (the shader step ended; at least `workers`).
    pub workers_after: usize,
}

/// Opens once: the shader step ended, the extra workers may start.
#[derive(Default)]
struct Gate {
    open: Mutex<bool>,
    cv: Condvar,
}

impl Gate {
    fn open(&self) {
        *self.open.lock().unwrap_or_else(PoisonError::into_inner) = true;
        self.cv.notify_all();
    }

    fn wait(&self) {
        let mut g = self.open.lock().unwrap_or_else(PoisonError::into_inner);
        while !*g {
            g = self.cv.wait(g).unwrap_or_else(PoisonError::into_inner);
        }
    }
}

/// Runs `work` on every item of `rx` with `narrow` threads, `wide` once `gate` opened (the extra threads wait for
/// it), until the sender is gone and the queue is empty.
fn drain<T: Send>(
    rx: Receiver<T>,
    narrow: usize,
    wide: usize,
    gate: &Gate,
    work: &(dyn Fn(T) + Sync),
) {
    let rx = Mutex::new(rx);
    let narrow = narrow.max(1);
    std::thread::scope(|s| {
        for i in 0..wide.max(narrow) {
            let rx = &rx;
            s.spawn(move || {
                if i >= narrow {
                    gate.wait();
                }
                loop {
                    let next = rx.lock().unwrap_or_else(PoisonError::into_inner).recv();
                    let Ok(item) = next else { break };
                    work(item);
                }
            });
        }
    });
}

/// A streamed archive: feed it finished shaders, then [`Stream::finish`].
pub struct Stream {
    groups: Mutex<Vec<Group>>,
    by_key: HashMap<String, Vec<usize>>,
    tx: Mutex<Option<Sender<usize>>>,
    rx: Mutex<Option<Receiver<usize>>>,
    wide: Gate,
}

fn pair_hash(v: &str, f: &str) -> [u8; 32] {
    sha256(format!("{v}\0{f}").as_bytes()).0
}

impl Stream {
    /// Groups the pairs, groups that need no shader are ready at once.
    pub fn new(inp: &StreamInputs) -> Stream {
        let bases: Vec<crate::cut::Base> = inp
            .pairs
            .iter()
            .enumerate()
            .map(|(index, (v, f))| crate::cut::Base {
                hash: pair_hash(v, f),
                // pairs of one vertex shader stay in one group (its functions compile once per batch)
                group: sha256(v.as_bytes()).0,
                index,
            })
            .collect();
        let mut groups = Vec::new();
        let mut by_key: HashMap<String, Vec<usize>> = HashMap::new();
        for g in cut(bases, inp.group_target) {
            let gi = groups.len();
            let mut pairs: Vec<(String, String)> =
                g.iter().map(|b| inp.pairs[b.index].clone()).collect();
            // keep the pairs in their file order inside the group (plan order)
            pairs.sort_by_key(|p| inp.pairs.iter().position(|q| q == p));
            let keys: HashSet<&String> = pairs.iter().flat_map(|(v, f)| [v, f]).collect();
            for k in &keys {
                by_key.entry((*k).clone()).or_default().push(gi);
            }
            groups.push(Group {
                waiting: keys.len(),
                pairs,
            });
        }
        let (tx, rx) = channel();
        Stream {
            groups: Mutex::new(groups),
            by_key,
            tx: Mutex::new(Some(tx)),
            rx: Mutex::new(Some(rx)),
            wide: Gate::default(),
        }
    }

    /// Number of groups.
    pub fn groups(&self) -> usize {
        self.groups
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// The shader keys in group order (the shader step builds them first, so groups complete one after another).
    pub fn key_order(&self) -> Vec<String> {
        let g = self.groups.lock().unwrap_or_else(PoisonError::into_inner);
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for grp in g.iter() {
            for (v, f) in &grp.pairs {
                for k in [f, v] {
                    if seen.insert(k.clone()) {
                        out.push(k.clone());
                    }
                }
            }
        }
        out
    }

    /// The shader step's key list is known: shaders it will not build count as done (a pair can name a shader of
    /// another version or one the split did not package), so no group waits for them.
    pub fn building(&self, keys: &HashSet<String>) {
        let mut groups = self.groups.lock().unwrap_or_else(PoisonError::into_inner);
        let tx = self.tx.lock().unwrap_or_else(PoisonError::into_inner);
        for (g, grp) in groups.iter_mut().enumerate() {
            let need: HashSet<&String> = grp
                .pairs
                .iter()
                .flat_map(|(v, f)| [v, f])
                .filter(|k| keys.contains(*k))
                .collect();
            let absent = grp
                .pairs
                .iter()
                .flat_map(|(v, f)| [v, f])
                .collect::<HashSet<_>>()
                .len()
                - need.len();
            grp.waiting = grp.waiting.saturating_sub(absent);
            if grp.waiting == 0
                && let Some(tx) = tx.as_ref()
            {
                let _ = tx.send(g);
            }
        }
    }

    /// A shader is finished (whatever its status: a failed one simply has no library, which the plan skips).
    pub fn done(&self, key: &str) {
        let Some(gs) = self.by_key.get(key) else {
            return;
        };
        let mut groups = self.groups.lock().unwrap_or_else(PoisonError::into_inner);
        for &g in gs {
            let w = &mut groups[g].waiting;
            *w = w.saturating_sub(1);
            if *w == 0
                && let Some(tx) = self
                    .tx
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .as_ref()
            {
                let _ = tx.send(g);
            }
        }
    }

    /// The shader step ended: release every group still waiting (shaders not built at all) and widen to
    /// [`StreamInputs::workers_after`] workers.
    pub fn finish(&self) {
        self.wide.open();
        let mut groups = self.groups.lock().unwrap_or_else(PoisonError::into_inner);
        let tx = self
            .tx
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(tx) = tx {
            for (g, grp) in groups.iter_mut().enumerate() {
                if grp.waiting > 0 {
                    grp.waiting = 0;
                    let _ = tx.send(g);
                }
            }
        }
    }

    /// Compile groups as they become ready until [`Stream::finish`] was called and all are done; then merge into
    /// `c.out`. Run it on its own thread beside the shader step. Returns the report and the planned base count.
    pub fn run(&self, c: &Config, inp: &StreamInputs) -> Result<(Report, usize), String> {
        let t0 = Instant::now();
        let rx = self
            .rx
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            .ok_or("stream already ran")?;
        fs::create_dir_all(&c.work).map_err(|e| format!("{}: {e}", c.work.display()))?;
        let store = Store::open(&c.store).map_err(|e| e.to_string())?;
        let reg = ProcessRegistry::new();
        let seen: Mutex<HashSet<PlannedBase>> = Mutex::new(HashSet::new());
        let pieces: Mutex<Vec<(ContentKey, PathBuf, BatchReport)>> = Mutex::new(Vec::new());
        let errors: Mutex<Vec<String>> = Mutex::new(Vec::new());
        let groups = &self.groups;
        let work = |g: usize| {
            let pairs = groups.lock().unwrap_or_else(PoisonError::into_inner)[g]
                .pairs
                .clone();
            match group(c, inp, &pairs, &seen, &store, &reg) {
                Ok(mut p) => pieces
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .append(&mut p),
                Err(e) => errors
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(e),
            }
        };
        drain(rx, inp.workers, inp.workers_after, &self.wide, &work);
        if let Some(e) = errors
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner)
            .first()
        {
            return Err(e.clone());
        }
        let mut pieces = pieces.into_inner().unwrap_or_else(PoisonError::into_inner);
        pieces.sort_by_key(|a| a.0.to_hex());
        let t1 = Instant::now();
        let mut report = Report::default();
        let keys: Vec<ContentKey> = pieces.iter().map(|p| p.0).collect();
        let paths: Vec<PathBuf> = pieces.iter().map(|p| p.1.clone()).collect();
        report.batches = pieces.into_iter().map(|p| p.2).collect();
        crate::merge(c, &store, &reg, &keys, &paths, &mut report)?;
        report.seconds = (0.0, (t1 - t0).as_secs_f64(), t1.elapsed().as_secs_f64());
        let n = seen
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner)
            .len();
        Ok((report, n))
    }
}

/// Plan, hash, cut and compile one group: its batches' (key, piece, report).
fn group(
    c: &Config,
    inp: &StreamInputs,
    pairs: &[(String, String)],
    seen: &Mutex<HashSet<PlannedBase>>,
    store: &Store,
    reg: &ProcessRegistry,
) -> Result<Vec<(ContentKey, PathBuf, BatchReport)>, String> {
    let p = plan::plan(&inp.lib_dir, &inp.chain, pairs, inp.zero);
    let bases: Vec<PlannedBase> = {
        let mut s = seen.lock().unwrap_or_else(PoisonError::into_inner);
        p.bases
            .into_iter()
            .filter(|b| s.insert(b.clone()))
            .collect()
    };
    if bases.is_empty() {
        return Ok(Vec::new());
    }
    let script = Script::parse(&plan::script_text(&inp.lib_dir, &bases))?;
    let mut digests = HashMap::new();
    for (label, path) in script.library_paths()? {
        let b = fs::read(Path::new(&path)).map_err(|e| format!("{path}: {e}"))?;
        digests.insert(label, sha256(&b));
    }
    let mut out = Vec::new();
    for batch in cut(script.bases(&digests)?, c.target) {
        let key = crate::batch_key(c, &batch);
        let (piece, rep) = crate::compile(c, &script, &batch, &key, store, reg)?;
        out.push((key, piece, rep));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(n: usize) -> StreamInputs {
        StreamInputs {
            lib_dir: PathBuf::from("/nonexistent"),
            chain: "c".into(),
            pairs: (0..n)
                .map(|i| (format!("v{i}"), format!("f{}", i % 7)))
                .collect(),
            zero: false,
            group_target: 8,
            workers: 1,
            workers_after: 1,
        }
    }

    #[test]
    fn groups_open_when_their_shaders_are_done() {
        let inp = inputs(100);
        let s = Stream::new(&inp);
        assert!(s.groups() > 1);
        let order = s.key_order();
        assert_eq!(order.len(), 107);
        let rx = s.rx.lock().unwrap_or_else(PoisonError::into_inner).take();
        let Some(rx) = rx else { panic!("receiver") };
        // finish the shaders of the first group in order: exactly that group opens first
        let first = s.groups.lock().unwrap_or_else(PoisonError::into_inner)[0]
            .pairs
            .clone();
        let keys: HashSet<String> = first
            .iter()
            .flat_map(|(v, f)| [v.clone(), f.clone()])
            .collect();
        for k in &keys {
            s.done(k);
        }
        assert_eq!(rx.try_recv().ok(), Some(0));
        s.finish();
        let rest: Vec<usize> = rx.try_iter().collect();
        assert!(!rest.contains(&0));
        assert!(rest.len() + 1 >= s.groups() - 1);
    }

    #[test]
    fn shaders_not_built_do_not_block() {
        let inp = inputs(40);
        let s = Stream::new(&inp);
        let rx = s.rx.lock().unwrap_or_else(PoisonError::into_inner).take();
        let Some(rx) = rx else { panic!("receiver") };
        // only the vertex shaders are built: every group waits for them alone
        let built: HashSet<String> = (0..40).map(|i| format!("v{i}")).collect();
        s.building(&built);
        assert!(rx.try_recv().is_err());
        for k in &built {
            s.done(k);
        }
        assert_eq!(rx.try_iter().count(), s.groups());
    }

    #[test]
    fn widens_when_the_shader_step_ends() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Duration;
        let (tx, rx) = channel();
        let gate = Gate::default();
        let (now, peak_before, peak_after, opened) = (
            AtomicUsize::new(0),
            AtomicUsize::new(0),
            AtomicUsize::new(0),
            std::sync::atomic::AtomicBool::new(false),
        );
        let work = |_: usize| {
            let n = now.fetch_add(1, Ordering::SeqCst) + 1;
            let peak = if opened.load(Ordering::SeqCst) {
                &peak_after
            } else {
                &peak_before
            };
            peak.fetch_max(n, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(20));
            now.fetch_sub(1, Ordering::SeqCst);
        };
        std::thread::scope(|s| {
            s.spawn(|| drain(rx, 2, 6, &gate, &work));
            for i in 0..8 {
                let _ = tx.send(i);
            }
            std::thread::sleep(Duration::from_millis(100));
            opened.store(true, Ordering::SeqCst);
            for i in 0..24 {
                let _ = tx.send(i);
            }
            gate.open();
            drop(tx);
        });
        assert!(peak_before.load(Ordering::SeqCst) <= 2);
        assert!(peak_after.load(Ordering::SeqCst) > 2);
        assert!(peak_after.load(Ordering::SeqCst) <= 6);
    }

    #[test]
    fn grouping_is_stable_under_insertion() {
        let a = Stream::new(&inputs(200));
        let mut more = inputs(200);
        more.pairs.push(("vX".into(), "fX".into()));
        let b = Stream::new(&more);
        let set = |s: &Stream| -> HashSet<Vec<(String, String)>> {
            s.groups
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .iter()
                .map(|g| g.pairs.clone())
                .collect()
        };
        let (sa, sb) = (set(&a), set(&b));
        // all groups but the one that took the new pair (and a neighbour split by the cap) are unchanged
        assert!(sa.intersection(&sb).count() + 2 >= sa.len());
    }
}
