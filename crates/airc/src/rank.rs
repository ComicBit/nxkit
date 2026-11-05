//! `airc rank`: which constructs the inputs use, in which order supporting them unlocks the
//! most inputs, and (with `--results` from `airc check`) how many inputs with each construct
//! already lower and validate.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::Opts;

/// Construct counters that say nothing about lowering work.
const IGNORED: &[&str] = &[
    "stmt",
    "lit",
    "member",
    "ssa-names-reassigned",
    "decl:ssa",
    "pp",
    "struct",
    "function",
];

fn constructs(path: &std::path::Path) -> Option<Vec<String>> {
    let out = msl_parse::check_file(path).ok()?;
    let msl_parse::Outcome::Checked(c) = out else {
        return Some(vec!["parse:fail".into()]);
    };
    let mut keys: Vec<String> = c
        .stats
        .iter()
        .map(|(k, _)| k.to_string())
        .filter(|k| !IGNORED.contains(&k.as_str()))
        .collect();
    for (k, _) in c.reasons.iter() {
        keys.push(format!("rejected:{k}"));
    }
    Some(keys)
}

/// Minimal reader of the JSONL that `airc check` writes: path -> (stage, key).
fn read_results(path: &std::path::Path) -> HashMap<String, (String, String)> {
    let mut out = HashMap::new();
    let Ok(text) = std::fs::read_to_string(path) else {
        return out;
    };
    for line in text.lines() {
        let field = |name: &str| -> Option<String> {
            let pat = format!("\"{name}\": \"");
            let k = line.find(&pat)? + pat.len();
            let mut s = String::new();
            let mut esc = false;
            for c in line[k..].chars() {
                if esc {
                    s.push(match c {
                        'n' => '\n',
                        't' => '\t',
                        c => c,
                    });
                    esc = false;
                } else if c == '\\' {
                    esc = true;
                } else if c == '"' {
                    break;
                } else {
                    s.push(c);
                }
            }
            Some(s)
        };
        if let (Some(p), Some(st), Some(k)) = (field("path"), field("stage"), field("key")) {
            out.insert(p, (st, k));
        }
    }
    out
}

