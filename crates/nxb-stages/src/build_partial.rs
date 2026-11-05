//! Manifest-first ordering and the partial state of the shader step (design 2.12): the small hooks `build` calls.
//! With an empty manifest and `partial` false every function here leaves the build exactly as it was.

use crate::build::BuildConfig;
use crate::build::fatal;
use nxb_error::Fatal;
use nxb_sched::{PARTIAL_FILE, Partial, UseManifest};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Before the work: a full build removes an old partial marker, a partial build keeps only the manifest's shaders of this
/// title as its keys. Returns the number of shaders of the title.
pub(crate) fn prepare<V>(
    cfg: &mut BuildConfig,
    out: &Path,
    loc: &BTreeMap<String, V>,
) -> Result<u64, Fatal> {
    if cfg.partial {
        cfg.keys = UseManifest::from_keys(cfg.manifest.clone()).known(|k| loc.contains_key(k));
        if cfg.keys.is_empty() {
            return Err(fatal(
                "partial build",
                "the manifest names no shader of this title",
            ));
        }
    } else if cfg.keys.is_empty() {
        let _ = fs::remove_file(out.join(PARTIAL_FILE));
    }
    Ok(loc.len() as u64)
}

/// The order of translation: a consumer's order first (the archive's pair groups), then the manifest's keys before all
/// others. Every part is a stable sort, so what neither names keeps its order.
pub(crate) fn sort(cfg: &BuildConfig, order: &mut [&String]) {
    if !cfg.order.is_empty() {
        let pos: BTreeMap<&String, usize> =
            cfg.order.iter().enumerate().map(|(i, k)| (k, i)).collect();
        order.sort_by_key(|k| pos.get(k).copied().unwrap_or(usize::MAX));
    }
    UseManifest::from_keys(cfg.manifest.clone()).reorder(order);
}

/// After the work: a partial build writes its marker next to the outputs.
pub(crate) fn mark(
    cfg: &BuildConfig,
    out: &Path,
    results: &[(String, String, String)],
    total: u64,
) -> Result<(), Fatal> {
    if !cfg.partial {
        return Ok(());
    }
    let built = results.iter().filter(|r| r.1 == "ok").count() as u64;
    fs::write(out.join(PARTIAL_FILE), Partial { built, total }.to_json())
        .map_err(|e| fatal(PARTIAL_FILE, e))
}
