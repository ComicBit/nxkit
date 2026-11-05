//! The title cache (design 2.8, row Cache): shader and pipeline results kept in the store under a [`CacheKey`].
//!
//! The key names everything a result depends on: the shader key, the translator id, the emitter id, the device family
//! and the variant. An entry holds `cache.key` (the five fields as text, so an entry can be listed and filtered without
//! the code that made it) and the payload files. Size cap and least-recently-used pruning work on whole entries.

use crate::error::StoreError;
use crate::manifest::Manifest;
use crate::store::Store;
use nxb_model::{ContentKey, Digest};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::time::SystemTime;

/// The name of the file that holds the key text in every cache entry.
pub const KEY_FILE: &str = "cache.key";

/// What a cached result depends on.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CacheKey {
    /// The shader key (the content key of the guest shader, as text).
    pub shader_key: String,
    /// The translator id (name and version).
    pub translator: String,
    /// The emitter id (name and version).
    pub emitter: String,
    /// The device family, for example `apple9`.
    pub device_family: String,
    /// The variant, for example `base`, `alpha` or `mesh`.
    pub variant: String,
}

impl CacheKey {
    /// The store key: a domain tag and the five fields.
    pub fn content_key(&self) -> ContentKey {
        ContentKey::builder("nxb.cache.v1")
            .field_str("shader", &self.shader_key)
            .field_str("translator", &self.translator)
            .field_str("emitter", &self.emitter)
            .field_str("device", &self.device_family)
            .field_str("variant", &self.variant)
            .finish()
    }

    /// The text kept in `cache.key`. Fields must not hold a line break (checked by [`CacheKey::check`]).
    pub fn to_text(&self) -> String {
        format!(
            "shader={}\ntranslator={}\nemitter={}\ndevice={}\nvariant={}\n",
            self.shader_key, self.translator, self.emitter, self.device_family, self.variant
        )
    }

    /// Parse the text of `cache.key`.
    pub fn parse(text: &str) -> Option<CacheKey> {
        let mut it = text.lines();
        let mut field = |name: &str| -> Option<String> {
            it.next()?
                .strip_prefix(name)?
                .strip_prefix('=')
                .map(String::from)
        };
        let k = CacheKey {
            shader_key: field("shader")?,
            translator: field("translator")?,
            emitter: field("emitter")?,
            device_family: field("device")?,
            variant: field("variant")?,
        };
        (it.next().is_none() && k.check().is_ok()).then_some(k)
    }

    /// Every field is non-empty text on one line.
    pub fn check(&self) -> Result<(), StoreError> {
        for v in [
            &self.shader_key,
            &self.translator,
            &self.emitter,
            &self.device_family,
            &self.variant,
        ] {
            if v.is_empty() || v.contains(['\n', '\r', '\0']) {
                return Err(StoreError::BadName(format!("cache key field {v:?}")));
            }
        }
        Ok(())
    }
}

/// Which entries a listing or an export takes. `None` matches everything.
#[derive(Clone, Debug, Default)]
pub struct Filter {
    /// Only this device family.
    pub device_family: Option<String>,
    /// Only this translator id.
    pub translator: Option<String>,
    /// Only this emitter id.
    pub emitter: Option<String>,
    /// Only this variant.
    pub variant: Option<String>,
}

impl Filter {
    fn matches(&self, k: &CacheKey) -> bool {
        let ok = |want: &Option<String>, have: &String| want.as_ref().is_none_or(|w| w == have);
        ok(&self.device_family, &k.device_family)
            && ok(&self.translator, &k.translator)
            && ok(&self.emitter, &k.emitter)
            && ok(&self.variant, &k.variant)
    }
}

/// One cache entry as listed.
#[derive(Clone, Debug)]
pub struct CacheEntry {
    /// The key fields.
    pub key: CacheKey,
    /// The manifest.
    pub manifest: Manifest,
    /// When the entry was last used (manifest time).
    pub used: SystemTime,
}

impl CacheEntry {
    /// The payload size in bytes (every file of the entry).
    pub fn bytes(&self) -> u64 {
        self.manifest.files.iter().map(|f| f.digest.len).sum()
    }
}

/// What [`Store::prune`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PruneReport {
    /// Entries removed.
    pub entries: u64,
    /// Blobs removed.
    pub blobs: u64,
    /// Bytes of removed blobs.
    pub bytes: u64,
    /// Bytes left in blobs.
    pub kept_bytes: u64,
}

impl Store {
    /// Publish a cache result: `cache.key` plus the payload files (names are validated, `cache.key` is reserved).
    pub fn put_cache(
        &self,
        key: &CacheKey,
        files: &[(&str, &[u8])],
    ) -> Result<Manifest, StoreError> {
        key.check()?;
        let mut w = self.entry(key.content_key());
        w.add_bytes(KEY_FILE, key.to_text().as_bytes())?;
        for (name, bytes) in files {
            w.add_bytes(name, bytes)?;
        }
        w.commit()
    }

    /// Find a cache result. A hit marks the entry as used now.
    pub fn get_cache(&self, key: &CacheKey) -> Result<Option<Manifest>, StoreError> {
        let k = key.content_key();
        let m = self.lookup(&k)?;
        if m.is_some() {
            self.touch(&k);
        }
        Ok(m)
    }

