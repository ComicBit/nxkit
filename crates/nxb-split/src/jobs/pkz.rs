//! PlatinumGames PKZ archives (`pkz_entries`, `pkz_jobs`, `pkz_shaders`), Oodle entries are `Unsupported`.

use std::sync::Arc;

use super::{Ctx, File, Job, JobOut, Plan, R, ranges};
use crate::SplitError;
use crate::bahs::BahsVar;
use crate::rd::{red, u32le, u64le};
use crate::shaders::{Found, scan_blob};
use crate::src::Src;

/// (name, compression, offset, stored length, length)
type Entry = (String, String, u64, u64, u64);

fn entries(s: &dyn Src, rel: &str) -> R<Option<Vec<Entry>>> {
    let h = s.read_up(0, 0x20)?;
    if h.get(..4) != Some(b"pkzl") {
        return Ok(None);
    }
    let (ver, n, eo, nl) = (
        u32le(&h, 4)?,
        u32le(&h, 16)?,
        u32le(&h, 20)?,
        u32le(&h, 24)?,
    );
    if ver != 0x10000 && ver != 0x20000 {
        return red(format!(
            "/{rel}: PKZ version {ver:#x}: only 0x10000 and 0x20000 are known"
        ));
    }
    let t = s.read_up(eo as u64, 0x20 * n + nl)?;
    let nm = |o: usize| -> R<String> {
        let a = 0x20 * n + o;
        let z = t
            .get(a..)
            .and_then(|x| x.iter().position(|&b| b == 0))
            .ok_or_else(|| SplitError::Red("ValueError: substring not found".into()))?;
        String::from_utf8(t[a..a + z].to_vec())
            .map_err(|_| SplitError::Red("UnicodeDecodeError".into()))
    };
    let mut out = Vec::new();
    for i in 0..n {
        let r = i * 0x20;
        let (a, c, usz, off, csz) = (
            u32le(&t, r)?,
            u32le(&t, r + 4)?,
            u64le(&t, r + 8)?,
            u64le(&t, r + 16)?,
            u64le(&t, r + 24)?,
        );
        out.push((nm(a)?, nm(c)?, off, csz, usz));
    }
    Ok(Some(out))
}

fn shaders(s: &dyn Src, rel: &str, ents: &[Entry]) -> R<JobOut> {
    let (mut out, mut var) = (Vec::<Found>::new(), BahsVar::new());
    for (name, comp, off, csz, usz) in ents {
        let c = s.read_up(*off, *csz as usize)?;
        let d = match comp.as_str() {
            "None" => c,
            "OodleKraken" => crate::oodle::decode(&c, *usz as usize, &format!("/{rel}/{name}"))?,
            "ZStandard" => crate::zstd::decompress(&c)?,
            _ => return red(format!("/{rel}/{name}: compression {comp:?} is not known")),
        };
        if d.len() as u64 != *usz {
            return red(format!(
                "/{rel}/{name}: {} bytes decoded, the entry says {usz}",
                d.len()
            ));
        }
        scan_blob(&d, &format!("/{rel}/{name}"), &mut out, &mut var, 0)?;
    }
    Ok(JobOut { found: out, var })
}

/// `pkz_jobs`.
pub fn plan(files: &[File], p: &mut Plan) -> R<()> {
    let mut pk = Vec::new();
    for (i, f) in files.iter().enumerate() {
        if f.live
            && f.rel.ends_with(".pkz")
            && let Some(es) = entries(f.src.as_ref(), &f.rel)?
        {
            pk.push((i, es));
        }
    }
    let step = pk.iter().flat_map(|x| x.1.iter().map(|e| e.3)).sum::<u64>() as f64 / 64.0;
    for (i, es) in pk {
        p.taken.insert(i);
        for chunk in ranges(&es, |e| e.3, step) {
            let (src, rel): (Arc<dyn Src>, String) = (files[i].src.clone(), files[i].rel.clone());
            let r2 = rel.clone();
            p.jobs.push(Job {
                rel,
                run: Box::new(move |_: &Ctx<'_>, _| shaders(src.as_ref(), &r2, &chunk)),
            });
        }
    }
    Ok(())
}
