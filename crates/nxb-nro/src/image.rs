//! The host's `DMROMFS1` image, read by ranges: the tree walk of `os.walk` over `romfs.py`'s view of it.

use std::os::unix::fs::FileExt;
use std::path::Path;
use std::sync::Arc;

use nxb_split::SplitError;
use nxb_split::src::Src;

/// A file of the image: path below the root, offset and size in the image.
pub(crate) struct Item {
    /// Path below the image root, `/` separated.
    pub(crate) rel: String,
    /// Offset in the image.
    pub(crate) off: u64,
    /// Size.
    pub(crate) size: u64,
}

/// A range of the image file as a [`Src`].
pub(crate) struct Range {
    file: Arc<std::fs::File>,
    off: u64,
    size: u64,
}

impl Src for Range {
    fn len(&self) -> u64 {
        self.size
    }
    fn read_at(&self, off: u64, buf: &mut [u8]) -> Result<(), SplitError> {
        if off.saturating_add(buf.len() as u64) > self.size {
            return Err(SplitError::Red("read past the end of the file".into()));
        }
        self.file
            .read_exact_at(buf, self.off + off)
            .map_err(|e| SplitError::Format(format!("read from the image: {e}")))
    }
}

/// An opened image.
pub(crate) struct Image {
    file: Arc<std::fs::File>,
    /// Every file in `os.walk` order (the files of a folder by name, then its folders).
    pub(crate) files: Vec<Item>,
}

fn u(b: &[u8], o: usize, n: usize) -> Option<u64> {
    let s = b.get(o..o + n)?;
    Some(s.iter().rev().fold(0u64, |a, &x| a << 8 | u64::from(x)))
}

impl Image {
    /// Opens `path`.`Err` names what is wrong (also for a folder or another format).
    pub(crate) fn open(path: &Path) -> Result<Image, String> {
        let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut h = [0u8; 32];
        file.read_exact_at(&mut h, 0)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if &h[..8] != b"DMROMFS1" {
            return Err("not a DMROMFS1 image".into());
        }
        let (off, size, n) = (
            u(&h, 8, 8).unwrap_or(0),
            u(&h, 16, 8).unwrap_or(0),
            u(&h, 24, 8).unwrap_or(0) as usize,
        );
        let mut idx = vec![0u8; usize::try_from(size).map_err(|e| e.to_string())?];
        file.read_exact_at(&mut idx, off)
            .map_err(|e| format!("image index: {e}"))?;
        let bad = || "damaged image index".to_string();
        if n == 0 || idx.len() < n * 24 {
            return Err(bad());
        }
        let pool = n * 24;
        let ent = |k: usize| -> Option<(u64, u64, usize, usize, u8)> {
            let o = k * 24;
            Some((
                u(&idx, o, 8)?,
                u(&idx, o + 8, 8)?,
                u(&idx, o + 16, 4)? as usize,
                u(&idx, o + 20, 2)? as usize,
                *idx.get(o + 22)?,
            ))
        };
        let mut files = Vec::new();
        let mut stack = vec![(0usize, String::new())];
        while let Some((d, prefix)) = stack.pop() {
            let (a, b, ..) = ent(d).ok_or_else(bad)?;
            let (mut dirs, mut fs) = (Vec::new(), Vec::new());
            for j in a as usize..(a + b) as usize {
                let (s, l, no, nl, t) = ent(j).ok_or_else(bad)?;
                let name = idx.get(pool + no..pool + no + nl).ok_or_else(bad)?;
                let rel = format!("{prefix}{}", String::from_utf8_lossy(name));
                if t == 1 {
                    fs.push((
                        name.to_vec(),
                        Item {
                            rel,
                            off: s,
                            size: l,
                        },
                    ));
                } else {
                    dirs.push((j, format!("{rel}/")));
                }
            }
            fs.sort_by(|x, y| x.0.cmp(&y.0));
            files.extend(fs.into_iter().map(|x| x.1));
            stack.extend(dirs.into_iter().rev());
        }
        Ok(Image {
            file: Arc::new(file),
            files,
        })
    }

    /// The range reader of one file.
    pub(crate) fn src(&self, it: &Item) -> Box<dyn Src> {
        Box::new(Range {
            file: self.file.clone(),
            off: it.off,
            size: it.size,
        })
    }
}
