//! The translator as one long-running host process (`dmsh_xlate_host`, translator-port S1): one .NET start serves
//! every batch of the build instead of one start per batch.
//!
//! Protocol (xlate-work/PROTOCOL.md): one JSON object per line. Request `{"id", "jobs": <job file>, "out": <dir>}`, reply
//! `{"id", "exit", "ok", "failed", "red": [...]}` after every output file is closed, replies come in completion order
//! and are routed by id. Outputs are the files `dmsh_xlate <jobs> <outdir>` writes, byte for byte.
//!
//! Failure handling: a host that dies or misses the deadline (`base + per_key x keys`) is killed (its process group) and
//! restarted, at most `max_restarts` times per build (circuit breaker). Keys that finished (`.spv` and `.res.json`) are
//! kept, the rest runs once more on the new host, then on the process bridge (`ProcessTranslator`, which bisects).
//! When the host cannot start, does not answer the ping, or the breaker opens, every batch goes to the process bridge.
//! Each fallback is counted with its reason ([`HostTranslator::fallbacks`]).

use crate::env::EnvPolicy;
use crate::hostwire::{Reply, parse_reply, ping, request};
use crate::process::ProcessRegistry;
use crate::tool::ToolId;
use crate::translator::{Translator, XlateFailure, XlateJob, XlateOutput, XlateResult};
use crate::xlate_process::{ProcessTranslator, red_lines, valid_key};
use nxb_error::{Reason, ToolError, ToolFailure};
use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

/// How to start and watch the host.
#[derive(Clone, Debug)]
pub struct HostConfig {
    /// The host program (`dmsh_xlate_host`).
    pub program: PathBuf,
    /// Its arguments (`--threads N`).
    pub args: Vec<String>,
    /// The translator environment (including `DOTNET_ROOT`).
    pub env: EnvPolicy,
    /// Folder for job files and output folders (private to the build).
    pub workdir: PathBuf,
    /// Watchdog base per request.
    pub base_timeout: Duration,
    /// Watchdog per key. The host runs a request on all its threads, so this is generous.
    pub per_key: Duration,
    /// Time the host gets to answer the first ping.
    pub start_timeout: Duration,
    /// Restarts allowed before the breaker opens and everything goes to the process bridge.
    pub max_restarts: u32,
}

impl HostConfig {
    /// Defaults: 60 s + 10 s per key, 10 s start, 6 restarts.
    pub fn new(program: PathBuf, args: Vec<String>, env: EnvPolicy, workdir: PathBuf) -> Self {
        HostConfig {
            program,
            args,
            env,
            workdir,
            base_timeout: Duration::from_secs(60),
            per_key: Duration::from_secs(10),
            start_timeout: Duration::from_secs(10),
            max_restarts: 6,
        }
    }
}

/// Reply routing table of one host. `None` after the host's stdout ended.
type Pending = Mutex<Option<HashMap<u64, Sender<Reply>>>>;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

struct Live {
    epoch: u64,
    child: Mutex<Child>,
    stdin: Mutex<Option<ChildStdin>>,
    pending: Arc<Pending>,
}

impl Live {
    fn kill(&self) {
        let mut c = lock(&self.child);
        let _ = nxb_sys::process::kill_group(c.id(), nxb_sys::process::Signal::Kill);
        let _ = c.wait();
    }
}

#[derive(Default)]
struct State {
    live: Option<Arc<Live>>,
    epoch: u64,
    restarts: u32,
    /// Why the host is off for the rest of the build.
    off: Option<String>,
}

enum Fail {
    Timeout,
    Dead,
    /// The host answered with `error` (a bridge bug) or garbage.
    Bad(String),
}

/// The translator bridge over one persistent host, with the process bridge as fallback.
pub struct HostTranslator {
    id: ToolId,
    cfg: HostConfig,
    registry: Arc<ProcessRegistry>,
    fallback: ProcessTranslator,
    state: Mutex<State>,
    next: AtomicU64,
    starts: AtomicU64,
    reasons: Mutex<BTreeMap<String, u64>>,
}

impl std::fmt::Debug for HostTranslator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HostTranslator({})", self.id.name)
    }
}

fn fail(detail: impl Into<String>) -> XlateFailure {
    XlateFailure {
        reason: Reason::TranslationFailed,
        detail: detail.into(),
    }
}

type Outcome = Result<XlateOutput, XlateFailure>;

impl HostTranslator {
    /// A bridge that starts the host on first use. `fallback` is the old per-batch bridge.
    pub fn new(
        id: ToolId,
        cfg: HostConfig,
        registry: Arc<ProcessRegistry>,
        fallback: ProcessTranslator,
    ) -> Self {
        HostTranslator {
            id,
            cfg,
            registry,
            fallback,
            state: Mutex::new(State::default()),
            next: AtomicU64::new(1),
            starts: AtomicU64::new(0),
            reasons: Mutex::new(BTreeMap::new()),
        }
    }

