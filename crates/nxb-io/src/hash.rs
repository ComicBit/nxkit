//! Parallel file hashing.

use nxb_model::{DualDigest, DualHasher, FileDigest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

const CHUNK: usize = 1 << 20;

/// A digest kind the helpers can compute: SHA-256 ([`FileDigest`]) or SHA-256 + SHA-1 in one pass ([`DualDigest`]).
pub trait FileHash: Send + Sized {
    /// Hash a reader to its end.
    fn read_all(r: &mut dyn Read, buf: &mut [u8]) -> io::Result<Self>;
}

fn feed(r: &mut dyn Read, buf: &mut [u8], mut f: impl FnMut(&[u8])) -> io::Result<()> {
    loop {
        match r.read(buf) {
            Ok(0) => return Ok(()),
            Ok(n) => f(&buf[..n]),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
}

impl FileHash for FileDigest {
    fn read_all(r: &mut dyn Read, buf: &mut [u8]) -> io::Result<Self> {
        let mut h = Sha256::new();
        feed(r, buf, |b| h.update(b))?;
        Ok(FileDigest {
            len: h.len(),
            sha256: h.finalize(),
        })
    }
}

impl FileHash for DualDigest {
    fn read_all(r: &mut dyn Read, buf: &mut [u8]) -> io::Result<Self> {
        let mut h = DualHasher::new();
        feed(r, buf, |b| h.update(b))?;
        Ok(h.finish())
    }
}

/// Hash one file (SHA-256) with a 1 MiB buffer.
pub fn hash_file(path: &Path) -> io::Result<FileDigest> {
    let mut buf = vec![0u8; CHUNK];
    hash_with(path, &mut buf)
}

fn hash_with<D: FileHash>(path: &Path, buf: &mut [u8]) -> io::Result<D> {
    D::read_all(&mut File::open(path)?, buf)
}

fn thread_count(threads: usize, jobs: usize) -> usize {
    let t = if threads == 0 {
        std::thread::available_parallelism().map_or(4, |n| n.get())
    } else {
        threads
    };
    t.clamp(1, jobs.max(1))
}

/// Hash `paths` on `threads` threads (0 = all logical CPUs). Largest files start first, the result has one entry per
/// path, **in input order**, an unreadable file is an `Err` for that entry only.
pub fn hash_files(paths: &[PathBuf], threads: usize) -> Vec<io::Result<FileDigest>> {
    hash_files_as(paths, threads)
}

/// As [`hash_files`], with SHA-1 beside SHA-256 in the same read (for `CodeResources` `files` and `files2`).
pub fn hash_files_dual(paths: &[PathBuf], threads: usize) -> Vec<io::Result<DualDigest>> {
    hash_files_as(paths, threads)
}

fn hash_files_as<D: FileHash>(paths: &[PathBuf], threads: usize) -> Vec<io::Result<D>> {
    let mut order: Vec<(u64, usize)> = paths
        .iter()
        .enumerate()
        .map(|(i, p)| (fs::metadata(p).map_or(0, |m| m.len()), i))
        .collect();
    // Largest first (LPT), ties by input index for a stable schedule.
    order.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let results: Vec<Mutex<Option<io::Result<D>>>> =
        paths.iter().map(|_| Mutex::new(None)).collect();
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..thread_count(threads, paths.len()) {
            s.spawn(|| {
                let mut buf = vec![0u8; CHUNK];
                loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&(_, i)) = order.get(k) else { break };
                    let r = hash_with(&paths[i], &mut buf);
                    *results[i].lock().unwrap_or_else(PoisonError::into_inner) = Some(r);
                }
            });
        }
    });
    results
        .into_iter()
        .map(|m| {
            m.into_inner()
                .unwrap_or_else(PoisonError::into_inner)
                .unwrap_or_else(|| Err(io::Error::other("file was not hashed")))
        })
        .collect()
}

/// A regular file of a tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeEntry<D = FileDigest> {
    /// Path relative to the root, `/`-separated.
    pub rel: String,
    /// Its digest.
    pub digest: D,
}

/// A symbolic link of a tree (not followed).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeSymlink {
    /// Path relative to the root, `/`-separated.
    pub rel: String,
    /// The link target as stored.
    pub target: PathBuf,
}

/// Every regular file and symlink under a root, each sorted by relative path (byte order of the UTF-8 path).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TreeHash<D = FileDigest> {
    /// Regular files with digests.
    pub files: Vec<TreeEntry<D>>,
    /// Symbolic links.
    pub symlinks: Vec<TreeSymlink>,
}

