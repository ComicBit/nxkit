//! File digests: multi-threaded hashing (each file read once, SHA-1 and SHA-256 from the same
//! buffers) and digests supplied from outside (a build store that already knows them).

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};

use crate::error::{Error, Result, format_err};
use crate::hash::{Sha1, Sha256, hex, unhex};

/// The digests of one file's content.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FileDigests {
    /// SHA-1, when it was asked for.
    pub sha1: Option<[u8; 20]>,
    /// SHA-256.
    pub sha256: [u8; 32],
}

/// A provider of digests that are already known, keyed by the path relative to `Contents/`.
///
/// The signer trusts what it gets: a store that supplies digests must guarantee they belong to
/// the bytes now on disk (for example by content addressing). A `None` makes the signer read
/// and hash the file itself. A returned value without `sha1` where SHA-1 is needed also falls
/// back to hashing.
pub trait DigestSource: Sync {
    /// Digests of `path` (relative to `Contents/`) whose size is `size`.
    fn lookup(&self, path: &str, size: u64) -> Option<FileDigests>;
}

/// No precomputed digests: every file is hashed.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoDigests;

impl DigestSource for NoDigests {
    fn lookup(&self, _: &str, _: u64) -> Option<FileDigests> {
        None
    }
}

/// Digests from a tab-separated file: `path <TAB> size <TAB> sha1-hex or - <TAB> sha256-hex`,
/// one line per file, paths relative to `Contents/`. A size mismatch ignores the line.
#[derive(Clone, Debug, Default)]
pub struct TsvDigests {
    map: HashMap<String, (u64, FileDigests)>,
}

impl TsvDigests {
    /// Parses the TSV text.
    pub fn parse(text: &str) -> Result<Self> {
        let mut map = HashMap::new();
        for (n, line) in text.lines().enumerate() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let bad = || format_err(format!("digest line {}: {line:?}", n + 1));
            let mut f = line.split('\t');
            let (Some(path), Some(size), Some(s1), Some(s256), None) =
                (f.next(), f.next(), f.next(), f.next(), f.next())
            else {
                return Err(bad());
            };
            let size: u64 = size.parse().map_err(|_| bad())?;
            let sha1 = if s1 == "-" {
                None
            } else {
                Some(unhex(s1).and_then(|v| v.try_into().ok()).ok_or_else(bad)?)
            };
            let sha256 = unhex(s256)
                .and_then(|v| v.try_into().ok())
                .ok_or_else(bad)?;
            map.insert(path.to_string(), (size, FileDigests { sha1, sha256 }));
        }
        Ok(TsvDigests { map })
    }

    /// Loads a TSV file.
    pub fn load(path: &Path) -> Result<Self> {
        Self::parse(&std::fs::read_to_string(path).map_err(|e| Error::io(path, e))?)
    }

    /// Formats one line of the TSV form.
    pub fn line(path: &str, size: u64, d: &FileDigests) -> String {
        format!(
            "{path}\t{size}\t{}\t{}",
            d.sha1.map_or("-".to_string(), |h| hex(&h)),
            hex(&d.sha256)
        )
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether there are no entries.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl DigestSource for TsvDigests {
    fn lookup(&self, path: &str, size: u64) -> Option<FileDigests> {
        self.map
            .get(path)
            .filter(|(s, _)| *s == size)
            .map(|(_, d)| *d)
    }
}

/// One file to hash.
#[derive(Clone, Debug)]
pub struct HashJob {
    /// Absolute path.
    pub path: PathBuf,
    /// Expected size (for scheduling, the hash covers what is read).
    pub size: u64,
    /// Whether SHA-1 is needed besides SHA-256.
    pub sha1: bool,
}

/// Read size per chunk.
const CHUNK: usize = 4 << 20;
/// Files at least this large hash SHA-1 on a second thread when both digests are needed.
const SPLIT_AT: u64 = 32 << 20;

fn read_full(f: &mut File, buf: &mut [u8], path: &Path) -> Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match f.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(Error::io(path, e)),
        }
    }
    Ok(n)
}

/// Hashes one file, reading it once. `buf` is a reusable read buffer.
pub fn hash_file(job: &HashJob, buf: &mut Vec<u8>) -> Result<FileDigests> {
    let mut f = File::open(&job.path).map_err(|e| Error::io(&job.path, e))?;
    if job.sha1 && job.size >= SPLIT_AT {
        return hash_file_split(&mut f, &job.path);
    }
    buf.resize(CHUNK, 0);
    let mut s256 = Sha256::new();
    let mut s1 = job.sha1.then(Sha1::new);
    loop {
        let n = read_full(&mut f, buf, &job.path)?;
        if n == 0 {
            break;
        }
        s256.update(&buf[..n]);
        if let Some(h) = s1.as_mut() {
            h.update(&buf[..n]);
        }
        if n < buf.len() {
            break;
        }
    }
    Ok(FileDigests {
        sha1: s1.map(Sha1::finalize),
        sha256: s256.finalize(),
    })
}

