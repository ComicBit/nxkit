//! The split's outputs (`main` of `shader_split.py` after the scan): keys first-seen across the results in order, the
//! package files (`BTCDBTCD` records), `shader_map.tsv` rows, the compute answer table, `shader_pairs.tsv` and
//! `pso_pairs.tsv`, all byte-exact.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::SplitError;
use crate::shaders::{CTL_MAGIC, Found, Profile, same_shader, skey};

/// One scanned unit's result: its keys (insertion order, first or last occurrence as the Python takes them) and pairs.
#[derive(Debug, Default)]
pub struct FileResult {
    /// `(key, shader)` in the order the Python dict holds them.
    pub keys: Vec<(String, Found)>,
    /// `(vertex, pixel, from a BAHS program)`.
    pub pairs: BTreeSet<(String, String, bool)>,
}

/// `uniq_keys` (files: the first occurrence of a key wins) or the dict comprehension used for `main` (`last`: the
/// last occurrence's value at the first occurrence's position).
pub fn uniq(
    p: &Profile,
    found: Vec<Found>,
    last: bool,
) -> Result<Vec<(String, Found)>, SplitError> {
    let mut pos: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<(String, Found)> = Vec::new();
    for f in found {
        let k = skey(p, &f.ctl, &f.code);
        match pos.get(&k) {
            Some(&i) => {
                if !last && !same_shader(p, (&out[i].1.ctl, &out[i].1.code), (&f.ctl, &f.code)) {
                    return Err(SplitError::Red(format!(
                        "key {k} is the same but the control section or code area differs ({})",
                        f.place
                    )));
                }
                if last {
                    out[i].1 = f;
                }
            }
            None => {
                pos.insert(k.clone(), out.len());
                out.push((k, f));
            }
        }
    }
    Ok(out)
}

/// Everything the split writes.
#[derive(Debug, Default)]
pub struct Outputs {
    /// Package file name to its bytes, sorted by name.
    pub packages: BTreeMap<String, Vec<u8>>,
    /// `shader_map.tsv` rows: (package, index, stage name, key, source).
    pub rows: Vec<(String, usize, &'static str, String, String)>,
    /// Compute answer rows: key, fact JSON.
    pub facts: BTreeMap<String, String>,
    /// `shader_pairs.tsv` pairs.
    pub pairs: BTreeSet<(String, String)>,
    /// `pso_pairs.tsv` pairs.
    pub pso_pairs: BTreeSet<(String, String)>,
    /// Shaders per stage, per source group.
    pub stats: (BTreeMap<&'static str, usize>, Vec<(String, usize)>),
}

fn stage_name(st: &str) -> &'static str {
    match st {
        "vs" => "vertex",
        "hs" => "hull",
        "ds" => "domain",
        "gs" => "geometry",
        "ps" => "pixel",
        "cs" => "compute",
        "ts" => "task",
        "ms" => "mesh",
        _ => "?",
    }
}

/// Merges the results (in the Python's order) and builds every output.
pub fn build(p: &Profile, results: &[FileResult]) -> Result<Outputs, SplitError> {
    let mut keys: HashMap<String, &Found> = HashMap::new();
    let mut per_src: Vec<(String, usize)> = Vec::new();
    let mut all_pairs: BTreeSet<(String, String, bool)> = BTreeSet::new();
    for r in results {
        all_pairs.extend(r.pairs.iter().cloned());
        for (k, f) in &r.keys {
            if let Some(old) = keys.get(k) {
                if !same_shader(p, (&f.ctl, &f.code), (&old.ctl, &old.code)) {
                    return Err(SplitError::Red(format!(
                        "key {k} is the same but the control section or code area differs ({} / {})",
                        f.place, old.place
                    )));
                }
                continue;
            }
            keys.insert(k.clone(), f);
            let g = p.group(&f.place);
            match per_src.iter_mut().find(|x| x.0 == g) {
                Some(x) => x.1 += 1,
                None => per_src.push((g, 1)),
            }
        }
    }
    let two: BTreeSet<(String, String)> = all_pairs
        .iter()
        .filter(|x| !x.2)
        .map(|x| (x.0.clone(), x.1.clone()))
        .collect();
    let prog: BTreeSet<(String, String)> = all_pairs
        .iter()
        .filter(|x| x.2)
        .map(|x| (x.0.clone(), x.1.clone()))
        .filter(|x| !two.contains(x))
        .collect();
    // separable_pairs: only for single-stage sources, which this port does not read yet
    let mut out = Outputs {
        pairs: two.clone(),
        pso_pairs: prog.difference(&two).cloned().collect(),
        ..Outputs::default()
    };

    let mut order: Vec<(String, &str, &str)> = keys
        .iter()
        .map(|(k, f)| (p.group(&f.place), f.place.as_str(), k.as_str()))
        .collect();
    order.sort();
    let mut stat: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for (g, _, k) in &order {
        let f = keys[*k];
        let name = format!("{}_{g}.bin", f.stage);
        let mut rec = b"BTCDBTCD".to_vec();
        rec.extend_from_slice(&[0u8; 0x30]);
        rec[0x28..0x2c].copy_from_slice(&(f.ctl.len() as u32).to_le_bytes());
        rec[0x2c..0x30].copy_from_slice(&(f.code.len() as u32).to_le_bytes());
        rec.extend_from_slice(&f.code);
        rec.extend_from_slice(&f.ctl);
        if crate::shaders::memfind(&rec, &CTL_MAGIC) != Some(0x38 + f.code.len()) {
            return Err(SplitError::Red(format!(
                "key {k}: the control magic appears in the code area, dmsh_build would split the record wrongly"
            )));
        }
        if crate::shaders::memfind(&rec[1..], b"BTCDBTCD").is_some() {
            return Err(SplitError::Red(format!(
                "key {k}: the record separator appears inside the record"
            )));
        }
        let pk = out.packages.entry(name.clone()).or_default();
        let c = counts.entry(name.clone()).or_default();
        let index = *c;
        *c += 1;
        pk.extend_from_slice(&rec);
        out.rows.push((
            name,
            index,
            stage_name(f.stage),
            k.to_string(),
            f.place.clone(),
        ));
        *stat.entry(f.stage).or_default() += 1;
        if f.stage == "cs" {
            let rd = |o: usize| crate::shaders::u32le(&f.ctl, o).unwrap_or(0);
            let (x, y, z, sh) = (rd(0x748), rd(0x74c), rd(0x750), rd(0x754));
            if x as u64 * y as u64 * z as u64 == 0 {
                return Err(SplitError::Red(format!(
                    "compute shader {k}: control +0x748 thread group is 0 ({})",
                    f.place
                )));
            }
            out.facts.insert(
                k.to_string(),
                format!(
                    "{{\"local\":[{x},{y},{z}],\"shared\":{}}}",
                    (sh as u64).div_ceil(16) * 16
                ),
            );
        }
    }
    // Counter.most_common: by count, ties in first-insertion order
    per_src.sort_by_key(|x| std::cmp::Reverse(x.1));
    out.stats = (stat, per_src);
    Ok(out)
}
