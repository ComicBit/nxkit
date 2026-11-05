//! Manifest-first ordering and the partial state (design 2.8 and 2.12).
//!
//! A use manifest is the ordered list of shader keys a title actually used (recorded at run time, or published by the
//! title's authors). Work in that order runs first. A build that stops after the manifest keys is *partial*: it leaves a small
//! marker file next to its outputs and says so in its summaries, so a host knows the other shaders still compile at run time.
//! Without a manifest nothing here changes an order or an output.

use std::collections::BTreeMap;
use std::path::Path;

/// Name of the marker file in a partial library folder (copied into the app with the other `*.json` files).
pub const PARTIAL_FILE: &str = "_b_partial.json";

/// An ordered list of distinct keys.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UseManifest {
    keys: Vec<String>,
}

impl UseManifest {
    /// Parse text: one key per line (the first column when there are more), `#` starts a comment, blank lines are
    /// skipped, repeated keys keep their first place. A key is non-empty ASCII without control characters.
    pub fn parse(text: &str) -> Result<UseManifest, String> {
        let mut keys: Vec<String> = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("");
            let Some(key) = line.split_whitespace().next() else {
                continue;
            };
            if !key.is_ascii() || key.chars().any(char::is_control) {
                return Err(format!("manifest line {}: {key:?} is not a key", n + 1));
            }
            if seen.insert(key.to_string()) {
                keys.push(key.to_string());
            }
        }
        Ok(UseManifest { keys })
    }

    /// From keys already in order, repeated keys keep their first place.
    pub fn from_keys(keys: impl IntoIterator<Item = String>) -> UseManifest {
        let mut seen = std::collections::BTreeSet::new();
        UseManifest {
            keys: keys
                .into_iter()
                .filter(|k| seen.insert(k.clone()))
                .collect(),
        }
    }

    /// Read and parse a file.
    pub fn read(path: &Path) -> Result<UseManifest, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The keys in manifest order.
    pub fn keys(&self) -> &[String] {
        &self.keys
    }

    /// No keys.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Put the manifest's keys first, in manifest order. The other items keep their relative order. With an empty
    /// manifest the slice is not touched.
    pub fn reorder<T: AsRef<str>>(&self, items: &mut [T]) {
        if self.keys.is_empty() {
            return;
        }
        let pos: BTreeMap<&str, usize> = self
            .keys
            .iter()
            .enumerate()
            .map(|(i, k)| (k.as_str(), i))
            .collect();
        items.sort_by_key(|k| pos.get(k.as_ref()).copied().unwrap_or(usize::MAX));
    }

    /// The manifest keys that `known` accepts, in manifest order.
    pub fn known(&self, known: impl Fn(&str) -> bool) -> Vec<String> {
        self.keys.iter().filter(|k| known(k)).cloned().collect()
    }
}

/// How much of a library folder a partial build made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Partial {
    /// Shaders built in this run.
    pub built: u64,
    /// Shaders of the title.
    pub total: u64,
}

impl Partial {
    /// The marker's text: `{"partial":true,"built":N,"total":M}` and a line break.
    pub fn to_json(&self) -> String {
        format!(
            "{{\"partial\":true,\"built\":{},\"total\":{}}}\n",
            self.built, self.total
        )
    }

    /// Read the marker's text back.
    pub fn parse(text: &str) -> Option<Partial> {
        let body = text
            .trim()
            .strip_prefix("{\"partial\":true,\"built\":")?
            .strip_suffix('}')?;
        let (built, total) = body.split_once(",\"total\":")?;
        Some(Partial {
            built: built.parse().ok()?,
            total: total.parse().ok()?,
        })
    }

    /// Read the marker of a library folder, if it has one.
    pub fn read(dir: &Path) -> Option<Partial> {
        Self::parse(&std::fs::read_to_string(dir.join(PARTIAL_FILE)).ok()?)
    }

    /// The words for a stage summary.
    pub fn note(&self) -> String {
        format!(
            "partial prebuilt state: {} of {} shaders",
            self.built, self.total
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_order_and_dedupe() {
        let m = UseManifest::parse("# used\nbb\naa\t12\n\nbb # again\ncc\n").unwrap();
        assert_eq!(m.keys(), ["bb", "aa", "cc"]);
        assert!(UseManifest::parse("a\u{e9}\n").is_err());
        assert!(UseManifest::parse("\n# nothing\n").unwrap().is_empty());
    }

    #[test]
    fn manifest_first_and_stable_rest() {
        let m = UseManifest::parse("cc\nzz\naa\n").unwrap();
        let mut v: Vec<String> = ["dd", "aa", "bb", "cc", "ee"].map(String::from).to_vec();
        m.reorder(&mut v);
        assert_eq!(v, ["cc", "aa", "dd", "bb", "ee"]);
        // no manifest: untouched
        let mut w = vec!["b", "a"];
        UseManifest::default().reorder(&mut w);
        assert_eq!(w, ["b", "a"]);
        assert_eq!(m.known(|k| k != "zz"), ["cc", "aa"]);
    }

    #[test]
    fn marker_round_trip() {
        let p = Partial {
            built: 708,
            total: 15767,
        };
        assert_eq!(Partial::parse(&p.to_json()), Some(p));
        assert_eq!(Partial::parse("{}"), None);
        assert!(p.note().contains("708 of 15767"));
    }
}
