//! The IPA (`dm/ios/tools/pack_ipa.py`): a zip written straight from the source folders, entries in mapping order
//! (parents, root, then each tree depth first by name), Unix modes, symlinks as links, DOS local times, the UTF-8
//! flag for non-ASCII names, Zip64 fields where a size, an offset or the count overflows.
//!
//! Compression: `pack_ipa.py` deflates with zlib level 6 (raw, `wbits -15`) and keeps a deflate only when it is
//! smaller. [`ZLIB`] is that, through the system zlib in `nxb-sys` (the library Python's `zlib` uses on this Mac);
//! [`STORE_ONLY`] stores everything. Files over 256 MiB are deflated in 16 MiB `compress` calls, as the Python streams
//! them. The reuse of the previous IPA (`<out>.idx`) is in the module `ipa_reuse`.

use std::collections::HashSet;
use std::fs;
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result, format_err};
use crate::ipa_reuse::{self, Fp};
pub use crate::ipa_zip::crc32;
use crate::ipa_zip::{Entry, Head, Zip, is_dir, is_link, is_reg, stat_of};

/// Raw deflate of a file's bytes (zlib level 6, `wbits -15`), or `None` to store it.
pub type Compressor<'a> = &'a dyn Fn(&[u8]) -> Option<Vec<u8>>;

/// Stores everything.
pub const STORE_ONLY: Compressor<'static> = &|_| None;

/// zlib level 6 raw deflate, as `zlib.compressobj(6, zlib.DEFLATED, -15)`, files over 256 MiB in 16 MiB pieces.
pub const ZLIB: Compressor<'static> = &|data| {
    let mut d = nxb_sys::zlib::Deflate::new(6, -15)?;
    let mut out = Vec::new();
    if data.len() > (256 << 20) {
        for c in data.chunks(16 << 20) {
            out.extend(d.compress(c)?);
        }
    } else {
        out.extend(d.compress(data)?);
    }
    out.extend(d.finish()?);
    Some(out)
};

/// `entries`: the archive's entries in order.
fn entries(
    maps: &[(String, PathBuf)],
    skip: &HashSet<String>,
    skip_suffix: &[String],
) -> Result<Vec<Entry>> {
    let roots: HashSet<&str> = maps.iter().map(|(a, _)| a.as_str()).collect();
    let mut out: Vec<Entry> = Vec::new();
    let mut seen = HashSet::new();
    let mut parents = HashSet::new();
    fn add(
        out: &mut Vec<Entry>,
        seen: &mut HashSet<String>,
        parents: &HashSet<String>,
        e: Entry,
    ) -> Result<()> {
        if parents.contains(&e.name) && e.st.is_some_and(|s| is_dir(s.0)) {
            return Ok(());
        }
        if !seen.insert(e.name.clone()) {
            return Err(format_err(format!("{} twice in the archive", e.name)));
        }
        out.push(e);
        Ok(())
    }
    fn walk(
        d: &Path,
        arc: &str,
        ctx: (&HashSet<String>, &HashSet<&str>, &[String], usize),
        out: &mut Vec<Entry>,
        seen: &mut HashSet<String>,
        parents: &HashSet<String>,
    ) -> Result<()> {
        let (skip, roots, skip_suffix, ri) = ctx;
        let mut es: Vec<(String, PathBuf)> = fs::read_dir(d)
            .map_err(|e| Error::io(d, e))?
            .filter_map(|e| e.ok())
            .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
            .collect();
        es.sort();
        for (n, p) in es {
            let name = format!("{arc}/{n}");
            let st = stat_of(&p, false)?;
            if skip.contains(&name)
                || roots.contains(name.as_str())
                || (!is_dir(st.0) && skip_suffix.iter().any(|s| n.ends_with(s.as_str())))
            {
                continue;
            }
            if is_dir(st.0) {
                add(
                    out,
                    seen,
                    parents,
                    Entry {
                        name: format!("{name}/"),
                        path: Some(p.clone()),
                        st: Some(st),
                        root: ri,
                    },
                )?;
                walk(&p, &name, ctx, out, seen, parents)?;
            } else if is_reg(st.0) || is_link(st.0) {
                add(
                    out,
                    seen,
                    parents,
                    Entry {
                        name,
                        path: Some(p),
                        st: Some(st),
                        root: ri,
                    },
                )?;
            } else {
                return Err(format_err(format!(
                    "neither a file, a folder nor a symlink: {}",
                    p.display()
                )));
            }
        }
        Ok(())
    }
    for (ri, (arc, src)) in maps.iter().enumerate() {
        let parts: Vec<&str> = arc.split('/').collect();
        for i in 1..parts.len() {
            let p = format!("{}/", parts[..i].join("/"));
            if !seen.contains(&p) {
                add(
                    &mut out,
                    &mut seen,
                    &parents,
                    Entry {
                        name: p.clone(),
                        path: None,
                        st: None,
                        root: ri,
                    },
                )?;
                parents.insert(p);
            }
        }
        let st = stat_of(src, true)?;
        if is_reg(st.0) {
            add(
                &mut out,
                &mut seen,
                &parents,
                Entry {
                    name: arc.clone(),
                    path: Some(src.clone()),
                    st: Some(st),
                    root: ri,
                },
            )?;
        } else {
            add(
                &mut out,
                &mut seen,
                &parents,
                Entry {
                    name: format!("{arc}/"),
                    path: Some(src.clone()),
                    st: Some(st),
                    root: ri,
                },
            )?;
            walk(
                src,
                arc,
                (skip, &roots, skip_suffix, ri),
                &mut out,
                &mut seen,
                &parents,
            )?;
        }
    }
    Ok(out)
}

