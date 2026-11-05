//! `shaderbuild guards <b-dir>`: the profile tables `c1_full.tsv` and `cb_extent.tsv` of a finished output folder
//! (`c1_guard.py`, `cb_extent.py` main), each shader checked on the executor's workers, rows in Python's order.

use nxb_pyjson::Value;
use nxb_sched::{Budget, Executor, Task};
use nxb_shader::guards::{c1_unproven, cb_row};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

/// `OUTPUT.fullmatch`: `[0-9a-f]{16}_b\w*\.(metal|json)`.
fn is_output(name: &str, ext: &str) -> bool {
    let Some(stem) = name.strip_suffix(&format!(".{ext}")) else {
        return false;
    };
    stem.len() >= 18
        && stem.is_char_boundary(16)
        && stem[..16]
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        && stem[16..].starts_with("_b")
        && stem[18..].chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// `dmsh_chain.table_head(lib)`: `# chain <dir name> inputs <stamp inputs or none>`.
pub fn table_head(b_dir: &Path) -> String {
    let inputs = fs::read_to_string(b_dir.join("inputs.tsv"))
        .ok()
        .and_then(|t| {
            t.lines()
                .filter(|l| !l.starts_with('#'))
                .find_map(|l| l.strip_prefix("inputs\t").map(str::to_string))
        })
        .unwrap_or_else(|| "none".into());
    let name = b_dir
        .file_name()
        .map_or(String::new(), |n| n.to_string_lossy().into_owned());
    format!("# chain {name} inputs {inputs}")
}

/// ISO 8601 UTC time of now, seconds (`+00:00`, Python writes local time with its offset: the stamp never compares).
fn stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs()) as i64;
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    // Civil from days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}+00:00",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// The two tables' bodies (without headers): unproven c1 keys, and cb_extent rows.
pub fn bodies(b_dir: &Path, jobs: u32) -> std::io::Result<(Vec<String>, Vec<String>)> {
    let mut names: Vec<String> = fs::read_dir(b_dir)?
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .collect();
    names.sort();
    let metal: Vec<String> = names
        .iter()
        .filter(|n| is_output(n, "metal"))
        .cloned()
        .collect();
    let json: Vec<String> = names
        .iter()
        .filter(|n| is_output(n, "json") && n.ends_with("_b.json"))
        .cloned()
        .collect();
    let c1: Arc<Mutex<Vec<(usize, String)>>> = Arc::default();
    let cb: Arc<Mutex<Vec<(usize, String)>>> = Arc::default();
    let dir = Arc::new(b_dir.to_path_buf());
    let refl = |dir: &Path, key: &str| -> Option<Value> {
        fs::read_to_string(dir.join(format!("{key}_b.json")))
            .ok()
            .and_then(|t| nxb_pyjson::parse(&t).ok())
    };
    let mut tasks = Vec::new();
    for (i, n) in metal.into_iter().enumerate() {
        let (c1, dir) = (c1.clone(), dir.clone());
        tasks.push(Task::new(move |_| {
            let Ok(src) = fs::read_to_string(dir.join(&n)) else {
                return;
            };
            let key = n
                .strip_suffix("_b.metal")
                .unwrap_or_else(|| &n[..n.len() - 6])
                .to_string();
            if c1_unproven(&src, refl(&dir, &key).as_ref()) {
                c1.lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push((i, key));
            }
        }));
    }
    for (i, n) in json.into_iter().enumerate() {
        let (cb, dir) = (cb.clone(), dir.clone());
        tasks.push(Task::new(move |_| {
            let key = n[..n.len() - "_b.json".len()].to_string();
            let Some(r) = refl(&dir, &key) else { return };
            let src = fs::read_to_string(dir.join(format!("{key}_b.metal")));
            let has = r.get("bindings").and_then(Value::as_list).is_some_and(|b| {
                b.iter()
                    .any(|x| matches!(x.get("kind").and_then(Value::as_str), Some("cbuf" | "sbuf")))
            });
            let Ok(src) = (if has { src } else { Ok(String::new()) }) else {
                return;
            };
            if let Some(toks) = cb_row(&src, &r) {
                cb.lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push((i, format!("{key}\t{toks}")));
            }
        }));
    }
    Executor::new(Budget::fixed(jobs)).run(tasks);
    let take = |m: &Arc<Mutex<Vec<(usize, String)>>>| {
        let mut v = std::mem::take(&mut *m.lock().unwrap_or_else(PoisonError::into_inner));
        v.sort();
        v.into_iter().map(|(_, s)| s).collect::<Vec<_>>()
    };
    Ok((take(&c1), take(&cb)))
}

/// Write both tables as Python does into `out_dir`.
pub fn write(b_dir: &Path, out_dir: &Path, jobs: u32) -> std::io::Result<(usize, usize)> {
    let (c1, cb) = bodies(b_dir, jobs)?;
    let head = format!("{} {}\n", table_head(b_dir), stamp());
    fs::create_dir_all(out_dir)?;
    let mut t = head.clone();
    for k in &c1 {
        t.push_str(k);
        t.push('\n');
    }
    fs::write(out_dir.join("c1_full.tsv"), t)?;
    let mut t = head;
    t.push_str("# shader key, then reg:bytes for each uniform slot whose reads stay inside its first bytes, and s<reg> for each storage slot it never writes (cb_extent.py)\n");
    for r in &cb {
        t.push_str(r);
        t.push('\n');
    }
    fs::write(out_dir.join("cb_extent.tsv"), t)?;
    Ok((c1.len(), cb.len()))
}
