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
    /// The keys of the profile's map that the split did not find. They have no record.
    pub unsplit: BTreeSet<String>,
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

/// The rows of a `shader_map.tsv`: (package file name, record index, key, column 5).
fn map_rows(text: &str) -> impl Iterator<Item = (&str, usize, &str, &str)> {
    text.lines().filter_map(|ln| {
        let c: Vec<&str> = ln.split('\t').collect();
        if c.len() > 4 && c[0].ends_with(".bin") {
            c[1].parse().ok().map(|i| (c[0], i, c[3], c[4]))
        } else {
            None
        }
    })
}

impl ShaderInputs {
    /// Load the profile tables (`profile_dir` holds `shader_map.tsv` and `shader_facts.tsv`) for the package folder.
    ///
    /// The record of a key is found through the `shader_map.tsv` beside `pkg_dir` when the split wrote one: the
    /// package files and that map come from the same split, while the profile's map can come from another split
    /// whose packages hold other records at the same index. The profile keys that map lacks are in [`Self::unsplit`].
    pub fn load(pkg_dir: &Path, profile_dir: &Path) -> io::Result<ShaderInputs> {
        let mut loc = BTreeMap::new();
        let mut pc_of = BTreeMap::new();
        let profile_map = fs::read_to_string(profile_dir.join("shader_map.tsv"))?;
        for (pk, i, key, pc) in map_rows(&profile_map) {
            pc_of.entry(key.to_string()).or_insert(pc.to_string());
            loc.entry(key.to_string()).or_insert((pk.to_string(), i));
        }
        let mut unsplit = BTreeSet::new();
        let split_map = pkg_dir.parent().map(|d| d.join("shader_map.tsv"));
        if let Some(text) = split_map.and_then(|p| fs::read_to_string(p).ok()) {
            let mut own = BTreeMap::new();
            for (pk, i, key, _) in map_rows(&text) {
                own.entry(key.to_string()).or_insert((pk.to_string(), i));
            }
            unsplit = loc
                .keys()
                .filter(|k| !own.contains_key(*k))
                .cloned()
                .collect();
            loc = own;
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
            unsplit,
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
            let why = if self.unsplit.contains(key) {
                "the split did not find it"
            } else {
                "it is not in the map"
            };
            io::Error::new(io::ErrorKind::NotFound, format!("{key}: {why}"))
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
    fn records_are_found_through_the_splits_own_map() {
        let d = std::env::temp_dir().join(format!("nxb-inputs-{}", std::process::id()));
        let (prof, pkg) = (d.join("profile"), d.join("shader/pkg"));
        fs::create_dir_all(&prof).unwrap();
        fs::create_dir_all(&pkg).unwrap();
        let head = "package\tindex\tstage\tdm_ctl_sha16\tsource\n";
        let profile = format!("{head}ps_a.bin\t0\tpixel\taaaa\tx\nps_a.bin\t1\tpixel\tbbbb\ty\n");
        fs::write(prof.join("shader_map.tsv"), profile).unwrap();
        fs::write(
            d.join("shader/shader_map.tsv"),
            format!("{head}ps_a.bin\t0\tpixel\tbbbb\ty\n"),
        )
        .unwrap();
        fs::write(pkg.join("ps_a.bin"), b"BTCDBTCDrecord-of-bbbb").unwrap();
        let s = ShaderInputs::load(&pkg, &prof).unwrap();
        assert_eq!(s.loc.get("bbbb"), Some(&("ps_a.bin".to_string(), 0)));
        assert_eq!(s.record("bbbb").unwrap().1, b"BTCDBTCDrecord-of-bbbb");
        assert!(s.unsplit.contains("aaaa"));
        let e = s.record("aaaa").unwrap_err().to_string();
        assert!(e.contains("the split did not find it"), "{e}");
        assert_eq!(s.pc_of.get("aaaa").map(String::as_str), Some("x"));
        fs::remove_dir_all(&d).unwrap();
    }

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
