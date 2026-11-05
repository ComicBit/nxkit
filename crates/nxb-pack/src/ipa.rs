//! The IPA (`dm/ios/tools/pack_ipa.py`): a zip written straight from the source folders, entries in mapping order
//! (parents, root, then each tree depth first by name), Unix modes, symlinks as links, DOS local times, the UTF-8
//! flag for non-ASCII names, Zip64 fields where a size, an offset or the count overflows.
//!
//! Compression: `pack_ipa.py` deflates with zlib level 6 (raw, `wbits -15`) and keeps a deflate only when it is
//! smaller. [`ZLIB`] is that, through the system zlib in `nxb-sys` (the library Python's `zlib` uses on this Mac);
//! [`STORE_ONLY`] stores everything. Files over 256 MiB are deflated in 16 MiB `compress` calls, as the Python streams
//! them. Not ported: the reuse of the previous IPA (`.idx`).

use std::collections::HashSet;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result, format_err};

const L32: u64 = 0xFFFF_FFFF;
const L16: usize = 0xFFFF;
const DIR_MODE: u32 = 0o040_000 | 0o755;

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

/// One archive entry.
#[derive(Clone, Debug)]
struct Entry {
    name: String,
    path: Option<PathBuf>,
    /// (mode, mtime seconds, size).`None` for a synthetic parent folder.
    st: Option<(u32, i64, u64)>,
}

fn stat_of(p: &Path, follow: bool) -> Result<(u32, i64, u64)> {
    let m = if follow {
        fs::metadata(p)
    } else {
        fs::symlink_metadata(p)
    }
    .map_err(|e| Error::io(p, e))?;
    Ok((m.mode(), m.mtime(), m.size()))
}

fn is_dir(mode: u32) -> bool {
    mode & 0o170_000 == 0o040_000
}

fn is_link(mode: u32) -> bool {
    mode & 0o170_000 == 0o120_000
}

fn is_reg(mode: u32) -> bool {
    mode & 0o170_000 == 0o100_000
}

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
        ctx: (&HashSet<String>, &HashSet<&str>, &[String]),
        out: &mut Vec<Entry>,
        seen: &mut HashSet<String>,
        parents: &HashSet<String>,
    ) -> Result<()> {
        let (skip, roots, skip_suffix) = ctx;
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
    for (arc, src) in maps {
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
                },
            )?;
            walk(
                src,
                arc,
                (skip, &roots, skip_suffix),
                &mut out,
                &mut seen,
                &parents,
            )?;
        }
    }
    Ok(out)
}

fn dos_time(t: i64) -> (u16, u16) {
    let (mut y, mut mo, mut d, mut h, mut mi, mut s) = crate::tz::local(t);
    if y < 1980 {
        (y, mo, d, h, mi, s) = (1980, 1, 1, 0, 0, 0);
    }
    (
        ((h << 11) | (mi << 5) | (s / 2)) as u16,
        ((((y - 1980) as u32) << 9) | (mo << 5) | d) as u16,
    )
}

struct Zip<W: Write + Seek> {
    f: W,
    pos: u64,
    cd: Vec<u8>,
    n: usize,
}

struct Head {
    off: u64,
    name: Vec<u8>,
    flags: u16,
    time: u16,
    date: u16,
}

impl<W: Write + Seek> Zip<W> {
    fn w(&mut self, b: &[u8]) -> Result<()> {
        self.f
            .write_all(b)
            .map_err(|e| format_err(format!("write: {e}")))?;
        self.pos += b.len() as u64;
        Ok(())
    }