/// Options of [`pack`].
pub struct PackOptions<'a> {
    /// Suffixes always stored (lower case, compared without regard to case).
    pub store: Vec<String>,
    /// Archive paths left out of folder walks.
    pub skip: HashSet<String>,
    /// File name suffixes left out of folder walks.
    pub skip_suffix: Vec<String>,
    /// Deflate.
    pub compress: Compressor<'a>,
    /// Reuse the previous IPA beside `out` (`<out>.idx`).`false` is `--no-reuse`.
    pub reuse: bool,
}

fn head_at(h: &Head, off: u64) -> Head {
    Head {
        off,
        name: h.name.clone(),
        flags: h.flags,
        time: h.time,
        date: h.date,
    }
}

fn suffixed(p: &Path, s: &str) -> PathBuf {
    let mut x = p.as_os_str().to_owned();
    x.push(s);
    PathBuf::from(x)
}

/// Packs `maps` (archive path, source) into `out` (written as `<out>.partial`, renamed at the end, then the index
/// `<out>.idx`). Returns the number of entries.
pub fn pack(out: &Path, maps: &[(String, PathBuf)], o: &PackOptions<'_>) -> Result<usize> {
    pack_report(out, maps, o).map(|r| r.entries)
}

/// What a pack did.
#[derive(Clone, Copy, Debug)]
pub struct Report {
    /// Entries in the archive.
    pub entries: usize,
    /// Bytes kept from the old IPA (the clone).
    pub kept: u64,
    /// Files copied compressed from the old IPA after the kept part.
    pub reused: usize,
}

