//! Unreal Engine paks with inline shaders and IoStore containers beside them (`ue_legacy_jobs`, `ue_legacy_shaders`,
//! `ue_io_shaders`).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use super::uepak::{Pak, inline_shaders, normpath};
use super::uerec::inline_records;
use super::uetoc::{Toc, dirname, inline_in_containers, mount_order};
use super::{Ctx, File, Job, JobOut, Plan, R};
use crate::SplitError;
use crate::bahs::BahsVar;
use crate::src::Src;

fn legacy_shaders(pk: &Pak, rel: &str, names: &[String]) -> R<JobOut> {
    let mut out = Vec::new();
    for name in names {
        let d = pk.read(name)?;
        let n = normpath(&format!("{}{}", pk.mount, name));
        let label = n.trim_start_matches("../");
        inline_records(&d, &format!("/{rel}#{label}"), &mut out)?;
    }
    Ok(JobOut {
        found: out,
        var: BahsVar::new(),
    })
}

/// A container with its `.ucas` partitions.
type Opened = (Toc, Vec<Option<Arc<dyn Src>>>);

/// `ue_legacy_jobs`.
pub fn plan(files: &[File], p: &mut Plan) -> R<()> {
    let beside = |rel: &str| {
        inline_in_containers(
            files
                .iter()
                .filter(|x| x.rel.ends_with(".utoc") && dirname(&x.rel) == dirname(rel))
                .map(|x| (x.rel.as_str(), x.src.as_ref())),
        )
    };
    let paks: Vec<usize> = (0..files.len())
        .filter(|&i| {
            files[i].rel.ends_with(".pak")
                && inline_shaders(files[i].src.as_ref(), &files[i].rel, &|| {
                    beside(&files[i].rel)
                })
        })
        .collect();
    if paks.is_empty() {
        return Ok(());
    }
    // (dir, normalised path) -> (pak, name, raw size): a patch pak replaces
    let mut view: BTreeMap<(String, String), (usize, String, u64)> = BTreeMap::new();
    let mut sorted = paks.clone();
    sorted.sort_by(|&a, &b| {
        (files[a].rel.ends_with("_P.pak"), &files[a].rel)
            .cmp(&(files[b].rel.ends_with("_P.pak"), &files[b].rel))
    });
    let mut paks_open: HashMap<usize, Arc<Pak>> = HashMap::new();
    for &i in &sorted {
        let f = &files[i];
        let r = Pak::open(f.src.clone(), &f.rel).and_then(|pk| {
            let mut v = Vec::new();
            for name in &pk.names {
                v.push((name.clone(), pk.entry(name)?.raw));
            }
            Ok((pk, v))
        });
        match r {
            Ok((pk, v)) => {
                for (name, raw) in v {
                    view.insert(
                        (
                            dirname(&f.rel).to_string(),
                            normpath(&format!("{}{name}", pk.mount)),
                        ),
                        (i, name, raw),
                    );
                }
                paks_open.insert(i, Arc::new(pk));
            }
            Err(SplitError::Red(e)) => p.notes.push(format!(
                "[RED] /{}: the pak cannot be read ({e}); its shaders are left out",
                f.rel
            )),
            Err(e) => return Err(e),
        }
    }
    let folders: HashSet<&str> = paks.iter().map(|&i| dirname(&files[i].rel)).collect();
    p.taken.extend(paks.iter().copied());
    let mut tocs: Vec<usize> = (0..files.len())
        .filter(|&i| files[i].rel.ends_with(".utoc") && folders.contains(dirname(&files[i].rel)))
        .collect();
    tocs.sort_by_key(|&i| mount_order(&files[i].rel));
    // (dir, chunk id) -> (toc, chunk index, length)
    let mut chunks: BTreeMap<(String, [u8; 12]), (usize, usize, u64)> = BTreeMap::new();
    let mut tocs_open: HashMap<usize, Arc<Opened>> = HashMap::new();
    for i in tocs {
        let f = &files[i];
        let t = match Toc::load(f.src.as_ref(), &f.rel) {
            Ok(t) => t,
            Err(SplitError::Red(e)) => {
                p.notes.push(format!(
                    "[RED] /{}: the container cannot be read ({e}); its shaders are left out",
                    f.rel
                ));
                continue;
            }
            Err(e) => return Err(e),
        };
        let bundles = t.export_bundles();
        if bundles.is_empty() {
            continue;
        }
        if t.encrypted {
            p.notes.push(format!("[RED] /{}: the container is encrypted, which is not read here; its shaders are left out", f.rel));
            continue;
        }
        p.taken.insert(i);
        let mut ucas = Vec::new();
        for up in t.ucas_paths() {
            let ui = files.iter().position(|x| x.rel == up);
            if let Some(ui) = ui {
                p.taken.insert(ui);
            }
            ucas.push(ui.map(|ui| files[ui].src.clone()));
        }
        for &b in &bundles {
            chunks.insert(
                (dirname(&f.rel).to_string(), t.ids[b]),
                (i, b, t.ranges[b].1),
            );
        }
        tocs_open.insert(i, Arc::new((t, ucas)));
    }
    // legacy files, then export bundles: each in jobs of about equal raw size
    let step = view.values().map(|v| v.2).sum::<u64>() as f64 / 64.0;
    let mut items: Vec<&(usize, String, u64)> = view.values().collect();
    items.sort_by(|a, b| (&files[a.0].rel, &a.1).cmp(&(&files[b.0].rel, &b.1)));
    let (mut cur, mut order, mut size) = (
        HashMap::<usize, Vec<String>>::new(),
        Vec::<usize>::new(),
        0u64,
    );
    let flush = |cur: &mut HashMap<usize, Vec<String>>, order: &mut Vec<usize>, p: &mut Plan| {
        for i in order.drain(..) {
            let names = cur.remove(&i).unwrap_or_default();
            let (pk, rel) = (paks_open[&i].clone(), files[i].rel.clone());
            let r2 = rel.clone();
            p.jobs.push(Job {
                rel,
                run: Box::new(move |_: &Ctx<'_>, _| legacy_shaders(&pk, &r2, &names)),
            });
        }
    };
    for (i, name, raw) in items {
        if !cur.contains_key(i) {
            order.push(*i);
        }
        cur.entry(*i).or_default().push(name.clone());
        size += raw;
        if size as f64 >= step {
            flush(&mut cur, &mut order, p);
            size = 0;
        }
    }
    flush(&mut cur, &mut order, p);
    let step = chunks.values().map(|v| v.2).sum::<u64>() as f64 / 64.0;
    let mut items: Vec<&(usize, usize, u64)> = chunks.values().collect();
    items.sort_by(|a, b| (&files[a.0].rel, a.1).cmp(&(&files[b.0].rel, b.1)));
    let (mut cur, mut order, mut size) = (
        HashMap::<usize, Vec<usize>>::new(),
        Vec::<usize>::new(),
        0u64,
    );
    let flush = |cur: &mut HashMap<usize, Vec<usize>>, order: &mut Vec<usize>, p: &mut Plan| {
        for i in order.drain(..) {
            let idxs = cur.remove(&i).unwrap_or_default();
            let (t, rel) = (tocs_open[&i].clone(), files[i].rel.clone());
            let r2 = rel.clone();
            p.jobs.push(Job {
                rel,
                run: Box::new(move |_: &Ctx<'_>, _| {
                    let names = t.0.names()?;
                    let mut out = Vec::new();
                    for &c in &idxs {
                        let d = t.0.read(c, &t.1)?;
                        let nm = names.get(&c).cloned().unwrap_or_else(|| {
                            t.0.ids[c].iter().map(|b| format!("{b:02x}")).collect()
                        });
                        inline_records(&d, &format!("/{r2}#{nm}"), &mut out)?;
                    }
                    Ok(JobOut {
                        found: out,
                        var: BahsVar::new(),
                    })
                }),
            });
        }
    };
    for (i, c, raw) in items {
        if !cur.contains_key(i) {
            order.push(*i);
        }
        cur.entry(*i).or_default().push(*c);
        size += raw;
        if size as f64 >= step {
            flush(&mut cur, &mut order, p);
            size = 0;
        }
    }
    flush(&mut cur, &mut order, p);
    Ok(())
}
