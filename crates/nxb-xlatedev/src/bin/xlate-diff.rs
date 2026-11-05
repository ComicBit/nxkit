//! `xlate-diff <corpus-dir> [report.tsv]`: run the Rust translator on every job file of `<corpus-dir>/jobs` and compare
//! with the C# outputs in `<corpus-dir>/ref/<job file stem>/` (`.spv`, `.res.json`, `.reader.spv`, red lines in
//! `<stem>.stderr`). Prints the counts per stage and class and writes one row per key to the report.
//! Classes: `identical`, `both-red`, `unported:<reason>`, `rust-error:<reason>`, `differs-spv`, `differs-res`,
//! `differs-reader`, `rust-ok-cs-red`, `cs-missing`, `panic`. With `XLATE_DIFF_OUT=<dir>` the Rust SPIR-V of every
//! `differs-spv` key is written there as `<key>.spv`.

use nxb_pyjson::parse;
use nxb_xlate::driver::run_job;
use nxb_xlate::job::Job;
use nxb_xlate::translation::core::Translator;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

fn red_keys(stderr: &Path) -> BTreeMap<String, String> {
    let text = std::fs::read_to_string(stderr).unwrap_or_default();
    let mut m = BTreeMap::new();
    for l in text.lines() {
        if let Some(r) = l.strip_prefix("[RED] ")
            && let Some((k, msg)) = r.split_once(": ")
        {
            m.insert(k.to_string(), msg.to_string());
        }
    }
    m
}

fn classify(job: &Job, ref_dir: &Path, red: &BTreeMap<String, String>) -> (String, String) {
    let cs_spv = std::fs::read(ref_dir.join(format!("{}.spv", job.key))).ok();
    let cs_res = std::fs::read(ref_dir.join(format!("{}.res.json", job.key))).ok();
    let run = std::fs::read(&job.code_path)
        .map_err(|e| e.to_string())
        .and_then(|code| {
            std::panic::catch_unwind(|| run_job(&Translator, job, &code))
                .unwrap_or_else(|_| Err("panic".to_string()))
        });
    let short = |e: &str| {
        e.lines()
            .next()
            .unwrap_or("")
            .chars()
            .take(110)
            .collect::<String>()
    };
    match (run, cs_spv, cs_res) {
        (Ok(o), Some(spv), Some(res)) => {
            if o.spv != spv {
                if let Some(dir) = std::env::var_os("XLATE_DIFF_OUT") {
                    let _ =
                        std::fs::write(Path::new(&dir).join(format!("{}.spv", job.key)), &o.spv);
                }
                return ("differs-spv".into(), String::new());
            }
            if o.res.to_text().as_bytes() != res.as_slice() {
                return ("differs-res".into(), String::new());
            }
            if let Some(rd) = &o.reader {
                let cs = std::fs::read(ref_dir.join(format!("{}.reader.spv", job.key))).ok();
                if cs.as_deref() != Some(rd.as_slice()) {
                    return ("differs-reader".into(), String::new());
                }
            }
            ("identical".into(), String::new())
        }
        (Ok(_), _, _) => (
            "rust-ok-cs-red".into(),
            red.get(&job.key).cloned().unwrap_or_default(),
        ),
        (Err(e), Some(_), Some(_)) => {
            let why = short(&e);
            if e == "panic" {
                ("panic".into(), why)
            } else if e.starts_with("unported") {
                (format!("unported:{why}"), String::new())
            } else {
                (format!("rust-error:{why}"), String::new())
            }
        }
        (Err(e), _, _) => {
            if red.contains_key(&job.key) {
                ("both-red".into(), short(&e))
            } else {
                ("cs-missing".into(), short(&e))
            }
        }
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let Some(dir) = a.first().map(PathBuf::from) else {
        eprintln!("usage: xlate-diff <corpus-dir> [report.tsv]");
        std::process::exit(2);
    };
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir.join("jobs"))
        .map(|d| d.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    files.sort();
    let next = Mutex::new(0usize);
    let rows = Mutex::new(Vec::<(String, String, String, String)>::new());
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(8);
    std::panic::set_hook(Box::new(|_| {}));
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = {
                        let mut n = next.lock().unwrap_or_else(|e| e.into_inner());
                        let i = *n;
                        *n += 1;
                        i
                    };
                    let Some(f) = files.get(i) else { break };
                    let stem = f
                        .file_stem()
                        .and_then(|x| x.to_str())
                        .unwrap_or("")
                        .to_string();
                    let ref_dir = dir.join("ref").join(&stem);
                    let red = red_keys(&dir.join("ref").join(format!("{stem}.stderr")));
                    let text = std::fs::read_to_string(f).unwrap_or_default();
                    let Ok(jobs) = nxb_xlate::job::parse_jobs(&text) else {
                        continue;
                    };
                    let _ = parse;
                    for job in &jobs {
                        let (c, why) = classify(job, &ref_dir, &red);
                        let mut r = rows.lock().unwrap_or_else(|e| e.into_inner());
                        r.push((job.key.clone(), job.stage.clone(), c, why));
                    }
                }
            });
        }
    });
    let mut rows = rows.into_inner().unwrap_or_default();
    rows.sort();
    let mut count: BTreeMap<(String, String), usize> = BTreeMap::new();
    for (_, st, c, _) in &rows {
        *count.entry((st.clone(), c.clone())).or_default() += 1;
    }
    let mut per_stage: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for ((st, c), n) in &count {
        let e = per_stage.entry(st).or_default();
        e.0 += n;
        if c == "identical" {
            e.1 += n;
        }
    }
    for (st, (n, same)) in &per_stage {
        println!("stage {st}: {same} of {n} identical");
    }
    for ((st, c), n) in &count {
        if c != "identical" {
            println!("  {st:3} {n:6} {c}");
        }
    }
    let total = rows.len();
    let same = rows.iter().filter(|r| r.2 == "identical").count();
    println!("total: {same} of {total} identical");
    if let Some(p) = a.get(1) {
        let text: String = rows
            .iter()
            .map(|(k, s, c, w)| format!("{k}\t{s}\t{c}\t{w}\n"))
            .collect();
        let _ = std::fs::write(p, text);
    }
}
