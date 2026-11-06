//! The shader step's inputs from the split: `shader_map.tsv` (where each key's record is), the answer table
//! `shader_facts.tsv`, and the package files of `BTCDBTCD` records (`dmsh_build.record_index`, `rec_of`).

use nxb_pyjson::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

/// An open package and its record offsets.
type Pack = (File, Vec<u64>);

/// The keys of a profile list: the lines that are not empty or comments, each reduced by `key`. A missing file is empty.
fn key_list(dir: &Path, name: &str, key: fn(&str) -> Option<&str>) -> BTreeSet<String> {
    let text = fs::read_to_string(dir.join(name)).unwrap_or_default();
    keys_of(&text, key)
}

fn keys_of(text: &str, key: fn(&str) -> Option<&str>) -> BTreeSet<String> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .filter_map(key)
        .map(str::to_string)
        .collect()
}

/// Where every key's record lies, and its answer-table facts.
#[derive(Debug, Default)]
pub struct ShaderInputs {
    /// key → (package file name, record index): the first row of each key (`loc.setdefault`).
    pub loc: BTreeMap<String, (String, usize)>,
    /// key → facts object.
    pub facts: BTreeMap<String, Value>,
    /// key → the map's column 5 (`pc_of`), overridden by the conflicts table.
    pub pc_of: BTreeMap<String, String>,
    /// The pixel shaders a pipeline blends with their output 1 as the second source (`dual_want.tsv`, first column).
    pub dual_want: BTreeSet<String>,
    /// The pixel shaders that get a fetch library without a rewritable sample (`bf_want.tsv`, one key per line).
    pub bf_want: BTreeSet<String>,
    pkg_dir: PathBuf,
    packs: Mutex<BTreeMap<String, std::sync::Arc<Pack>>>,
}

impl ShaderInputs {
    /// Load the profile tables (`profile_dir` holds `shader_map.tsv` and `shader_facts.tsv`) for the package folder.
    pub fn load(pkg_dir: &Path, profile_dir: &Path) -> io::Result<ShaderInputs> {
        let mut loc = BTreeMap::new();
        let mut pc_of = BTreeMap::new();
        for ln in fs::read_to_string(profile_dir.join("shader_map.tsv"))?.lines() {
            let c: Vec<&str> = ln.split('\t').collect();
            if c.len() > 4
                && c[0].ends_with(".bin")
                && let Ok(i) = c[1].parse()
            {
                pc_of.entry(c[3].to_string()).or_insert(c[4].to_string());
                loc.entry(c[3].to_string()).or_insert((c[0].to_string(), i));
            }
        }
        if let Ok(t) = fs::read_to_string(profile_dir.join("shader_map.tsv.conflicts.tsv")) {
            for ln in t.lines() {
                let c: Vec<&str> = ln.split('\t').collect();
                if c.len() > 1
                    && !ln.starts_with('#')
                    && !ln.starts_with("dm_")
                    && pc_of.contains_key(c[0])
                {
                    pc_of.insert(c[0].to_string(), c[1].to_string());
                }
            }
        }
        let mut facts = BTreeMap::new();
        if let Ok(t) = fs::read_to_string(profile_dir.join("shader_facts.tsv")) {
            for ln in t.lines() {
                if ln.trim().is_empty() || ln.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = ln.split_once('\t')
                    && let Ok(j) = nxb_pyjson::parse(v)
                {
                    facts.insert(k.to_string(), j);
                }
            }
        }
        Ok(ShaderInputs {
            loc,
            facts,
            pc_of,
            dual_want: key_list(profile_dir, "dual_want.tsv", |l| {
                l.split_whitespace().next()
            }),
            bf_want: key_list(profile_dir, "bf_want.tsv", |l| Some(l.trim())),
            pkg_dir: pkg_dir.to_path_buf(),
            packs: Mutex::new(BTreeMap::new()),
        })
    }

    fn index(&self, name: &str) -> io::Result<std::sync::Arc<Pack>> {
        let mut g = self.packs.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(p) = g.get(name) {
            return Ok(p.clone());
        }
        let path = self.pkg_dir.join(name);
        let data = fs::read(&path)?;
        let mut offs = Vec::new();
        let mut i = 0;
        const M: &[u8] = b"BTCDBTCD";
        while let Some(k) = data[i..].windows(M.len()).position(|w| w == M) {
            offs.push((i + k) as u64);
            i += k + M.len();
        }
        offs.push(data.len() as u64);
        let p = std::sync::Arc::new((File::open(&path)?, offs));
        g.insert(name.to_string(), p.clone());
        Ok(p)
    }

    /// `rec_of(key)`: (package name, the record's bytes).
    pub fn record(&self, key: &str) -> io::Result<(String, Vec<u8>)> {
        let (name, k) = self.loc.get(key).ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, format!("{key} is not in the map"))
        })?;
        let p = self.index(name)?;
        let (a, b) = (
            p.1.get(*k).copied().unwrap_or(0),
            p.1.get(k + 1).copied().unwrap_or(0),
        );
        let mut buf = vec![0u8; usize::try_from(b.saturating_sub(a)).unwrap_or(0)];
        read_exact_at(&p.0, &mut buf, a)?;
        Ok((name.clone(), buf))
    }
}

#[cfg(unix)]
fn read_exact_at(f: &File, buf: &mut [u8], off: u64) -> io::Result<()> {
    std::os::unix::fs::FileExt::read_exact_at(f, buf, off)
}

#[cfg(not(unix))]
fn read_exact_at(f: &File, buf: &mut [u8], off: u64) -> io::Result<()> {
    std::os::windows::fs::FileExt::seek_read(f, buf, off).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_want_list_has_one_key_per_line() {
        let text = "# generated\n\n0123456789abcdef\tnote\n  fedcba9876543210  \n";
        let dual = keys_of(text, |l| l.split_whitespace().next());
        assert_eq!(
            dual.iter().map(String::as_str).collect::<Vec<_>>(),
            ["0123456789abcdef", "fedcba9876543210"]
        );
        let bf = keys_of(text, |l| Some(l.trim()));
        assert!(bf.contains("fedcba9876543210"));
        assert!(!bf.contains("0123456789abcdef"));
    }
}