fn walk(
    root: &Path,
    dir: &Path,
    files: &mut Vec<(String, PathBuf)>,
    links: &mut Vec<TreeSymlink>,
) -> io::Result<()> {
    for e in fs::read_dir(dir)? {
        let e = e?;
        let p = e.path();
        let ft = e.file_type()?;
        let rel = p
            .strip_prefix(root)
            .ok()
            .and_then(Path::to_str)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "path is not UTF-8"))?
            .replace(std::path::MAIN_SEPARATOR, "/");
        if ft.is_symlink() {
            links.push(TreeSymlink {
                rel,
                target: fs::read_link(&p)?,
            });
        } else if ft.is_dir() {
            walk(root, &p, files, links)?;
        } else if ft.is_file() {
            files.push((rel, p));
        }
    }
    Ok(())
}

/// Hash every regular file under `root` in parallel. Symlinks are listed, not followed, other file types are skipped.
/// Any unreadable file fails the whole call (a tree with a hole is not a usable seal input).
pub fn hash_tree(root: &Path, threads: usize) -> io::Result<TreeHash> {
    hash_tree_as(root, threads)
}

/// As [`hash_tree`], with SHA-1 beside SHA-256 for every file, each file read once.
pub fn hash_tree_dual(root: &Path, threads: usize) -> io::Result<TreeHash<DualDigest>> {
    hash_tree_as(root, threads)
}

fn hash_tree_as<D: FileHash>(root: &Path, threads: usize) -> io::Result<TreeHash<D>> {
    let mut found = Vec::new();
    let mut symlinks = Vec::new();
    walk(root, root, &mut found, &mut symlinks)?;
    found.sort();
    symlinks.sort_by(|a, b| a.rel.cmp(&b.rel));
    let paths: Vec<PathBuf> = found.iter().map(|(_, p)| p.clone()).collect();
    let digests = hash_files_as::<D>(&paths, threads);
    let mut files = Vec::with_capacity(found.len());
    for ((rel, _), d) in found.into_iter().zip(digests) {
        files.push(TreeEntry { rel, digest: d? });
    }
    Ok(TreeHash { files, symlinks })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nxb_testkit::check;
    use std::sync::atomic::AtomicU32;

    static N: AtomicU32 = AtomicU32::new(0);

    fn scratch() -> PathBuf {
        let base = std::env::var_os("NXB_TEST_TMP")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let d = base.join(format!(
            "nxb-io-test-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn files_in_input_order_with_errors_isolated() {
        let d = scratch();
        let mut paths = Vec::new();
        let mut want = Vec::new();
        for (i, size) in [5usize, 3 << 20, 0, 70_000].into_iter().enumerate() {
            let data: Vec<u8> = (0..size).map(|j| (j * 13 + i) as u8).collect();
            let p = d.join(format!("f{i}"));
            fs::write(&p, &data).unwrap();
            paths.push(p);
            want.push(FileDigest::of(&data));
        }
        paths.insert(2, d.join("missing"));
        let got = hash_files(&paths, 3);
        assert_eq!(got.len(), 5);
        assert!(got[2].is_err());
        let ok: Vec<FileDigest> = got
            .into_iter()
            .enumerate()
            .filter(|(i, _)| *i != 2)
            .map(|(_, r)| r.unwrap())
            .collect();
        assert_eq!(ok, want);
        assert_eq!(hash_file(&paths[1]).unwrap(), want[1]);
        fs::remove_dir_all(d).unwrap();
    }

    /// Property: a random tree hashes to the digests of its contents, sorted, for any thread count.
    #[test]
    fn tree_matches_contents() {
        check(91, 6, |r| {
            let d = scratch();
            let mut want = Vec::new();
            for i in 0..r.below(30) {
                let sub = if r.chance(1, 2) { "a/b/" } else { "" };
                let rel = format!("{sub}file{i}.bin");
                let n = r.below(20_000) as usize;
                let data = r.bytes(n);
                fs::create_dir_all(d.join(sub)).unwrap();
                fs::write(d.join(&rel), &data).unwrap();
                want.push(TreeEntry {
                    rel,
                    digest: FileDigest::of(&data),
                });
            }
            want.sort_by(|a, b| a.rel.cmp(&b.rel));
            #[cfg(unix)]
            std::os::unix::fs::symlink("file0.bin", d.join("link")).unwrap();
            let threads = r.below(5) as usize;
            let t = hash_tree(&d, threads).unwrap();
            assert_eq!(t.files, want);
            let dual = hash_tree_dual(&d, threads).unwrap();
            for (a, b) in dual.files.iter().zip(&want) {
                assert_eq!(a.rel, b.rel);
                assert_eq!(a.digest.file, b.digest);
                let content = fs::read(d.join(&a.rel)).unwrap();
                assert_eq!(a.digest.sha1, nxb_model::sha1(&content));
            }
            #[cfg(unix)]
            assert_eq!(t.symlinks.len(), 1);
            fs::remove_dir_all(d).unwrap();
        });
    }
}