    /// Translator processes started so far: hosts plus fallback runs (each one .NET start).
    pub fn starts(&self) -> u64 {
        self.starts.load(Ordering::Relaxed) + self.fallback.runs()
    }

    /// Host restarts so far (crash or hang).
    pub fn restarts(&self) -> u32 {
        lock(&self.state).restarts
    }

    /// Keys sent to the process bridge, by reason.
    pub fn fallbacks(&self) -> Vec<(String, u64)> {
        lock(&self.reasons)
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    fn count(&self, reason: &str, keys: usize) {
        *lock(&self.reasons).entry(reason.to_string()).or_default() += keys as u64;
    }

    fn spawn(&self, epoch: u64) -> std::io::Result<Live> {
        let mut cmd = Command::new(&self.cfg.program);
        cmd.args(&self.cfg.args)
            .env_clear()
            .envs(self.cfg.env.vars())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .process_group(0);
        let mut child = cmd.spawn()?;
        if !self.registry.register(child.id()) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::other("cancelled"));
        }
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| std::io::Error::other("no stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| std::io::Error::other("no stdout"))?;
        let pending: Arc<Pending> = Arc::new(Mutex::new(Some(HashMap::new())));
        let p = pending.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                let Some(r) = parse_reply(&line) else {
                    continue;
                };
                let tx =
                    r.id.and_then(|id| lock(&p).as_mut().and_then(|m| m.remove(&id)));
                if let Some(tx) = tx {
                    let _ = tx.send(r);
                }
            }
            // end of output: waiters see a closed channel
            *lock(&p) = None;
        });
        Ok(Live {
            epoch,
            child: Mutex::new(child),
            stdin: Mutex::new(Some(stdin)),
            pending,
        })
    }

    /// The live host, started when there is none. `Err` holds the reason the host is off.
    fn acquire(&self) -> Result<Arc<Live>, String> {
        let mut st = lock(&self.state);
        if let Some(why) = &st.off {
            return Err(why.clone());
        }
        if let Some(l) = &st.live {
            return Ok(l.clone());
        }
        st.epoch += 1;
        self.starts.fetch_add(1, Ordering::Relaxed);
        let live = match self.spawn(st.epoch) {
            Ok(l) => Arc::new(l),
            Err(e) => {
                let why = format!("host start failed: {:?}", e.kind());
                st.off = Some(why.clone());
                return Err(why);
            }
        };
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let ok = self
            .send(&live, id, &ping(id), self.cfg.start_timeout)
            .is_ok_and(|r| r.error.is_none());
        if !ok {
            live.kill();
            self.registry.unregister(lock(&live.child).id());
            let why = "host did not answer ping".to_string();
            st.off = Some(why.clone());
            return Err(why);
        }
        st.live = Some(live.clone());
        Ok(live)
    }

    /// Kill `live` unless another caller already replaced it. True when this call did it.
    fn drop_host(&self, live: &Live) -> bool {
        let mut st = lock(&self.state);
        if st.live.as_ref().is_none_or(|l| l.epoch != live.epoch) {
            return false;
        }
        live.kill();
        self.registry.unregister(lock(&live.child).id());
        st.live = None;
        st.restarts += 1;
        if st.restarts > self.cfg.max_restarts {
            st.off = Some("host restart limit".into());
        }
        true
    }

    fn send(&self, live: &Live, id: u64, line: &str, wait: Duration) -> Result<Reply, Fail> {
        let (tx, rx) = channel();
        match lock(&live.pending).as_mut() {
            Some(m) => {
                m.insert(id, tx);
            }
            None => return Err(Fail::Dead),
        }
        let w = {
            match lock(&live.stdin).as_mut() {
                Some(si) => si.write_all(line.as_bytes()).and_then(|()| si.flush()),
                None => Err(std::io::Error::other("closed")),
            }
        };
        if w.is_err() {
            return Err(Fail::Dead);
        }
        match rx.recv_timeout(wait) {
            Ok(r) => Ok(r),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if let Some(m) = lock(&live.pending).as_mut() {
                    m.remove(&id);
                }
                Err(Fail::Timeout)
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(Fail::Dead),
        }
    }

    fn io_err(&self, e: &std::io::Error) -> ToolError {
        ToolError::new(&self.id.name, ToolFailure::Io(e.kind()))
    }

    /// One request over `jobs` on `live`. `Ok(Ok)`: the host replied, one outcome per job. `Ok(Err)`: the host failed,
    /// with the keys that finished before (`None`: not finished).
    #[allow(clippy::type_complexity, reason = "private two-level result")]
    fn run(
        &self,
        live: &Live,
        jobs: &[&XlateJob],
    ) -> Result<Result<Vec<Outcome>, (Fail, Vec<Option<XlateOutput>>)>, ToolError> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let jf = self.cfg.workdir.join(format!("job_{pid}_{id}.json"));
        let out = self.cfg.workdir.join(format!("out_{pid}_{id}"));
        let body: Vec<&str> = jobs.iter().map(|j| j.job_json.as_str()).collect();
        std::fs::create_dir_all(&out).map_err(|e| self.io_err(&e))?;
        std::fs::write(&jf, format!("[\n{}\n]\n", body.join(",\n")))
            .map_err(|e| self.io_err(&e))?;
        let keys = u32::try_from(jobs.len()).unwrap_or(u32::MAX);
        let wait = self.cfg.base_timeout + self.cfg.per_key * keys;
        let line = request(id, &jf.to_string_lossy(), &out.to_string_lossy());
        let reply = self.send(live, id, &line, wait);
        let _ = std::fs::remove_file(&jf);
        let res = match reply {
            Ok(r) if r.error.is_none() => {
                let red = red_lines(&r.red.join("\n"));
                Ok(jobs
                    .iter()
                    .map(|j| match red.get(&j.key) {
                        Some(m) => Err(fail(m.clone())),
                        None => read_out(&out, &j.key).ok_or_else(|| fail("no spv")),
                    })
                    .collect())
            }
            Ok(r) => Err((Fail::Bad(r.error.unwrap_or_default()), Vec::new())),
            Err(f) => Err((f, jobs.iter().map(|j| read_out(&out, &j.key)).collect())),
        };
        let _ = std::fs::remove_dir_all(&out);
        Ok(res)
    }
}

