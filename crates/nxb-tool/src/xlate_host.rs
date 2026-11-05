//! The translator as a persistent host process (design-v2 P4): one `.NET` start serves many jobs instead of one per
//! batch. Up to `hosts` processes live for the run, a batch takes an idle one (or starts one), streams its jobs in and
//! reads the answers back while a writer thread keeps the host's stdin fed (no pipe deadlock).
//!
//! Wire protocol (proposal "nxb-xlate-host 1", until the translator side settles it, framing in ASCII headers, payloads
//! raw):
//! - the host is started as `<program> <args…>` with the translator environment, and may print one greeting line
//!   starting with `hello ` first.
//! - request: `job <key> <n>\n` then `n` bytes of the job object's JSON (one job, the same object the batch file holds).
//! - answer, in request order: `ok <key> <spv bytes> <res.json bytes>\n` then both payloads, or `red <key> <message>\n`.
//! - end: stdin closed, the host exits.
//!
//! Recovery as the batch bridge: a host that dies or misses the per-job deadline is killed (its process group), the job
//! it was on fails with `crashed`/`timed out`, the jobs after it go to a fresh host once.

use crate::env::EnvPolicy;
use crate::tool::ToolId;
use crate::translator::{Translator, XlateFailure, XlateJob, XlateOutput, XlateResult};
use nxb_error::{Reason, ToolError, ToolFailure};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

/// How to start a host.
#[derive(Clone, Debug)]
pub struct HostConfig {
    /// The host program (`dmsh_xlate`).
    pub program: PathBuf,
    /// Its arguments (for example `--host`).
    pub args: Vec<String>,
    /// The translator environment.
    pub env: EnvPolicy,
    /// Deadline per job (the host's first job also pays the .NET start).
    pub per_job: Duration,
}

enum Frame {
    Ok(String, XlateOutput),
    Red(String, String),
    Bad(String),
}

struct Host {
    child: Child,
    stdin: Option<ChildStdin>,
    rx: Receiver<Frame>,
}

impl Host {
    fn start(cfg: &HostConfig) -> std::io::Result<Host> {
        let mut cmd = Command::new(&cfg.program);
        cmd.args(&cfg.args)
            .env_clear()
            .envs(cfg.env.vars())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .process_group(0);
        let mut child = cmd.spawn()?;
        let stdin = child.stdin.take();
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| std::io::Error::other("no stdout"))?;
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let mut r = BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match r.read_line(&mut line) {
                    Ok(0) | Err(_) => return,
                    Ok(_) => {}
                }
                let l = line.trim_end_matches('\n');
                if l.starts_with("hello ") {
                    continue;
                }
                let mut it = l.splitn(4, ' ');
                let frame = match (it.next(), it.next(), it.next(), it.next()) {
                    (Some("ok"), Some(k), Some(a), Some(b)) => {
                        let (Ok(a), Ok(b)) = (a.parse::<usize>(), b.parse::<usize>()) else {
                            let _ = tx.send(Frame::Bad(l.to_string()));
                            return;
                        };
                        let (mut spirv, mut res) = (vec![0; a], vec![0; b]);
                        if r.read_exact(&mut spirv).is_err() || r.read_exact(&mut res).is_err() {
                            return;
                        }
                        Frame::Ok(
                            k.to_string(),
                            XlateOutput {
                                spirv,
                                resources_json: res,
                            },
                        )
                    }
                    (Some("red"), Some(k), a, b) => {
                        let msg = [a, b].into_iter().flatten().collect::<Vec<_>>().join(" ");
                        Frame::Red(k.to_string(), msg)
                    }
                    _ => Frame::Bad(l.to_string()),
                };
                if tx.send(frame).is_err() {
                    return;
                }
            }
        });
        Ok(Host { child, stdin, rx })
    }

    fn kill(mut self) {
        let _ = nxb_sys::process::kill_group(self.child.id(), nxb_sys::process::Signal::Kill);
        let _ = self.child.wait();
    }
}