/// Builds the report.
pub fn run(opts: &Opts) -> ExitCode {
    let n = opts.inputs.len();
    let next = AtomicUsize::new(0);
    let sets: Mutex<Vec<Option<Vec<String>>>> = Mutex::new(vec![None; n]);
    std::thread::scope(|sc| {
        for _ in 0..opts.jobs {
            sc.spawn(|| loop {
                let k = next.fetch_add(1, Ordering::Relaxed);
                let Some(p) = opts.inputs.get(k) else { break };
                let c = constructs(p);
                if let Ok(mut s) = sets.lock() {
                    s[k] = c;
                }
            });
        }
    });
    let sets = sets.into_inner().unwrap_or_default();
    let results = opts
        .results
        .as_deref()
        .map(read_results)
        .unwrap_or_default();

    // key index
    let mut names: Vec<String> = Vec::new();
    let mut idx: HashMap<String, usize> = HashMap::new();
    let mut files_of: Vec<Vec<usize>> = Vec::new();
    for (f, s) in sets.iter().enumerate() {
        for k in s.iter().flatten() {
            let id = *idx.entry(k.clone()).or_insert_with(|| {
                names.push(k.clone());
                files_of.push(Vec::new());
                names.len() - 1
            });
            files_of[id].push(f);
        }
    }
    let ok: Vec<bool> = opts
        .inputs
        .iter()
        .map(|p| {
            results
                .get(&p.display().to_string())
                .is_some_and(|(s, _)| s == "ok")
        })
        .collect();

    // reverse greedy: drop the key that loses the fewest still-covered files
    let mut covered = vec![true; n];
    for (f, s) in sets.iter().enumerate() {
        if s.is_none() {
            covered[f] = false;
        }
    }
    let mut alive: Vec<bool> = vec![true; names.len()];
    let mut removal: Vec<(usize, usize)> = Vec::new();
    let mut n_cov = covered.iter().filter(|c| **c).count();
    for _ in 0..names.len() {
        let mut best: Option<(usize, usize)> = None;
        for (k, files) in files_of.iter().enumerate() {
            if !alive[k] {
                continue;
            }
            let lost = files.iter().filter(|f| covered[**f]).count();
            if best.is_none_or(|(_, b)| lost < b) {
                best = Some((k, lost));
            }
        }
        let Some((k, _)) = best else { break };
        alive[k] = false;
        for f in &files_of[k] {
            if covered[*f] {
                covered[*f] = false;
                n_cov -= 1;
            }
        }
        removal.push((k, n_cov));
    }
    // unlock order: reverse of removal, files covered once keys 0..=i are supported
    let mut order: Vec<(usize, usize)> = Vec::new();
    for (pos, (k, _)) in removal.iter().enumerate().rev() {
        let cov_after = if pos == 0 {
            sets.iter().filter(|s| s.is_some()).count()
        } else {
            removal[pos - 1].1
        };
        order.push((*k, cov_after));
    }

    let mut md = String::new();
    let _ = writeln!(md, "# Construct ranking over {n} inputs\n");
    let _ = writeln!(
        md,
        "{} distinct construct keys (parser counters of `msl-parse`).\n",
        names.len()
    );
    if !results.is_empty() {
        let n_ok = ok.iter().filter(|x| **x).count();
        let _ = writeln!(
            md,
            "Inputs that fully lower and validate: **{n_ok} / {n}**.\n"
        );
        let mut blockers: BTreeMap<String, usize> = BTreeMap::new();
        for p in &opts.inputs {
            if let Some((st, k)) = results.get(&p.display().to_string()) {
                if st != "ok" {
                    let key = if st == "front" {
                        k.clone()
                    } else {
                        format!("{st}: {k}")
                    };
                    *blockers.entry(key).or_default() += 1;
                }
            }
        }
        let mut bl: Vec<(String, usize)> = blockers.into_iter().collect();
        bl.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let _ = writeln!(
            md,
            "## First blocker per input (top {})\n\n| blocker | inputs |\n|---|---|",
            opts.top
        );
        for (k, c) in bl.iter().take(opts.top) {
            let _ = writeln!(md, "| `{}` | {c} |", k.replace('|', "\\|"));
        }
        md.push('\n');
    }
    let _ = writeln!(md, "## Unlock order (reverse greedy)\n");
    let _ = writeln!(
        md,
        "Supporting the keys up to row K covers the inputs in the last column.\n"
    );
    let _ = writeln!(md, "| K | key | files with key | covered after K |");
    let _ = writeln!(md, "|---|---|---|---|");
    for (pos, (k, cov)) in order.iter().enumerate() {
        if *cov == 0 && pos + 1 < order.len() && order[pos + 1].1 == 0 {
            continue;
        }
        let _ = writeln!(
            md,
            "| {} | `{}` | {} | {} |",
            pos + 1,
            names[*k],
            files_of[*k].len(),
            cov
        );
    }
    md.push('\n');
    let _ = writeln!(md, "## Prevalence by family\n");
    let _ = writeln!(md, "| key | files | files that validate |");
    let _ = writeln!(md, "|---|---|---|");
    let mut keys: Vec<usize> = (0..names.len()).collect();
    keys.sort_by(|a, b| {
        names[*a]
            .split(':')
            .next()
            .cmp(&names[*b].split(':').next())
            .then(files_of[*b].len().cmp(&files_of[*a].len()))
    });
    for k in keys {
        let okc = files_of[k].iter().filter(|f| ok[**f]).count();
        let _ = writeln!(md, "| `{}` | {} | {} |", names[k], files_of[k].len(), okc);
    }
    print!("{md}");
    ExitCode::SUCCESS
}
