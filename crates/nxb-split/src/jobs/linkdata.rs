//! Koei Tecmo's LINKDATA archives with an index beside them (`linkdata_jobs`, `linkdata_shaders`).

use std::sync::Arc;

use super::{Ctx, File, Job, JobOut, Plan, R};
use crate::bahs::BahsVar;
use crate::rd::u64le;
use crate::shaders::Found;
use crate::src::Src;

/// The index beside a LINKDATA archive: Three Houses' `DATA0.bin` beside `DATA1.bin`, `LI_x.IDX` beside `LD_x.BIN`.
fn index_of(rel: &str, files: &[File]) -> Option<usize> {
    let (dir, name) = rel.rsplit_once('/').map_or(("", rel), |(d, n)| (d, n));
    let idx = if name == "DATA1.bin" {
        "DATA0.bin".to_string()
    } else if name.starts_with("LD_") && name.ends_with(".BIN") {
        format!("LI_{}.IDX", &name[3..name.len() - 4])
    } else {
        return None;
    };
    let want = if dir.is_empty() {
        idx
    } else {
        format!("{dir}/{idx}")
    };
    files.iter().position(|f| f.rel == want)
}

fn read_index(s: &dyn Src) -> R<Vec<[u64; 4]>> {
    let b = s.read_up(0, (s.len() / 32 * 32) as usize)?;
    b.chunks_exact(32)
        .map(|c| Ok([u64le(c, 0)?, u64le(c, 8)?, u64le(c, 16)?, u64le(c, 24)?]))
        .collect()
}

fn shaders(s: &dyn Src, idx: &[[u64; 4]], rel: &str, lo: usize, hi: usize) -> R<JobOut> {
    let (mut out, mut var) = (Vec::<Found>::new(), BahsVar::new());
    for (i, e) in idx.iter().enumerate().take(hi).skip(lo) {
        let [off, n, stored, comp] = *e;
        if n != 0 {
            let d = s.read_up(off, (if comp != 0 { stored } else { n }) as usize)?;
            crate::packs::kt_shaders(&d, &format!("/{rel}#{i}"), &mut out, &mut var)?;
        }
    }
    Ok(JobOut { found: out, var })
}

/// `linkdata_jobs` over every file: one job per range of entries of about equal stored size.
pub fn plan(files: &[File], p: &mut Plan) -> R<()> {
    for (i, f) in files.iter().enumerate() {
        let Some(xi) = index_of(&f.rel, files) else {
            continue;
        };
        let idx = read_index(files[xi].src.as_ref())?;
        if idx.is_empty() {
            continue;
        }
        let n = idx.len();
        let step = f.src.len() as f64 / 64.0;
        let (mut jobs, mut lo) = (0usize, 0usize);
        let idx = Arc::new(idx);
        for k in 1..=n {
            if k == n || idx[k][0] as f64 >= (jobs + 1) as f64 * step {
                let (src, ix, rel): (Arc<dyn Src>, _, String) =
                    (f.src.clone(), idx.clone(), f.rel.clone());
                let (a, b) = (lo, k);
                let r2 = rel.clone();
                p.jobs.push(Job {
                    rel,
                    run: Box::new(move |_: &Ctx<'_>, _| shaders(src.as_ref(), &ix, &r2, a, b)),
                });
                jobs += 1;
                lo = k;
            }
        }
        p.taken.insert(i);
    }
    Ok(())
}