    /// Every entry key in the store (damaged names are skipped).
    pub fn entry_keys(&self) -> Result<Vec<ContentKey>, StoreError> {
        let dir = self.root().join("entries");
        let mut keys = Vec::new();
        let io = |e| StoreError::Io {
            op: "list entries",
            path: dir.clone(),
            source: e,
        };
        for shard in fs::read_dir(&dir).map_err(io)? {
            let shard = shard.map_err(io)?.path();
            if !shard.is_dir() {
                continue;
            }
            for f in fs::read_dir(&shard).map_err(io)?.flatten() {
                let name = f.file_name();
                let hex = name.to_string_lossy();
                if let Some(k) = hex
                    .strip_suffix(".manifest")
                    .and_then(|h| ContentKey::from_hex(h).ok())
                {
                    keys.push(k);
                }
            }
        }
        keys.sort();
        Ok(keys)
    }

    /// The cache entries that pass the filter, oldest use first. An entry whose key file does not parse or does not
    /// give the entry's own key is not a cache entry and is left out.
    pub fn list_cache(&self, filter: &Filter) -> Result<Vec<CacheEntry>, StoreError> {
        let mut out = Vec::new();
        for ck in self.entry_keys()? {
            let Some(manifest) = self.lookup(&ck)? else {
                continue;
            };
            let Some(f) = manifest.files.iter().find(|f| f.name == KEY_FILE) else {
                continue;
            };
            let text = self.read_blob(&f.digest.sha256)?;
            let Some(key) = std::str::from_utf8(&text).ok().and_then(CacheKey::parse) else {
                continue;
            };
            if key.content_key() != ck || !filter.matches(&key) {
                continue;
            }
            let used = fs::metadata(self.entry_path(&ck))
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            out.push(CacheEntry {
                key,
                manifest,
                used,
            });
        }
        out.sort_by(|a, b| a.used.cmp(&b.used).then_with(|| a.key.cmp(&b.key)));
        Ok(out)
    }

    /// Keep the blob bytes of the store at or under `cap` bytes: remove the least recently used entries (any entry,
    /// cache or not) until the unique blobs fit, then remove the blobs no entry names. Needs [`Store::open_exclusive`].
    pub fn prune(&self, cap: u64) -> Result<PruneReport, StoreError> {
        if !self.is_exclusive() {
            return Err(StoreError::Locked(self.root().to_path_buf()));
        }
        let mut entries: Vec<(SystemTime, ContentKey, Manifest)> = Vec::new();
        for k in self.entry_keys()? {
            if let Some(m) = self.lookup(&k)? {
                let t = fs::metadata(self.entry_path(&k))
                    .and_then(|m| m.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                entries.push((t, k, m));
            }
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        // reference counts of blobs over the live entries
        let mut refs: BTreeMap<Digest, (u64, u64)> = BTreeMap::new();
        for (_, _, m) in &entries {
            for f in &m.files {
                refs.entry(f.digest.sha256).or_insert((f.digest.len, 0)).1 += 1;
            }
        }
        let mut total: u64 = refs.values().map(|v| v.0).sum();
        let mut rep = PruneReport::default();
        for (_, k, m) in &entries {
            if total <= cap {
                break;
            }
            fs::remove_file(self.entry_path(k)).map_err(|e| StoreError::Io {
                op: "remove entry",
                path: self.entry_path(k),
                source: e,
            })?;
            rep.entries += 1;
            for f in &m.files {
                if let Some(r) = refs.get_mut(&f.digest.sha256) {
                    r.1 -= 1;
                    if r.1 == 0 {
                        total -= r.0;
                    }
                }
            }
        }
        // sweep: every blob file that no remaining entry names
        let live: BTreeSet<Digest> = refs
            .iter()
            .filter(|(_, v)| v.1 > 0)
            .map(|(d, _)| *d)
            .collect();
        let dir = self.root().join("blobs");
        let io = |e| StoreError::Io {
            op: "list blobs",
            path: dir.clone(),
            source: e,
        };
        for shard in fs::read_dir(&dir).map_err(io)? {
            let shard = shard.map_err(io)?.path();
            if !shard.is_dir() {
                continue;
            }
            for f in fs::read_dir(&shard).map_err(io)?.flatten() {
                let name = f.file_name();
                let Ok(d) = Digest::from_hex(&name.to_string_lossy()) else {
                    continue;
                };
                let len = f.metadata().map(|m| m.len()).unwrap_or(0);
                if live.contains(&d) {
                    rep.kept_bytes += len;
                } else if fs::remove_file(f.path()).is_ok() {
                    rep.blobs += 1;
                    rep.bytes += len;
                }
            }
        }
        Ok(rep)
    }

    #[cfg(test)]
    pub(crate) fn entry_path_for_test(&self, key: &CacheKey) -> std::path::PathBuf {
        self.entry_path(&key.content_key())
    }

    /// Mark an entry as used now (its manifest time), for the LRU order. Errors are ignored: a read-only store still works.
    pub(crate) fn touch(&self, key: &ContentKey) {
        if let Ok(f) = fs::OpenOptions::new()
            .write(true)
            .open(self.entry_path(key))
        {
            let _ = f.set_modified(SystemTime::now());
        }
    }
}