/// [`pack`] with its report.
pub fn pack_report(out: &Path, maps: &[(String, PathBuf)], o: &PackOptions<'_>) -> Result<Report> {
    let mut reused = 0;
    let es = entries(maps, &o.skip, &o.skip_suffix)?;
    let apfs: Vec<bool> = maps
        .iter()
        .map(|(_, s)| nxb_sys::system::is_apfs(s))
        .collect();
    let flags: Vec<bool> = es
        .iter()
        .map(|e| {
            let n = e.name.to_lowercase();
            !o.store.iter().any(|s| n.ends_with(s.as_str()))
        })
        .collect();
    let fps: Vec<Option<Fp>> = es
        .iter()
        .zip(&flags)
        .map(|(e, &d)| match (e.st, &e.path) {
            (Some((m, _, _)), Some(p)) if is_reg(m) => Fp::of(p, d),
            _ => None,
        })
        .collect();
    let (part, tmp, ipart) = (
        suffixed(out, ".partial"),
        suffixed(out, ".clone"),
        suffixed(out, ".idx.partial"),
    );
    for p in [&part, &tmp, &ipart] {
        let _ = fs::remove_file(p);
    }
    let old = if o.reuse { ipa_reuse::load(out) } else { None };
    let (mut keep, mut kept) = old
        .as_ref()
        .map_or((vec![], 0), |old| ipa_reuse::prefix(&es, &fps, old, &apfs));
    if kept > 0 {
        let cloned = old
            .as_ref()
            .is_some_and(|_| nxb_sys::clone::clone_only(out, &tmp).unwrap_or(false));
        if cloned {
            let f = fs::OpenOptions::new()
                .write(true)
                .open(&tmp)
                .map_err(|e| Error::io(&tmp, e))?;
            f.set_len(kept).map_err(|e| Error::io(&tmp, e))?;
            drop(f);
            fs::rename(&tmp, &part).map_err(|e| Error::io(&part, e))?;
        } else {
            (keep, kept) = (vec![], 0);
        }
    }
    let _ = fs::remove_file(&tmp);
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(kept == 0)
        .open(&part)
        .map_err(|e| Error::io(&part, e))?;
    f.seek(SeekFrom::Start(kept))
        .map_err(|e| Error::io(&part, e))?;
    let mut z = Zip {
        f: std::io::BufWriter::with_capacity(1 << 20, f),
        pos: kept,
        cd: Vec::new(),
        n: 0,
    };
    for (i, head, info) in &keep {
        z.central(
            &es[*i],
            &head_at(head, info.off),
            info.method,
            info.crc,
            info.csize,
            info.usize_,
        );
    }
    for (i, e) in es.iter().enumerate().skip(keep.len()) {
        let Some((mode, _, size)) = e.st else {
            z.put(e, 0, 0, &[], 0)?;
            continue;
        };
        let path = e.path.clone().unwrap_or_default();
        if is_link(mode) {
            let t = fs::read_link(&path).map_err(|x| Error::io(&path, x))?;
            let b = t.as_os_str().as_encoded_bytes().to_vec();
            z.put(e, 0, crc32(&b), &b, b.len() as u64)?;
        } else if is_dir(mode) {
            z.put(e, 0, 0, &[], 0)?;
        } else {
            if let Some(old) = &old
                && let Some((_, _, info)) = ipa_reuse::matched(e, fps[i].as_ref(), old, &apfs)
            {
                z.copy_from(e, &old.file, &info)?;
                reused += 1;
                continue;
            }
            let deflate = flags[i];
            if size > (256 << 20) && !deflate {
                z.stream_stored(e, &path, size)?;
                continue;
            }
            let data = fs::read(&path).map_err(|x| Error::io(&path, x))?;
            let crc = crc32(&data);
            match (
                deflate,
                if deflate && !data.is_empty() {
                    (o.compress)(&data)
                } else {
                    None
                },
            ) {
                (true, Some(c)) if c.len() < data.len() => {
                    z.put(e, 8, crc, &c, data.len() as u64)?
                }
                _ => z.put(e, 0, crc, &data, data.len() as u64)?,
            }
        }
    }
    let (n, kept_bytes) = (es.len(), kept);
    let mut w = z.close()?;
    w.flush().map_err(|e| format_err(e.to_string()))?;
    drop(w);
    let m = fs::metadata(&part).map_err(|e| Error::io(&part, e))?;
    let ipa = (m.size(), m.mtime() * 1_000_000_000 + m.mtime_nsec());
    drop(old);
    fs::rename(&part, out).map_err(|e| Error::io(out, e))?;
    // the index only after the rename: an IPA it does not describe (size, time) makes it ignored, a failure only
    // makes the next pack pack every file again
    let files: Vec<(&str, Fp)> = es
        .iter()
        .zip(&fps)
        .filter_map(|(e, f)| Some((e.name.as_str(), (*f)?)))
        .collect();
    let idx = suffixed(out, ".idx");
    if fs::write(&ipart, ipa_reuse::index_text(ipa, &files)).is_ok() {
        let _ = fs::rename(&ipart, &idx);
    }
    Ok(Report {
        entries: n,
        kept: kept_bytes,
        reused,
    })
}
