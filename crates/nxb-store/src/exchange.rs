//! Store export: a title's cache entries in one file, content-addressed and checked on import.
//!
//! ```text
//! nxb-cache-export 1
//! entries <n>
//! entry <key hex> <nfiles>            sorted by key, each followed by its file lines
//! file <sha256 hex> <len> <name>
//! blobs <m>
//! blob <sha256 hex> <len>             unique digests, sorted
//! data
//! <the blob bytes in the order listed>
//! end <sha256 hex of every byte before this line>
//! ```
//!
//! The same entries give the same bytes. Import re-hashes every blob against its listed digest, checks the trailer over
//! the whole file and checks that each entry's key is the key its `cache.key` file names. Entries are committed only
//! after every check passed, so a damaged file adds no entry (blobs it brought stay, unreferenced, until a prune).

use crate::cache::{CacheKey, Filter, KEY_FILE};
use crate::error::StoreError;
use crate::store::Store;
use nxb_model::{ContentKey, Digest, FileDigest, HashingWriter, Sha256};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};

const MAGIC: &str = "nxb-cache-export 1";
const MAX_LINE: u64 = 4096;
const MAX_ITEMS: usize = 10_000_000;

/// What an export wrote.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExportReport {
    /// Entries.
    pub entries: u64,
    /// Unique blobs.
    pub blobs: u64,
    /// Bytes of blob data.
    pub bytes: u64,
}

/// What an import did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportReport {
    /// Entries committed (new in this store).
    pub added: u64,
    /// Entries the store had already.
    pub present: u64,
    /// Blobs read.
    pub blobs: u64,
    /// Bytes of blob data read.
    pub bytes: u64,
}

fn bad(s: impl Into<String>) -> StoreError {
    StoreError::BadExport(s.into())
}

fn wio(e: std::io::Error) -> StoreError {
    StoreError::Io {
        op: "write export",
        path: "<export>".into(),
        source: e,
    }
}

impl Store {
    /// Write the cache entries that pass `filter` to `w`.
    pub fn export_cache(&self, filter: &Filter, w: impl Write) -> Result<ExportReport, StoreError> {
        let list = self.list_cache(filter)?;
        let mut by_key: BTreeMap<ContentKey, &crate::Manifest> = BTreeMap::new();
        let mut blobs: BTreeMap<Digest, u64> = BTreeMap::new();
        for e in &list {
            by_key.insert(e.manifest.key, &e.manifest);
            for f in &e.manifest.files {
                blobs.insert(f.digest.sha256, f.digest.len);
            }
        }
        let mut h = format!("{MAGIC}\nentries {}\n", by_key.len());
        for (k, m) in &by_key {
            h.push_str(&format!("entry {} {}\n", k.to_hex(), m.files.len()));
            for f in &m.files {
                h.push_str(&format!(
                    "file {} {} {}\n",
                    f.digest.sha256, f.digest.len, f.name
                ));
            }
        }
        h.push_str(&format!("blobs {}\n", blobs.len()));
        for (d, len) in &blobs {
            h.push_str(&format!("blob {d} {len}\n"));
        }
        h.push_str("data\n");
        let mut out = HashingWriter::new(w);
        out.write_all(h.as_bytes()).map_err(wio)?;
        let mut rep = ExportReport {
            entries: by_key.len() as u64,
            blobs: blobs.len() as u64,
            bytes: 0,
        };
        let mut buf = vec![0u8; 256 * 1024];
        for (d, len) in &blobs {
            let path = self.blob_path(d);
            let mut f = File::open(&path).map_err(|e| StoreError::Io {
                op: "open blob",
                path: path.clone(),
                source: e,
            })?;
            let mut hasher = Sha256::new();
            loop {
                let n = f.read(&mut buf).map_err(|e| StoreError::Io {
                    op: "read blob",
                    path: path.clone(),
                    source: e,
                })?;
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
                out.write_all(&buf[..n]).map_err(wio)?;
            }
            if hasher.len() != *len || hasher.finalize() != *d {
                return Err(bad(format!(
                    "blob {d} in the store is damaged, run a verify"
                )));
            }
            rep.bytes += len;
        }
        let (mut w, digest) = out.finish();
        writeln!(w, "end {}", digest.sha256).map_err(wio)?;
        w.flush().map_err(wio)?;
        Ok(rep)
    }