    fn local(
        &mut self,
        e: &Entry,
        method: u16,
        crc: u32,
        csize: u64,
        usize_: u64,
        z64: bool,
    ) -> Result<Head> {
        let name = e.name.as_bytes().to_vec();
        let flags: u16 = if e.name.is_ascii() { 0 } else { 0x800 };
        let (time, date) = dos_time(e.st.map_or(0, |s| s.1));
        let mut h = Vec::with_capacity(30 + name.len() + 20);
        h.extend_from_slice(&0x0403_4B50u32.to_le_bytes());
        let ver: u16 = if z64 {
            45
        } else if method == 8 {
            20
        } else {
            10
        };
        for v in [ver, flags, method, time, date] {
            h.extend_from_slice(&v.to_le_bytes());
        }
        h.extend_from_slice(&crc.to_le_bytes());
        let (c, u) = if z64 {
            (L32 as u32, L32 as u32)
        } else {
            (csize as u32, usize_ as u32)
        };
        h.extend_from_slice(&c.to_le_bytes());
        h.extend_from_slice(&u.to_le_bytes());
        h.extend_from_slice(&(name.len() as u16).to_le_bytes());
        h.extend_from_slice(&(if z64 { 20u16 } else { 0 }).to_le_bytes());
        h.extend_from_slice(&name);
        if z64 {
            h.extend_from_slice(&1u16.to_le_bytes());
            h.extend_from_slice(&16u16.to_le_bytes());
            h.extend_from_slice(&usize_.to_le_bytes());
            h.extend_from_slice(&csize.to_le_bytes());
        }
        let off = self.pos;
        self.w(&h)?;
        Ok(Head {
            off,
            name,
            flags,
            time,
            date,
        })
    }

    fn central(&mut self, e: &Entry, h: &Head, method: u16, crc: u32, csize: u64, usize_: u64) {
        let z: Vec<u64> = [usize_, csize, h.off]
            .into_iter()
            .filter(|&v| v >= L32)
            .collect();
        let mode = e.st.map_or(DIR_MODE, |s| s.0);
        let c = &mut self.cd;
        c.extend_from_slice(&0x0201_4B50u32.to_le_bytes());
        let ver: u16 = if !z.is_empty() {
            45
        } else if method == 8 {
            20
        } else {
            10
        };
        for v in [0x031Eu16, ver, h.flags, method, h.time, h.date] {
            c.extend_from_slice(&v.to_le_bytes());
        }
        c.extend_from_slice(&crc.to_le_bytes());
        c.extend_from_slice(&(csize.min(L32) as u32).to_le_bytes());
        c.extend_from_slice(&(usize_.min(L32) as u32).to_le_bytes());
        c.extend_from_slice(&(h.name.len() as u16).to_le_bytes());
        c.extend_from_slice(
            &(if z.is_empty() {
                0u16
            } else {
                4 + 8 * z.len() as u16
            })
            .to_le_bytes(),
        );
        for v in [0u16, 0, 0] {
            c.extend_from_slice(&v.to_le_bytes());
        }
        c.extend_from_slice(&((mode << 16) | if is_dir(mode) { 0x10 } else { 0 }).to_le_bytes());
        c.extend_from_slice(&(h.off.min(L32) as u32).to_le_bytes());
        c.extend_from_slice(&h.name);
        if !z.is_empty() {
            c.extend_from_slice(&1u16.to_le_bytes());
            c.extend_from_slice(&(8 * z.len() as u16).to_le_bytes());
            for v in z {
                c.extend_from_slice(&v.to_le_bytes());
            }
        }
        self.n += 1;
    }

    fn put(&mut self, e: &Entry, method: u16, crc: u32, data: &[u8], usize_: u64) -> Result<()> {
        let z64 = usize_ >= L32 || data.len() as u64 >= L32;
        let h = self.local(e, method, crc, data.len() as u64, usize_, z64)?;
        self.w(data)?;
        self.central(e, &h, method, crc, data.len() as u64, usize_);
        Ok(())
    }

    /// A file read in chunks, stored, its CRC patched into the local header afterwards (`stream` with deflate off).
    fn stream_stored(&mut self, e: &Entry, path: &Path, size: u64) -> Result<()> {
        let z64 = size >= L32;
        let h = self.local(e, 0, 0, size, size, z64)?;
        let mut f = fs::File::open(path).map_err(|x| Error::io(path, x))?;
        let mut buf = vec![0u8; 16 << 20];
        let mut crc = crc32fast_init();
        let mut read = 0u64;
        loop {
            let n = f.read(&mut buf).map_err(|x| Error::io(path, x))?;
            if n == 0 {
                break;
            }
            crc = crc32_update(crc, &buf[..n]);
            self.w(&buf[..n])?;
            read += n as u64;
        }
        if read != size {
            return Err(format_err(format!(
                "{} changed size while it was packed",
                path.display()
            )));
        }
        let crc = !crc;
        let end = self.pos;
        self.f
            .seek(SeekFrom::Start(h.off + 14))
            .map_err(|x| format_err(x.to_string()))?;
        self.f
            .write_all(&crc.to_le_bytes())
            .map_err(|x| format_err(x.to_string()))?;
        self.f
            .seek(SeekFrom::Start(end))
            .map_err(|x| format_err(x.to_string()))?;
        self.central(e, &h, 0, crc, size, size);
        Ok(())
    }

