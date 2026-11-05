//! Host bridge tests against a fake host in Python speaking the protocol (skipped without `/usr/bin/python3`).

#![allow(clippy::unwrap_used, reason = "test helpers")]

use super::*;
use crate::xlate_process::XlateConfig;
use std::sync::atomic::AtomicU32;

static N: AtomicU32 = AtomicU32::new(0);

/// Fake host: per job in the file, `"mode":"red"` is a red line, `"die"` exits the process, `"hang"` sleeps. When
/// `FAKE_DIE_ONCE` names a file that does not exist yet, a `"mode":"die1"` job creates it and dies (a transient crash).
const FAKE: &str = r#"
import sys, json, time, os
for line in sys.stdin:
    r = json.loads(line)
    if r.get("op") == "ping":
        print(json.dumps({"id": r["id"], "ok": True, "pid": os.getpid(), "threads": 1}), flush=True); continue
    jobs = json.load(open(r["jobs"])); os.makedirs(r["out"], exist_ok=True); red = []
    for j in jobs:
        m = j.get("mode", "")
        if m == "die": os._exit(9)
        if m == "die1" and not os.path.exists(os.environ["FAKE_FLAG"]):
            open(os.environ["FAKE_FLAG"], "w").close(); os._exit(9)
        if m == "hang": time.sleep(60)
        if m == "red": red.append("[RED] %s: bad shader" % j["key"]); continue
        open(r["out"] + "/%s.spv" % j["key"], "w").write("host-" + j["key"])
        open(r["out"] + "/%s.res.json" % j["key"], "w").write("{}")
    print(json.dumps({"id": r["id"], "exit": 1 if red else 0, "ok": len(jobs) - len(red), "failed": len(red), "red": red, "ms": 1.0}), flush=True)
"#;

/// Fake per-batch translator (the fallback): writes `proc-<key>`.
const PROC: &str = r#"#!/bin/sh
jobs="$1"; out="$2"
grep '"key"' "$jobs" | while IFS= read -r line; do
  key=$(echo "$line" | sed 's/.*"key":"\([A-Za-z0-9_]*\)".*/\1/')
  case "$line" in
    *'"mode":"die"'*) echo "[RED] $key: bad shader" >&2 ;;
    *) printf 'proc-%s' "$key" > "$out/$key.spv"; printf '{}' > "$out/$key.res.json" ;;
  esac
done
exit 0
"#;

