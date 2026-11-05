//! Recovery tests against a fake translator (a shell script with the same command line and output contract).

#![allow(
    clippy::unwrap_used,
    reason = "test helpers outside #[test] functions (the module is cfg(all(test, unix)))"
)]

use super::*;
use std::sync::atomic::AtomicU32;
use std::time::Instant;

static N: AtomicU32 = AtomicU32::new(0);

/// Fake `dmsh_xlate <jobs.json> <outdir>`: one job object per line.`"mode":"crash"` kills the whole script,
/// `"mode":"hang"` sleeps, `"mode":"red"` reports `[RED] <key>: bad shader`, anything else writes both outputs.
const FAKE: &str = r#"#!/bin/sh
jobs="$1"; out="$2"
grep '"key"' "$jobs" > "$out/.lines.$$"
while IFS= read -r line; do
  key=$(echo "$line" | sed 's/.*"key":"\([A-Za-z0-9_]*\)".*/\1/')
  case "$line" in
    *'"mode":"crash"'*) rm -f "$out/.lines.$$"; kill -SEGV $$ ;;
    *'"mode":"hang"'*) /bin/sleep 30 ;;
    *'"mode":"red"'*) echo "[RED] $key: bad shader" >&2 ;;
    *) printf 'spv-%s' "$key" > "$out/$key.spv"; printf '{}' > "$out/$key.res.json" ;;
  esac
done < "$out/.lines.$$"
rm -f "$out/.lines.$$"
exit 3
"#;

fn setup(base: Duration, per_key: Duration) -> (ProcessTranslator, PathBuf) {
    let root = std::env::var_os("NXB_TEST_TMP")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!(
            "nxb-xlate-test-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
    let _ = fs::remove_dir_all(&root);
    let work = root.join("work");
    fs::create_dir_all(&work).unwrap();
    let script = root.join("fake_xlate");
    fs::write(&script, FAKE).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let env = EnvPolicy::empty().set("PATH", "/usr/bin:/bin").unwrap();
    let mut cfg = XlateConfig::new(work, env);
    cfg.base_timeout = base;
    cfg.per_key = per_key;
    let id = ToolId {
        name: "dmsh_xlate".into(),
        path: script,
        version: "fake".into(),
    };
    (
        ProcessTranslator::new(id, Arc::new(ProcessRegistry::new()), cfg),
        root,
    )
}

fn jobs(modes: &[&str]) -> Vec<XlateJob> {
    modes
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let key = format!("{i:016x}");
            XlateJob {
                job_json: format!(r#"{{"key":"{key}","stage":"ps","mode":"{m}"}}"#),
                key,
            }
        })
        .collect()
}

fn detail(r: &XlateResult) -> Option<&str> {
    r.outcome.as_ref().err().map(|f| f.detail.as_str())
}

#[test]
fn normal_batch_with_red_key() {
    let (t, root) = setup(Duration::from_secs(20), Duration::from_secs(1));
    let res = t.translate(&jobs(&["ok", "red", "ok"])).unwrap();
    assert_eq!(t.runs(), 1);
    assert_eq!(
        res[0].outcome.as_ref().unwrap().spirv,
        b"spv-0000000000000000"
    );
    assert_eq!(detail(&res[1]), Some("bad shader"));
    assert!(res[2].outcome.is_ok());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn red_lines_parse() {
    let m = red_lines("noise\n[RED] abc_1: msg: with colon\n[RED] bad key: x\n");
    assert_eq!(m.get("abc_1").map(String::as_str), Some("msg: with colon"));
    assert_eq!(m.len(), 1);
}

/// One poison key among 16 crashes the translator: bisection isolates exactly it, every other key translates,
/// and the number of runs stays within 2 + 2·log2(16).
#[test]
fn crash_is_bisected_to_the_poison_key() {
    let (t, root) = setup(Duration::from_secs(20), Duration::from_secs(1));
    let mut modes = vec!["ok"; 16];
    modes[5] = "crash";
    let res = t.translate(&jobs(&modes)).unwrap();
    for (i, r) in res.iter().enumerate() {
        if i == 5 {
            assert_eq!(detail(r), Some("crashed: signal 11"));
        } else {
            assert!(r.outcome.is_ok(), "key {i}: {:?}", detail(r));
        }
    }
    assert!(t.runs() <= 10, "runs {}", t.runs());
    fs::remove_dir_all(root).unwrap();
}

/// Two poison keys and a red key: each poison fails alone, the red message is kept from whichever run reported it.
#[test]
fn several_poison_keys() {
    let (t, root) = setup(Duration::from_secs(20), Duration::from_secs(1));
    let mut modes = vec!["ok"; 12];
    modes[0] = "crash";
    modes[7] = "crash";
    modes[9] = "red";
    let res = t.translate(&jobs(&modes)).unwrap();
    for (i, r) in res.iter().enumerate() {
        match i {
            0 | 7 => assert_eq!(detail(r), Some("crashed: signal 11")),
            9 => assert_eq!(detail(r), Some("bad shader")),
            _ => assert!(r.outcome.is_ok(), "key {i}"),
        }
    }
    fs::remove_dir_all(root).unwrap();
}

/// A hanging key is killed by the watchdog and isolated, the others translate. The bases are generous because the
/// endpoint scanner can stall an exec by seconds under load.
#[test]
fn hang_is_isolated() {
    let (t, root) = setup(Duration::from_millis(2500), Duration::from_millis(100));
    let start = Instant::now();
    let res = t
        .translate(&jobs(&["ok", "ok", "hang", "ok", "ok"]))
        .unwrap();
    assert_eq!(detail(&res[2]), Some("timed out"));
    for i in [0, 1, 3, 4] {
        assert!(res[i].outcome.is_ok(), "key {i}");
    }
    assert!(start.elapsed() < Duration::from_secs(30));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn missing_translator_fails_the_bridge() {
    let (mut t, root) = setup(Duration::from_secs(5), Duration::from_secs(1));
    t.id.path = root.join("not-there");
    let e = t.translate(&jobs(&["ok"])).unwrap_err();
    assert_eq!(
        e.env.map(|x| x.kind),
        Some(nxb_error::EnvKind::ToolchainMissing)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn invalid_keys_never_reach_the_file_system() {
    let (t, root) = setup(Duration::from_secs(5), Duration::from_secs(1));
    let mut j = jobs(&["ok"]);
    j[0].key = "../evil".into();
    let res = t.translate(&j).unwrap();
    assert_eq!(detail(&res[0]), Some("invalid key"));
    assert_eq!(t.runs(), 0);
    fs::remove_dir_all(root).unwrap();
}