/// The translator bridge over persistent hosts.
pub struct HostTranslator {
    id: ToolId,
    cfg: HostConfig,
    idle: Mutex<Vec<Host>>,
    starts: AtomicU64,
}

impl std::fmt::Debug for HostTranslator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HostTranslator({})", self.id.name)
    }
}

fn failed(key: &str, detail: impl Into<String>) -> XlateResult {
    XlateResult {
        key: key.to_string(),
        outcome: Err(XlateFailure {
            reason: Reason::TranslationFailed,
            detail: detail.into(),
        }),
    }
}

impl HostTranslator {
    /// A bridge that starts hosts on demand.
    pub fn new(id: ToolId, cfg: HostConfig) -> Self {
        HostTranslator {
            id,
            cfg,
            idle: Mutex::new(Vec::new()),
            starts: AtomicU64::new(0),
        }
    }

    /// Host processes started so far (each one .NET start).
    pub fn starts(&self) -> u64 {
        self.starts.load(Ordering::Relaxed)
    }

    fn host(&self) -> Result<Host, ToolError> {
        if let Some(h) = self
            .idle
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop()
        {
            return Ok(h);
        }
        self.starts.fetch_add(1, Ordering::Relaxed);
        Host::start(&self.cfg).map_err(|e| ToolError::new(&self.id.name, ToolFailure::Io(e.kind())))
    }

    /// Run `jobs` on one host: answers in order.`Err(i)` when the host failed at job `i` (killed already).
    fn run_on(
        &self,
        h: &mut Host,
        jobs: &[&XlateJob],
        out: &mut Vec<XlateResult>,
    ) -> Result<(), (usize, String)> {
        let Some(mut stdin) = h.stdin.take() else {
            return Err((0, "no stdin".into()));
        };
        let reqs: Vec<(String, Vec<u8>)> = jobs
            .iter()
            .map(|j| (j.key.clone(), j.job_json.clone().into_bytes()))
            .collect();
        let writer = std::thread::spawn(move || {
            for (k, body) in &reqs {
                if writeln!(stdin, "job {k} {}", body.len()).is_err()
                    || stdin.write_all(body).is_err()
                {
                    return None;
                }
            }
            let _ = stdin.flush();
            Some(stdin)
        });
        let mut res = Ok(());
        for (i, j) in jobs.iter().enumerate() {
            match h.rx.recv_timeout(self.cfg.per_job) {
                Ok(Frame::Ok(k, o)) if k == j.key => out.push(XlateResult {
                    key: k,
                    outcome: Ok(o),
                }),
                Ok(Frame::Red(k, m)) if k == j.key => out.push(failed(&k, m)),
                Ok(Frame::Ok(k, _) | Frame::Red(k, _)) => {
                    res = Err((i, format!("host answered {k} for {}", j.key)));
                    break;
                }
                Ok(Frame::Bad(l)) => {
                    res = Err((i, format!("host protocol: {l}")));
                    break;
                }
                Err(RecvTimeoutError::Timeout) => {
                    res = Err((i, "timed out".into()));
                    break;
                }
                Err(RecvTimeoutError::Disconnected) => {
                    res = Err((i, "crashed".into()));
                    break;
                }
            }
        }
        if res.is_err() {
            // unblock the writer: the caller kills the host
            let _ = nxb_sys::process::kill_group(h.child.id(), nxb_sys::process::Signal::Kill);
        }
        h.stdin = writer.join().ok().flatten();
        res
    }
}

impl Translator for HostTranslator {
    fn identity(&self) -> &ToolId {
        &self.id
    }