fn setup(
    args: Vec<String>,
    tweak: impl FnOnce(&mut HostConfig),
) -> Option<(HostTranslator, PathBuf)> {
    let py = PathBuf::from("/usr/bin/python3");
    if !py.is_file() {
        return None;
    }
    let root = std::env::var_os("NXB_TEST_TMP")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!(
            "nxb-host-test-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
    let _ = std::fs::remove_dir_all(&root);
    let work = root.join("work");
    std::fs::create_dir_all(&work).unwrap();
    let script = root.join("fake_xlate");
    std::fs::write(&script, PROC).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let env = EnvPolicy::empty()
        .set("PATH", "/usr/bin:/bin")
        .unwrap()
        .set("FAKE_FLAG", root.join("flag").display().to_string())
        .unwrap();
    let id = ToolId {
        name: "dmsh_xlate".into(),
        path: script,
        version: "fake".into(),
    };
    let reg = Arc::new(ProcessRegistry::new());
    let fb = ProcessTranslator::new(
        id.clone(),
        reg.clone(),
        XlateConfig::new(work.clone(), env.clone()),
    );
    let mut cfg = HostConfig::new(py, args, env, work);
    tweak(&mut cfg);
    Some((HostTranslator::new(id, cfg, reg, fb), root))
}

fn fake_args() -> Vec<String> {
    vec!["-c".into(), FAKE.into()]
}

fn job(k: &str, mode: &str) -> XlateJob {
    XlateJob {
        key: k.into(),
        job_json: format!("{{\"key\":\"{k}\",\"mode\":\"{mode}\"}}"),
    }
}

fn spv(r: &XlateResult) -> String {
    String::from_utf8(r.outcome.as_ref().unwrap().spirv.clone()).unwrap()
}

#[test]
fn many_batches_one_start_and_red() {
    let Some((b, root)) = setup(fake_args(), |_| {}) else {
        return;
    };
    let jobs: Vec<XlateJob> = (0..40)
        .map(|i| job(&format!("k{i:03}"), if i == 7 { "red" } else { "" }))
        .collect();
    let r = b.translate(&jobs).unwrap();
    assert_eq!(spv(&r[0]), "host-k000");
    assert_eq!(r[7].outcome.as_ref().err().unwrap().detail, "bad shader");
    let r2 = std::thread::scope(|s| {
        let h: Vec<_> = (0..4)
            .map(|_| s.spawn(|| b.translate(&jobs[..5]).unwrap()))
            .collect();
        h.into_iter().map(|h| h.join().unwrap()).collect::<Vec<_>>()
    });
    assert!(r2.iter().all(|x| spv(&x[4]) == "host-k004"));
    assert_eq!(b.starts(), 1);
    assert!(b.fallbacks().is_empty());
    drop(b);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn transient_crash_reruns_on_new_host() {
    let Some((b, root)) = setup(fake_args(), |_| {}) else {
        return;
    };
    let r = b
        .translate(&[job("a1", ""), job("a2", "die1"), job("a3", "")])
        .unwrap();
    assert_eq!(
        (spv(&r[0]), spv(&r[1]), spv(&r[2])),
        ("host-a1".into(), "host-a2".into(), "host-a3".into())
    );
    assert_eq!(b.starts(), 2);
    assert!(b.fallbacks().is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn poison_key_goes_to_process_bridge() {
    let Some((b, root)) = setup(fake_args(), |_| {}) else {
        return;
    };
    let r = b
        .translate(&[job("a1", ""), job("a2", "die"), job("a3", "")])
        .unwrap();
    assert_eq!(spv(&r[0]), "host-a1", "finished before the crash");
    assert_eq!(r[1].outcome.as_ref().err().unwrap().detail, "bad shader");
    assert_eq!(spv(&r[2]), "proc-a3");
    assert_eq!(b.fallbacks(), [("host crashed".to_string(), 2)]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn hang_hits_watchdog_and_breaker_opens() {
    let Some((b, root)) = setup(fake_args(), |c| {
        c.base_timeout = Duration::from_millis(500);
        c.per_key = Duration::ZERO;
        c.max_restarts = 1;
    }) else {
        return;
    };
    let r = b.translate(&[job("h1", "hang")]).unwrap();
    assert_eq!(spv(&r[0]), "proc-h1");
    assert!(
        b.fallbacks()
            .iter()
            .any(|(k, _)| k == "host timed out" || k == "host restart limit")
    );
    // breaker open: the next batch skips the host
    let before = b.starts();
    let r = b.translate(&[job("b1", "")]).unwrap();
    assert_eq!(spv(&r[0]), "proc-b1");
    assert!(b.fallbacks().iter().any(|(k, _)| k == "host restart limit"));
    assert_eq!(b.starts(), before + 1, "only the process run");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn missing_host_falls_back() {
    let Some((b, root)) = setup(Vec::new(), |c| {
        c.program = PathBuf::from("/nonexistent/host")
    }) else {
        return;
    };
    let r = b.translate(&[job("a1", "")]).unwrap();
    assert_eq!(spv(&r[0]), "proc-a1");
    assert_eq!(
        b.fallbacks(),
        [("host start failed: NotFound".to_string(), 1)]
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn silent_host_fails_ping() {
    let Some((b, root)) = setup(
        vec!["-c".into(), "import time; time.sleep(30)".into()],
        |c| {
            c.start_timeout = Duration::from_millis(300);
        },
    ) else {
        return;
    };
    let r = b.translate(&[job("a1", "")]).unwrap();
    assert_eq!(spv(&r[0]), "proc-a1");
    assert_eq!(b.fallbacks(), [("host did not answer ping".to_string(), 1)]);
    let _ = std::fs::remove_dir_all(root);
}