    /// Read an export into this store. Any check that fails returns an error and commits no entry.
    pub fn import_cache(&self, r: impl Read) -> Result<ImportReport, StoreError> {
        let mut rd = Rd {
            r: BufReader::new(r),
            h: Sha256::new(),
        };
        if rd.line()? != MAGIC {
            return Err(bad("not a cache export (header)"));
        }
        let n_entries = rd.count("entries")?;
        let mut entries: Vec<(ContentKey, Vec<(String, FileDigest)>)> = Vec::new();
        for _ in 0..n_entries {
            let l = rd.line()?;
            let mut p = l
                .strip_prefix("entry ")
                .ok_or_else(|| bad("entry line expected"))?
                .split(' ');
            let key = ContentKey::from_hex(p.next().unwrap_or("")).map_err(|_| bad("entry key"))?;
            let nf: usize = p
                .next()
                .and_then(|v| v.parse().ok())
                .filter(|n| *n <= MAX_ITEMS)
                .ok_or_else(|| bad("entry file count"))?;
            let mut files = Vec::new();
            for _ in 0..nf {
                let l = rd.line()?;
                let rest = l
                    .strip_prefix("file ")
                    .ok_or_else(|| bad("file line expected"))?;
                let (hex, rest) = rest.split_once(' ').ok_or_else(|| bad("file line"))?;
                let (len, name) = rest.split_once(' ').ok_or_else(|| bad("file line"))?;
                crate::validate_name(name)?;
                let digest = FileDigest {
                    len: len.parse().map_err(|_| bad("file length"))?,
                    sha256: Digest::from_hex(hex).map_err(|_| bad("file digest"))?,
                };
                files.push((name.to_string(), digest));
            }
            entries.push((key, files));
        }
        let n_blobs = rd.count("blobs")?;
        let mut blobs: Vec<FileDigest> = Vec::new();
        for _ in 0..n_blobs {
            let l = rd.line()?;
            let (hex, len) = l
                .strip_prefix("blob ")
                .and_then(|r| r.split_once(' '))
                .ok_or_else(|| bad("blob line"))?;
            blobs.push(FileDigest {
                len: len.parse().map_err(|_| bad("blob length"))?,
                sha256: Digest::from_hex(hex).map_err(|_| bad("blob digest"))?,
            });
        }
        if rd.line()? != "data" {
            return Err(bad("data marker expected"));
        }
        // every file of every entry must be a listed blob of the same size
        for (_, files) in &entries {
            for (name, d) in files {
                if !blobs.contains(d) {
                    return Err(bad(format!("file {name} names a blob that is not listed")));
                }
            }
        }
        let mut rep = ImportReport {
            blobs: blobs.len() as u64,
            ..Default::default()
        };
        for b in &blobs {
            let got = self.put_reader(Tee { rd: &mut rd }.take(b.len))?;
            if got != *b {
                return Err(bad(format!(
                    "blob {} does not match its digest or is cut short",
                    b.sha256
                )));
            }
            rep.bytes += b.len;
        }
        let want = rd.line_unhashed()?;
        let sum = rd.h.finalize();
        if want != format!("end {sum}") || !rd.r.fill_buf().map_err(wio)?.is_empty() {
            return Err(bad("trailer checksum does not match or data follows it"));
        }
        // the key of each entry must be the one its key file names
        for (key, files) in &entries {
            let (_, kd) = files
                .iter()
                .find(|(n, _)| n == KEY_FILE)
                .ok_or_else(|| bad("entry without cache.key"))?;
            let text = self.read_blob(&kd.sha256)?;
            let ck = std::str::from_utf8(&text)
                .ok()
                .and_then(CacheKey::parse)
                .ok_or_else(|| bad("cache.key does not parse"))?;
            if ck.content_key() != *key {
                return Err(bad(format!("entry {key} does not match its cache.key")));
            }
        }
        for (key, files) in entries {
            if self.lookup(&key)?.is_some() {
                rep.present += 1;
                continue;
            }
            let mut w = self.entry(key);
            for (name, d) in files {
                w.add(&name, d)?;
            }
            w.commit()?;
            rep.added += 1;
        }
        Ok(rep)
    }
}

/// A buffered reader that hashes every byte handed out (lines and blob data), not what it only buffered.
struct Rd<R: Read> {
    r: BufReader<R>,
    h: Sha256,
}

impl<R: Read> Rd<R> {
    fn raw_line(&mut self) -> Result<String, StoreError> {
        let mut b = Vec::new();
        let n = (&mut self.r)
            .take(MAX_LINE)
            .read_until(b'\n', &mut b)
            .map_err(|e| StoreError::Io {
                op: "read export",
                path: "<export>".into(),
                source: e,
            })?;
        if b.last() != Some(&b'\n') {
            return Err(bad(if n == 0 {
                "file ends early"
            } else {
                "line too long or file ends early"
            }));
        }
        b.pop();
        String::from_utf8(b).map_err(|_| bad("line is not text"))
    }

    fn line(&mut self) -> Result<String, StoreError> {
        let l = self.raw_line()?;
        self.h.update(l.as_bytes());
        self.h.update(b"\n");
        Ok(l)
    }

    /// The trailer: not part of the hash.
    fn line_unhashed(&mut self) -> Result<String, StoreError> {
        self.raw_line()
    }

    fn count(&mut self, word: &str) -> Result<usize, StoreError> {
        let l = self.line()?;
        l.strip_prefix(word)
            .and_then(|r| r.strip_prefix(' '))
            .and_then(|n| n.parse().ok())
            .filter(|n| *n <= MAX_ITEMS)
            .ok_or_else(|| bad(format!("`{word} <n>` line expected")))
    }
}

struct Tee<'a, R: Read> {
    rd: &'a mut Rd<R>,
}

impl<R: Read> Read for Tee<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.rd.r.read(buf)?;
        self.rd.h.update(&buf[..n]);
        Ok(n)
    }
}