    fn translate(&self, batch: &[XlateJob]) -> Result<Vec<XlateResult>, ToolError> {
        let mut out = Vec::with_capacity(batch.len());
        let mut rest: Vec<&XlateJob> = batch.iter().collect();
        let mut retried = false;
        while !rest.is_empty() {
            let mut h = self.host()?;
            match self.run_on(&mut h, &rest, &mut out) {
                Ok(()) => {
                    self.idle
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .push(h);
                    break;
                }
                Err((i, why)) => {
                    h.kill();
                    out.push(failed(&rest[i].key, why));
                    rest = rest.split_off(i + 1);
                    if retried {
                        out.extend(
                            rest.iter()
                                .map(|j| failed(&j.key, "not run: the host failed twice")),
                        );
                        break;
                    }
                    retried = true;
                }
            }
        }
        Ok(out)
    }
}

impl Drop for HostTranslator {
    fn drop(&mut self) {
        for mut h in std::mem::take(&mut *self.idle.lock().unwrap_or_else(PoisonError::into_inner))
        {
            h.stdin = None;
            let _ = h.child.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A host in Python speaking the protocol: echoes the job back as SPIR-V, `res` = key.`bad` keys are red, a `die`
    /// key exits, a `hang` key sleeps.
    const FAKE: &str = r#"
import sys, time
i, o = sys.stdin.buffer, sys.stdout.buffer
o.write(b"hello fake 1\n"); o.flush()
while True:
    h = i.readline()
    if not h: break
    _, k, n = h.decode().split()
    body = i.read(int(n))
    if k.startswith("die"): sys.exit(3)
    if k.startswith("hang"): time.sleep(30)
    if k.startswith("bad"): o.write(("red %s cannot translate\n" % k).encode())
    else:
        o.write(b"ok %s %d %d\n" % (k.encode(), len(body), len(k))); o.write(body); o.write(k.encode())
    o.flush()
"#;

    fn bridge() -> Option<HostTranslator> {
        let py = PathBuf::from("/usr/bin/python3");
        if !py.is_file() {
            return None;
        }
        let env = EnvPolicy::empty().set("PATH", "/usr/bin:/bin").ok()?;
        Some(HostTranslator::new(
            ToolId {
                name: "fake-host".into(),
                path: py.clone(),
                version: String::new(),
            },
            HostConfig {
                program: py,
                args: vec!["-c".into(), FAKE.into()],
                env,
                per_job: Duration::from_secs(5),
            },
        ))
    }

    fn job(k: &str) -> XlateJob {
        XlateJob {
            key: k.into(),
            job_json: format!("{{\"key\": \"{k}\"}}"),
        }
    }

    #[test]
    fn many_jobs_one_host() {
        let Some(b) = bridge() else { return };
        let jobs: Vec<XlateJob> = (0..300).map(|i| job(&format!("k{i:04}"))).collect();
        let r = b.translate(&jobs).expect("bridge");
        assert_eq!(r.len(), 300);
        let r2 = b.translate(&jobs[..10]).expect("bridge");
        assert_eq!(b.starts(), 1, "the second batch reuses the host");
        let first = r[0].outcome.as_ref().expect("ok");
        assert_eq!(first.spirv, jobs[0].job_json.as_bytes());
        assert_eq!(first.resources_json, b"k0000");
        assert_eq!(r2.len(), 10);
    }

    #[test]
    fn red_crash_and_retry() {
        let Some(b) = bridge() else { return };
        let jobs = vec![job("a1"), job("bad1"), job("die1"), job("a2"), job("a3")];
        let r = b.translate(&jobs).expect("bridge");
        let keys: Vec<&str> = r.iter().map(|x| x.key.as_str()).collect();
        assert_eq!(keys, ["a1", "bad1", "die1", "a2", "a3"]);
        assert!(r[0].outcome.is_ok() && r[3].outcome.is_ok() && r[4].outcome.is_ok());
        assert_eq!(
            r[1].outcome.as_ref().err().map(|f| f.detail.as_str()),
            Some("cannot translate")
        );
        assert_eq!(
            r[2].outcome.as_ref().err().map(|f| f.detail.as_str()),
            Some("crashed")
        );
        assert_eq!(b.starts(), 2);
    }
}
