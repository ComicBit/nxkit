//! Real-data oracle of the translator host: every job file of a folder goes through the process bridge and through the
//! host bridge, and `.spv` and `res.json` must be byte-identical. Skips without the environment variables:
//! `NXB_XLATE` (dmsh_xlate), `NXB_XLATE_HOST` (dmsh_xlate_host), `NXB_DOTNET_ROOT`, `NXB_XLATE_JOBS` (folder of
//! `job*.json`, each a JSON array whose `code` paths exist). `NXB_TEST_TMP` is the scratch folder.

#![allow(clippy::unwrap_used, reason = "test helpers")]

use nxb_tool::{
    EnvPolicy, HostConfig, HostTranslator, ProcessRegistry, ProcessTranslator, ToolId, Translator,
    XlateConfig, XlateJob,
};
use std::path::PathBuf;
use std::sync::Arc;

/// Split a JSON array into its top-level objects (text kept as is).
fn objects(text: &str) -> Vec<&str> {
    let (mut out, mut depth, mut start, mut in_str, mut esc) = (Vec::new(), 0, 0, false, false);
    for (i, c) in text.char_indices() {
        if in_str {
            match (esc, c) {
                (true, _) => esc = false,
                (false, '\\') => esc = true,
                (false, '"') => in_str = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '{' => {
                if depth == 0 {
                    start = i;
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    out.push(&text[start..=i]);
                }
            }
            _ => {}
        }
    }
    out
}

fn key_of(obj: &str) -> String {
    let r = obj.split("\"key\": \"").nth(1).unwrap_or_default();
    r.split('"').next().unwrap_or_default().to_string()
}

struct Real {
    proc_: ProcessTranslator,
    host: HostTranslator,
    files: Vec<PathBuf>,
    tmp: PathBuf,
}

fn real(tweak: impl FnOnce(&mut HostConfig)) -> Option<Real> {
    let var = |n: &str| std::env::var_os(n).map(PathBuf::from);
    let (xlate, host, dotnet, jobs) = (
        var("NXB_XLATE")?,
        var("NXB_XLATE_HOST")?,
        var("NXB_DOTNET_ROOT")?,
        var("NXB_XLATE_JOBS")?,
    );
    let tmp = var("NXB_TEST_TMP")
        .unwrap_or_else(std::env::temp_dir)
        .join(format!(
            "nxb-hostreal-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
    let (w1, w2) = (tmp.join("proc"), tmp.join("host"));
    std::fs::create_dir_all(&w1).unwrap();
    std::fs::create_dir_all(&w2).unwrap();
    let env = EnvPolicy::translator(&tmp, &tmp, Some(dotnet.as_os_str()), None, None)
        .and_then(|e| e.set("PATH", format!("{}:/usr/bin:/bin", dotnet.display())))
        .unwrap();
    let id = ToolId {
        name: "dmsh_xlate".into(),
        path: xlate,
        version: String::new(),
    };
    let reg = Arc::new(ProcessRegistry::new());
    let proc_ = ProcessTranslator::new(id.clone(), reg.clone(), XlateConfig::new(w1, env.clone()));
    let fb = ProcessTranslator::new(
        id.clone(),
        reg.clone(),
        XlateConfig::new(w2.clone(), env.clone()),
    );
    let threads = std::env::var("NXB_XLATE_THREADS").unwrap_or_else(|_| "8".into());
    let mut cfg = HostConfig::new(host, vec!["--threads".into(), threads], env, w2);
    tweak(&mut cfg);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&jobs)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    Some(Real {
        proc_,
        host: HostTranslator::new(id, cfg, reg, fb),
        files,
        tmp,
    })
}

fn batch_of(f: &PathBuf) -> Vec<XlateJob> {
    let text = std::fs::read_to_string(f).unwrap();
    objects(&text)
        .into_iter()
        .map(|o| XlateJob {
            key: key_of(o),
            job_json: o.to_string(),
        })
        .collect()
}

#[test]
fn host_equals_process_bridge() {
    let Some(r) = real(|_| {}) else { return };
    let (mut same, mut total, mut t_proc, mut t_host) = (0, 0, 0.0, 0.0);
    for f in &r.files {
        let batch = batch_of(f);
        let t = std::time::Instant::now();
        let a = r.proc_.translate(&batch).unwrap();
        t_proc += t.elapsed().as_secs_f64();
        let t = std::time::Instant::now();
        let b = r.host.translate(&batch).unwrap();
        t_host += t.elapsed().as_secs_f64();
        for (x, y) in a.iter().zip(&b) {
            total += 1;
            if x == y {
                same += 1;
            } else {
                eprintln!("DIFF {}", x.key);
            }
        }
    }
    eprintln!(
        "host oracle: {same}/{total} identical, process bridge {t_proc:.1} s ({} starts), host {t_host:.1} s ({} starts), fallbacks {:?}",
        r.proc_.runs(),
        r.host.starts(),
        r.host.fallbacks()
    );
    let _ = std::fs::remove_dir_all(&r.tmp);
    assert_eq!(same, total);
    assert!(r.host.fallbacks().is_empty());
}

/// A watchdog far too short: the real host is killed on every request (a hang), the breaker opens, and the process
/// bridge still delivers identical results.
#[test]
fn hung_host_falls_back_identically() {
    let Some(r) = real(|c| {
        c.base_timeout = std::time::Duration::from_millis(30);
        c.per_key = std::time::Duration::ZERO;
        c.max_restarts = 2;
    }) else {
        return;
    };
    let batch = batch_of(&r.files[0]);
    let a = r.proc_.translate(&batch).unwrap();
    let b = r.host.translate(&batch).unwrap();
    let b2 = r.host.translate(&batch).unwrap();
    assert!(a == b && a == b2, "results differ after the fallback");
    let fb = r.host.fallbacks();
    eprintln!(
        "hung host: restarts {}, fallbacks {fb:?}",
        r.host.restarts()
    );
    assert!(fb.iter().any(|(k, _)| k == "host timed out"), "{fb:?}");
    assert!(r.host.restarts() >= 1);
    let _ = std::fs::remove_dir_all(&r.tmp);
}