    fn close(mut self) -> Result<W> {
        let cd_off = self.pos;
        let blob = std::mem::take(&mut self.cd);
        let n = self.n;
        self.w(&blob)?;
        if n >= L16 || cd_off >= L32 || blob.len() as u64 >= L32 {
            let z = self.pos;
            let mut r = Vec::new();
            r.extend_from_slice(&0x0606_4B50u32.to_le_bytes());
            r.extend_from_slice(&44u64.to_le_bytes());
            r.extend_from_slice(&0x031Eu16.to_le_bytes());
            r.extend_from_slice(&45u16.to_le_bytes());
            r.extend_from_slice(&[0u8; 8]);
            r.extend_from_slice(&(n as u64).to_le_bytes());
            r.extend_from_slice(&(n as u64).to_le_bytes());
            r.extend_from_slice(&(blob.len() as u64).to_le_bytes());
            r.extend_from_slice(&cd_off.to_le_bytes());
            r.extend_from_slice(&0x0706_4B50u32.to_le_bytes());
            r.extend_from_slice(&0u32.to_le_bytes());
            r.extend_from_slice(&z.to_le_bytes());
            r.extend_from_slice(&1u32.to_le_bytes());
            self.w(&r)?;
        }
        let mut e = Vec::new();
        e.extend_from_slice(&0x0605_4B50u32.to_le_bytes());
        for v in [0u16, 0, n.min(L16) as u16, n.min(L16) as u16] {
            e.extend_from_slice(&v.to_le_bytes());
        }
        e.extend_from_slice(&((blob.len() as u64).min(L32) as u32).to_le_bytes());
        e.extend_from_slice(&(cd_off.min(L32) as u32).to_le_bytes());
        e.extend_from_slice(&0u16.to_le_bytes());
        self.w(&e)?;
        Ok(self.f)
    }
}

fn crc32fast_init() -> u32 {
    0xFFFF_FFFF
}

fn crc_table() -> &'static [u32; 256] {
    static T: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, x) in t.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
            *x = c;
        }
        t
    })
}

fn crc32_update(mut c: u32, data: &[u8]) -> u32 {
    let t = crc_table();
    for &b in data {
        c = t[((c ^ b as u32) & 0xff) as usize] ^ (c >> 8);
    }
    c
}

/// `zlib.crc32`.
pub fn crc32(data: &[u8]) -> u32 {
    !crc32_update(0xFFFF_FFFF, data)
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
}

/// Packs `maps` (archive path, source) into `out` (written as `<out>.partial`, renamed at the end). Returns the number
/// of entries.
pub fn pack(out: &Path, maps: &[(String, PathBuf)], o: &PackOptions<'_>) -> Result<usize> {
    let es = entries(maps, &o.skip, &o.skip_suffix)?;
    let mut part = out.as_os_str().to_owned();
    part.push(".partial");
    let part = PathBuf::from(part);
    let f = fs::File::create(&part).map_err(|e| Error::io(&part, e))?;
    let mut z = Zip {
        f: std::io::BufWriter::with_capacity(1 << 20, f),
        pos: 0,
        cd: Vec::new(),
        n: 0,
    };
    for e in &es {
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
            let deflate = !o
                .store
                .iter()
                .any(|s| e.name.to_lowercase().ends_with(s.as_str()));
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
    let n = es.len();
    let mut w = z.close()?;
    w.flush().map_err(|e| format_err(e.to_string()))?;
    drop(w);
    fs::rename(&part, out).map_err(|e| Error::io(out, e))?;
    Ok(n)
}
