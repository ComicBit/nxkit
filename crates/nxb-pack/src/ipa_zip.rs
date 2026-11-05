//! The zip writer of the IPA: entries, their metadata and the local and central records.

use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result, format_err};

pub(crate) const L32: u64 = 0xFFFF_FFFF;
pub(crate) const L16: usize = 0xFFFF;
pub(crate) const DIR_MODE: u32 = 0o040_000 | 0o755;

/// One archive entry.
#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub(crate) name: String,
    pub(crate) path: Option<PathBuf>,
    /// (mode, mtime seconds, size).`None` for a synthetic parent folder.
    pub(crate) st: Option<(u32, i64, u64)>,
    /// Index of the mapping whose source folder holds it.
    pub(crate) root: usize,
}

pub(crate) fn stat_of(p: &Path, follow: bool) -> Result<(u32, i64, u64)> {
    let m = if follow {
        fs::metadata(p)
    } else {
        fs::symlink_metadata(p)
    }
    .map_err(|e| Error::io(p, e))?;
    Ok((m.mode(), m.mtime(), m.size()))
}

pub(crate) fn is_dir(mode: u32) -> bool {
    mode & 0o170_000 == 0o040_000
}

pub(crate) fn is_link(mode: u32) -> bool {
    mode & 0o170_000 == 0o120_000
}

pub(crate) fn is_reg(mode: u32) -> bool {
    mode & 0o170_000 == 0o100_000
}

pub(crate) fn dos_time(t: i64) -> (u16, u16) {
    let (mut y, mut mo, mut d, mut h, mut mi, mut s) = crate::tz::local(t);
    if y < 1980 {
        (y, mo, d, h, mi, s) = (1980, 1, 1, 0, 0, 0);
    }
    (
        ((h << 11) | (mi << 5) | (s / 2)) as u16,
        ((((y - 1980) as u32) << 9) | (mo << 5) | d) as u16,
    )
}

/// A local header: its bytes and the fields the central record repeats (`off` 0).
pub(crate) fn header(
    e: &Entry,
    method: u16,
    crc: u32,
    csize: u64,
    usize_: u64,
    z64: bool,
) -> (Vec<u8>, Head) {
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
    (
        h,
        Head {
            off: 0,
            name,
            flags,
            time,
            date,
        },
    )
}

pub(crate) struct Zip<W: Write + Seek> {
    pub(crate) f: W,
    pub(crate) pos: u64,
    pub(crate) cd: Vec<u8>,
    pub(crate) n: usize,
}

pub(crate) struct Head {
    pub(crate) off: u64,
    pub(crate) name: Vec<u8>,
    pub(crate) flags: u16,
    pub(crate) time: u16,
    pub(crate) date: u16,
}

impl<W: Write + Seek> Zip<W> {
    pub(crate) fn w(&mut self, b: &[u8]) -> Result<()> {
        self.f
            .write_all(b)
            .map_err(|e| format_err(format!("write: {e}")))?;
        self.pos += b.len() as u64;
        Ok(())
    }

    pub(crate) fn local(
        &mut self,
        e: &Entry,
        method: u16,
        crc: u32,
        csize: u64,
        usize_: u64,
        z64: bool,
    ) -> Result<Head> {
        let (h, mut head) = header(e, method, crc, csize, usize_, z64);
        head.off = self.pos;
        self.w(&h)?;
        Ok(head)
    }

    pub(crate) fn central(
        &mut self,
        e: &Entry,
        h: &Head,
        method: u16,
        crc: u32,
        csize: u64,
        usize_: u64,
    ) {
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

    pub(crate) fn put(
        &mut self,
        e: &Entry,
        method: u16,
        crc: u32,
        data: &[u8],
        usize_: u64,
    ) -> Result<()> {
        let z64 = usize_ >= L32 || data.len() as u64 >= L32;
        let h = self.local(e, method, crc, data.len() as u64, usize_, z64)?;
        self.w(data)?;
        self.central(e, &h, method, crc, data.len() as u64, usize_);
        Ok(())
    }

    /// A file read in chunks, stored, its CRC patched into the local header afterwards (`stream` with deflate off).
    pub(crate) fn stream_stored(&mut self, e: &Entry, path: &Path, size: u64) -> Result<()> {
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

    /// A file whose compressed bytes are copied from the old IPA behind a fresh local header.
    pub(crate) fn copy_from(
        &mut self,
        e: &Entry,
        old: &fs::File,
        i: &crate::ipa_reuse::Info,
    ) -> Result<()> {
        use std::os::unix::fs::FileExt;
        let h = self.local(
            e,
            i.method,
            i.crc,
            i.csize,
            i.usize_,
            i.usize_ >= L32 || i.csize >= L32,
        )?;
        let (mut at, mut left) = (i.off + 30 + h.name.len() as u64, i.csize);
        // the old local header's extra field length (the new one has the same bytes: it was matched)
        let mut el = [0u8; 2];
        old.read_exact_at(&mut el, i.off + 28)
            .map_err(|x| format_err(x.to_string()))?;
        at += u16::from_le_bytes(el) as u64;
        let mut buf = vec![0u8; 16 << 20];
        while left > 0 {
            let n = left.min(buf.len() as u64) as usize;
            old.read_exact_at(&mut buf[..n], at).map_err(|x| {
                format_err(format!(
                    "the old IPA ended while {} was copied from it: {x}",
                    e.name
                ))
            })?;
            self.w(&buf[..n])?;
            left -= n as u64;
            at += n as u64;
        }
        self.central(e, &h, i.method, i.crc, i.csize, i.usize_);
        Ok(())
    }

    pub(crate) fn close(mut self) -> Result<W> {
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

pub(crate) fn crc32fast_init() -> u32 {
    0xFFFF_FFFF
}

pub(crate) fn crc_table() -> &'static [u32; 256] {
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

pub(crate) fn crc32_update(mut c: u32, data: &[u8]) -> u32 {
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
