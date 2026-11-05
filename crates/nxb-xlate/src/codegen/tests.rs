use super::generator::generate;
use super::props::Props;
use super::testutil::*;
use crate::ir::{Ir, text};
use crate::structured::program::{FunctionIn, make_structured_program};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

/// Reference `.spv` of every key (the retry job's output wins where a key has two).
fn ref_paths() -> Option<HashMap<String, PathBuf>> {
    let dir = PathBuf::from(std::env::var_os("NXB_XLATE_REF")?);
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    let mut map = HashMap::new();
    for d in dirs {
        for f in std::fs::read_dir(&d).ok()?.flatten() {
            let p = f.path();
            if p.extension().is_some_and(|e| e == "spv")
                && let Some(stem) = p.file_stem().and_then(|s| s.to_str())
            {
                // job1_* sorts before job_*: keep the first found only for job1_ (retry) directories
                let retry = d
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("job1_"));
                if retry || !map.contains_key(stem) {
                    map.insert(stem.to_string(), p);
                }
            }
        }
    }
    Some(map)
}

fn translate(e: &crate::corpus::Entry) -> Result<Vec<u8>, String> {
    translate_attempt(e, usize::MAX)
}

/// The back end on the n-th attempt of the key's dumps (the last when n is out of range).
fn translate_attempt(e: &crate::corpus::Entry, n: usize) -> Result<Vec<u8>, String> {
    let key = &e.job.key;
    let get = |p: &str| {
        let all = attempts(&e.dump_dir, key, p);
        all.get(n.min(all.len().saturating_sub(1)))
            .cloned()
            .ok_or_else(|| format!("no {p} dump"))
    };
    let props = Props::parse(&get("props")?)?;
    let mut ir = Ir::default();
    let mut funcs = text::parse(&mut ir, &get("sfunc")?)?;
    apply_use_order(&mut ir, &funcs, &get("suses")?)?;
    let meta = get("sfuncmeta")?;
    let metas: Vec<Vec<&str>> = meta
        .lines()
        .skip(1)
        .map(|l| l.split(' ').collect())
        .collect();
    let mut ins: Vec<FunctionIn> = funcs
        .iter_mut()
        .map(|f| FunctionIn {
            name: metas[f.index][1].to_string(),
            blocks: &mut f.blocks,
            in_arguments_count: metas[f.index][3].parse().unwrap_or(0),
            out_arguments_count: metas[f.index][4].parse().unwrap_or(0),
            returns_value: metas[f.index][2] == "1",
        })
        .collect();
    let mut res = ReplayResources {
        map: props.cb_slot_to_binding.clone(),
    };
    let info = make_structured_program(
        &mut ir,
        &mut ins,
        &props.definitions,
        &mut res,
        props.passthrough_attributes,
        true,
    )?;
    generate(info, props)
}

/// SPIR-V of every key from the structured program and the dumped properties, against the reference.
#[test]
fn spirv_matches_reference() {
    let (Some(entries), Some(refs)) = (crate::corpus::load(), ref_paths()) else {
        eprintln!("skipped: corpus env missing");
        return;
    };
    let only = std::env::var("NXB_ONLY").ok();
    let mut seen = std::collections::HashSet::new();
    let (mut ok, mut bad, mut total) = (0, 0, 0usize);
    let mut errs: BTreeMap<String, usize> = BTreeMap::new();
    let mut shown = 0;
    for e in &entries {
        let key = &e.job.key;
        if !seen.insert(key.clone()) || only.as_ref().is_some_and(|o| o != key) {
            continue;
        }
        let Some(rp) = refs.get(key) else { continue };
        let want = std::fs::read(rp).unwrap();
        match translate(e) {
            Ok(got) if got == want => {
                ok += 1;
                total += got.len();
            }
            Ok(got) => {
                bad += 1;
                *errs.entry("differs".into()).or_default() += 1;
                if shown < 3 {
                    shown += 1;
                    let gw: Vec<u32> = got
                        .chunks(4)
                        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                        .collect();
                    let ww: Vec<u32> = want
                        .chunks(4)
                        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                        .collect();
                    let pos = gw.iter().zip(&ww).position(|(a, b)| a != b);
                    eprintln!(
                        "{key}: len {} vs {} words, first diff at word {pos:?}",
                        gw.len(),
                        ww.len()
                    );
                }
            }
            Err(err) => {
                bad += 1;
                *errs.entry(err.chars().take(90).collect()).or_default() += 1;
            }
        }
    }
    eprintln!(
        "spirv identical: {ok} of {} (bad {bad}), {total} bytes",
        ok + bad
    );
    for (k, v) in &errs {
        eprintln!("  {v:4} {k}");
    }
    assert_eq!(bad, 0);
}

/// The 15 keys translated twice (the retry job lowers FP64): the first job's output is one of the earlier attempts.
#[test]
fn spirv_first_job_of_retried_keys() {
    let (Some(entries), Some(dir)) = (crate::corpus::load(), std::env::var_os("NXB_XLATE_REF"))
    else {
        eprintln!("skipped: corpus env missing");
        return;
    };
    let dir = PathBuf::from(dir);
    let mut retried = std::collections::HashSet::new();
    let mut seen = std::collections::HashSet::new();
    for e in &entries {
        if !seen.insert(e.job.key.clone()) {
            retried.insert(e.job.key.clone());
        }
    }
    let (mut ok, mut bad) = (0, 0);
    for key in &retried {
        let e = entries.iter().find(|e| &e.job.key == key).unwrap();
        let mut want = None;
        for d in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = d.path().join(format!("{key}.spv"));
            if d.file_name().to_string_lossy().starts_with("job_") && p.exists() {
                want = Some(std::fs::read(p).unwrap());
            }
        }
        let want = want.unwrap();
        let n = attempts(&e.dump_dir, key, "props").len();
        if (0..n).any(|i| translate_attempt(e, i).is_ok_and(|g| g == want)) {
            ok += 1;
        } else {
            bad += 1;
        }
    }
    eprintln!(
        "first-job outputs of retried keys: {ok} of {} identical",
        ok + bad
    );
    assert_eq!(bad, 0);
}
