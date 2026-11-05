//! Text summary and TSV of a [`crate::recompile::RecompileReport`].

use crate::recompile::{Outcome, RecompileReport, Row};
use std::fmt::Write as _;
use std::time::Duration;

fn secs(d: Duration) -> f64 {
    d.as_secs_f64()
}

fn quantile(mut v: Vec<f64>, q: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    // q in 0..=1 and the index stays in range.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let i = ((v.len() - 1) as f64 * q).round() as usize;
    v[i.min(v.len() - 1)]
}

/// Human summary: identity rate, throughput, where the time goes.
pub fn summary(r: &RecompileReport, jobs: u32) -> String {
    let n = r.rows.len();
    let count = |f: &dyn Fn(&Row) -> bool| r.rows.iter().filter(|x| f(x)).count();
    let identical = count(&|x| x.outcome == Outcome::Identical);
    let different = count(&|x| x.outcome == Outcome::Different);
    let failed = count(&|x| matches!(x.outcome, Outcome::Failed(_)));
    let noref = count(&|x| x.outcome == Outcome::NoReference);
    let compiled: Vec<&Row> = r.rows.iter().filter(|x| x.source == "compiled").collect();
    let wall = secs(r.wall);
    let ms = |v: Vec<f64>, q| quantile(v, q) * 1000.0;
    let cw: Vec<f64> = compiled.iter().map(|x| secs(x.compile.wall)).collect();
    let cc: Vec<f64> = compiled.iter().map(|x| secs(x.compile.cpu)).collect();
    let lw: Vec<f64> = compiled.iter().map(|x| secs(x.link.wall)).collect();
    let lc: Vec<f64> = compiled.iter().map(|x| secs(x.link.cpu)).collect();
    let task: f64 = r.rows.iter().map(|x| secs(x.task_wall)).sum();
    let store: f64 = r.rows.iter().map(|x| secs(x.store)).sum();
    let cmpw: f64 = r.rows.iter().map(|x| secs(x.compare)).sum();
    let sidew: f64 = r.rows.iter().map(|x| secs(x.side)).sum();
    let sw: Vec<f64> = compiled.iter().map(|x| secs(x.store)).collect();
    let (cws, ccs, lws, lcs): (f64, f64, f64, f64) = (
        cw.iter().sum(),
        cc.iter().sum(),
        lw.iter().sum(),
        lc.iter().sum(),
    );
    let rss = compiled
        .iter()
        .map(|x| x.compile.max_rss)
        .max()
        .unwrap_or(0);
    let mut o = String::new();
    let _ = writeln!(
        o,
        "recompile: {n} shaders, identical {identical} ({:.2} %), different {different}, failed {failed}, no reference {noref}",
        if n == 0 {
            0.0
        } else {
            100.0 * identical as f64 / n as f64
        }
    );
    let _ = writeln!(
        o,
        "kinds: plain {}, side (unit compile + cut into _b and _bx) {}, variants _bp {} _bv {} _bf {} (of which no variant on either side {})",
        count(&|x| x.kind == "plain"),
        count(&|x| x.kind == "side"),
        count(&|x| x.kind == "bp"),
        count(&|x| x.kind == "bv"),
        count(&|x| x.kind == "bf"),
        count(&|x| x.source == "none")
    );
    for k in ["plain", "side", "bp", "bv", "bf"] {
        let all = count(&|x| x.kind == k);
        let same = count(&|x| x.kind == k && x.outcome == Outcome::Identical);
        let _ = writeln!(o, "  {k}: {same} of {all} identical");
    }
    let _ = writeln!(
        o,
        "sources: compiled {}, store {}, joined {} (single-flight); header {}",
        compiled.len(),
        count(&|x| x.source == "store"),
        count(&|x| x.source == "joined"),
        if r.pch { "on" } else { "off" }
    );
    let _ = writeln!(
        o,
        "wall {wall:.1} s (setup {:.1} s) with {jobs} jobs: {:.1} shaders/s, {:.1} compiles/s; worker utilisation {:.0} %",
        secs(r.setup),
        if wall > 0.0 { n as f64 / wall } else { 0.0 },
        if wall > 0.0 {
            compiled.len() as f64 / wall
        } else {
            0.0
        },
        100.0 * r.exec.utilisation()
    );
    let _ = writeln!(
        o,
        "metal -c: wall median {:.0} ms p90 {:.0} ms, CPU median {:.0} ms, sum {ccs:.0} core-s; peak RSS {} MB",
        ms(cw.clone(), 0.5),
        ms(cw, 0.9),
        ms(cc, 0.5),
        rss >> 20
    );
    let _ = writeln!(
        o,
        "metallib: wall median {:.0} ms, CPU median {:.0} ms, sum {lcs:.0} core-s",
        ms(lw, 0.5),
        ms(lc, 0.5)
    );
    let other = task - cws - lws - store - cmpw - sidew;
    let pct = |x: f64| 100.0 * x / task.max(1e-9);
    let _ = writeln!(
        o,
        "task wall split: metal {:.0} %, metallib {:.0} %, store write {:.0} % (median {:.0} ms), compare {:.0} %, unit + cut {:.1} %, rest (read, key, queue) {:.0} %; children CPU {:.1} cores busy on average",
        pct(cws),
        pct(lws),
        pct(store),
        ms(sw, 0.5),
        pct(cmpw),
        pct(sidew),
        pct(other),
        (ccs + lcs) / wall.max(1e-9)
    );
    o
}

/// One line per shader: key, outcome, source, MSL bytes, compile wall/CPU ms, link wall/CPU ms, task ms.
pub fn tsv(r: &RecompileReport) -> String {
    let mut o = String::from(
        "key\toutcome\tsource\tkind\tmsl_bytes\tcompile_ms\tcompile_cpu_ms\tlink_ms\tlink_cpu_ms\ttask_ms\n",
    );
    for x in &r.rows {
        let outcome = match &x.outcome {
            Outcome::Identical => "identical".to_string(),
            Outcome::Different => "different".to_string(),
            Outcome::NoReference => "no_reference".to_string(),
            Outcome::Failed(m) => format!("failed: {}", m.replace(['\t', '\n'], " ")),
        };
        let _ = writeln!(
            o,
            "{}\t{outcome}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            x.key,
            x.source,
            x.kind,
            x.msl_bytes,
            x.compile.wall.as_millis(),
            x.compile.cpu.as_millis(),
            x.link.wall.as_millis(),
            x.link.cpu.as_millis(),
            x.store.as_millis(),
            x.task_wall.as_millis()
        );
    }
    o
}