/// Large file, both digests: this thread reads and does SHA-256, a helper does SHA-1.
fn hash_file_split(f: &mut File, path: &Path) -> Result<FileDigests> {
    std::thread::scope(|s| {
        let (tx, rx) = mpsc::sync_channel::<Arc<Vec<u8>>>(4);
        let helper = s.spawn(move || {
            let mut h = Sha1::new();
            for chunk in rx {
                h.update(&chunk);
            }
            h.finalize()
        });
        let mut s256 = Sha256::new();
        let result = (|| -> Result<()> {
            loop {
                let mut buf = vec![0u8; CHUNK];
                let n = read_full(f, &mut buf, path)?;
                if n == 0 {
                    return Ok(());
                }
                buf.truncate(n);
                let chunk = Arc::new(buf);
                tx.send(Arc::clone(&chunk))
                    .map_err(|_| format_err("SHA-1 helper stopped"))?;
                s256.update(&chunk);
                if n < CHUNK {
                    return Ok(());
                }
            }
        })();
        drop(tx);
        let sha1 = helper
            .join()
            .map_err(|_| format_err("SHA-1 helper panicked"))?;
        result?;
        Ok(FileDigests {
            sha1: Some(sha1),
            sha256: s256.finalize(),
        })
    })
}

/// Hashes `jobs` on `threads` threads (largest files first) and returns digests in job order.
pub fn hash_files(jobs: &[HashJob], threads: usize) -> Result<Vec<FileDigests>> {
    let mut order: Vec<usize> = (0..jobs.len()).collect();
    order.sort_by_key(|i| std::cmp::Reverse(jobs[*i].size));
    let next = AtomicUsize::new(0);
    let threads = threads.clamp(1, jobs.len().max(1));
    let parts: Vec<Result<Vec<(usize, FileDigests)>>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    let mut buf = Vec::new();
                    let mut out = Vec::new();
                    loop {
                        let k = next.fetch_add(1, Ordering::Relaxed);
                        let Some(&i) = order.get(k) else { break };
                        match hash_file(&jobs[i], &mut buf) {
                            Ok(d) => out.push((i, d)),
                            Err(e) => {
                                next.store(order.len(), Ordering::Relaxed);
                                return Err(e);
                            }
                        }
                    }
                    Ok(out)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err(format_err("hash worker panicked")))
            })
            .collect()
    });
    let mut result = vec![None; jobs.len()];
    for part in parts {
        for (i, d) in part? {
            result[i] = Some(d);
        }
    }
    result
        .into_iter()
        .map(|d| d.ok_or_else(|| format_err("file not hashed")))
        .collect()
}

/// Default number of hashing threads: the available parallelism.
pub fn default_threads() -> usize {
    std::thread::available_parallelism().map_or(4, |n| n.get())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::{sha1, sha256};

    #[test]
    fn hashes_files_in_parallel_and_split() {
        let dir = std::env::temp_dir().join(format!("nxb-codesign-digest-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sizes = [
            0usize,
            1,
            4 << 20,
            (4 << 20) + 3,
            (SPLIT_AT as usize) + 12345,
        ];
        let mut jobs = Vec::new();
        let mut contents = Vec::new();
        for (i, n) in sizes.iter().enumerate() {
            let data: Vec<u8> = (0..*n).map(|k| (k * 31 + i) as u8).collect();
            let p = dir.join(format!("f{i}"));
            std::fs::write(&p, &data).unwrap();
            jobs.push(HashJob {
                path: p,
                size: *n as u64,
                sha1: i % 2 == 0 || i == 4,
            });
            contents.push(data);
        }
        let got = hash_files(&jobs, 3).unwrap();
        for ((j, d), c) in jobs.iter().zip(&got).zip(&contents) {
            assert_eq!(d.sha256, sha256(c));
            assert_eq!(d.sha1, j.sha1.then(|| sha1(c)));
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn tsv_round_trip() {
        let d = FileDigests {
            sha1: Some([1; 20]),
            sha256: [2; 32],
        };
        let e = FileDigests {
            sha1: None,
            sha256: [3; 32],
        };
        let text = format!(
            "{}\n{}\n",
            TsvDigests::line("a b/c", 5, &d),
            TsvDigests::line("x", 0, &e)
        );
        let t = TsvDigests::parse(&text).unwrap();
        assert_eq!(t.lookup("a b/c", 5), Some(d));
        assert_eq!(t.lookup("a b/c", 6), None);
        assert_eq!(t.lookup("x", 0), Some(e));
        assert!(TsvDigests::parse("a\t1\tzz\t00").is_err());
    }
}