fn read_out(dir: &std::path::Path, key: &str) -> Option<XlateOutput> {
    Some(XlateOutput {
        spirv: std::fs::read(dir.join(format!("{key}.spv"))).ok()?,
        resources_json: std::fs::read(dir.join(format!("{key}.res.json"))).ok()?,
    })
}

impl Translator for HostTranslator {
    fn identity(&self) -> &ToolId {
        &self.id
    }

    fn translate(&self, batch: &[XlateJob]) -> Result<Vec<XlateResult>, ToolError> {
        let mut out: Vec<Option<Outcome>> = vec![None; batch.len()];
        let mut todo: Vec<usize> = Vec::new();
        for (i, j) in batch.iter().enumerate() {
            if valid_key(&j.key) {
                todo.push(i);
            } else {
                out[i] = Some(Err(fail("invalid key")));
            }
        }
        let mut reason = String::new();
        let (mut own_failures, mut rounds) = (0, 0);
        while !todo.is_empty() && own_failures < 2 && rounds < 6 {
            rounds += 1;
            let live = match self.acquire() {
                Ok(l) => l,
                Err(why) => {
                    reason = why;
                    break;
                }
            };
            let jobs: Vec<&XlateJob> = todo.iter().map(|&i| &batch[i]).collect();
            match self.run(&live, &jobs)? {
                Ok(res) => {
                    for (&i, r) in todo.iter().zip(res) {
                        out[i] = Some(r);
                    }
                    todo.clear();
                }
                Err((Fail::Bad(e), _)) => {
                    reason = format!("host protocol error: {e}");
                    break;
                }
                Err((f, done)) => {
                    let mine = self.drop_host(&live);
                    reason = match f {
                        Fail::Timeout => "host timed out",
                        _ => "host crashed",
                    }
                    .to_string();
                    let mut rest = Vec::new();
                    for (&i, d) in todo.iter().zip(done) {
                        match d {
                            Some(o) => out[i] = Some(Ok(o)),
                            None => rest.push(i),
                        }
                    }
                    todo = rest;
                    // a host killed by another caller is not this batch's fault
                    if mine {
                        own_failures += 1;
                    }
                }
            }
        }
        if !todo.is_empty() {
            self.count(&reason, todo.len());
            let jobs: Vec<XlateJob> = todo.iter().map(|&i| batch[i].clone()).collect();
            for (&i, r) in todo.iter().zip(self.fallback.translate(&jobs)?) {
                out[i] = Some(r.outcome);
            }
        }
        Ok(batch
            .iter()
            .zip(out)
            .map(|(j, o)| XlateResult {
                key: j.key.clone(),
                outcome: o.unwrap_or_else(|| Err(fail("not translated"))),
            })
            .collect())
    }
}

impl Drop for HostTranslator {
    fn drop(&mut self) {
        let live = lock(&self.state).live.take();
        let Some(live) = live else { return };
        // close stdin: the host finishes its requests and exits, kill it after a grace period
        *lock(&live.stdin) = None;
        let pid = lock(&live.child).id();
        for _ in 0..50 {
            if lock(&live.child).try_wait().ok().flatten().is_some() {
                self.registry.unregister(pid);
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        live.kill();
        self.registry.unregister(pid);
    }
}

#[cfg(all(test, unix))]
mod tests;
