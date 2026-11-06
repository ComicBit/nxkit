//! A RomFS the source checks read from: a `RomFS.img` (read by ranges) or a folder. The checks of
//! `profile_guess.py` `build_sources` look into the files, so a manifest with the first 32 bytes is not enough.

use super::survey_extract::image_files;
use nxb_extract::storage::{FileStorage, Storage};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// One file of the RomFS.
pub struct RomFile {
    /// The path from the root, starting with `/`.
    pub path: String,
    /// The size in bytes.
    pub size: u64,
    at: Place,
}

enum Place {
    Image(u64),
    Disk(PathBuf),
    #[cfg(test)]
    Mem(Vec<u8>),
}

/// The files of a RomFS.
pub struct Rom {
    files: Vec<RomFile>,
    image: Option<FileStorage>,
}

impl Rom {
    /// Opens an image file or a folder.
    pub fn open(path: &Path) -> Result<Rom, String> {
        if path.is_dir() {
            let mut files = Vec::new();
            walk(path, path, &mut files)?;
            files.sort_by(|a, b| a.path.cmp(&b.path));
            return Ok(Rom { files, image: None });
        }
        let s = FileStorage::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut files: Vec<RomFile> = image_files(&s, path)?
            .into_iter()
            .map(|e| RomFile {
                path: String::from_utf8_lossy(&e.path).into_owned(),
                size: e.size,
                at: Place::Image(e.offset),
            })
            .collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(Rom {
            files,
            image: Some(s),
        })
    }

    /// A RomFS of files in memory.
    #[cfg(test)]
    pub fn in_memory(files: Vec<(&str, Vec<u8>)>) -> Rom {
        let mut files: Vec<RomFile> = files
            .into_iter()
            .map(|(p, b)| RomFile {
                path: p.to_string(),
                size: b.len() as u64,
                at: Place::Mem(b),
            })
            .collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Rom { files, image: None }
    }

    /// Every file, in path order.
    pub fn files(&self) -> &[RomFile] {
        &self.files
    }

    /// Up to `n` bytes of `f` from `off`: fewer when the file ends (as Python's `f.read`).
    pub fn read(&self, f: &RomFile, off: u64, n: u64) -> Result<Vec<u8>, String> {
        let len = n.min(f.size.saturating_sub(off));
        let len = usize::try_from(len).map_err(|_| format!("{}: too large", f.path))?;
        match (&f.at, &self.image) {
            (Place::Image(base), Some(s)) => s
                .read_vec(base + off.min(f.size), len)
                .map_err(|e| format!("{}: {e}", f.path)),
            (Place::Disk(p), _) => {
                let bad = |e: std::io::Error| format!("{}: {e}", p.display());
                let mut h = std::fs::File::open(p).map_err(bad)?;
                h.seek(SeekFrom::Start(off)).map_err(bad)?;
                let mut v = vec![0u8; len];
                h.read_exact(&mut v).map_err(bad)?;
                Ok(v)
            }
            #[cfg(test)]
            (Place::Mem(b), _) => {
                let a = usize::try_from(off.min(f.size)).unwrap_or(0);
                Ok(b[a..a + len].to_vec())
            }
            _ => Err(format!("{}: no image", f.path)),
        }
    }

    /// The first `n` bytes of `f`.
    pub fn head(&self, f: &RomFile, n: u64) -> Result<Vec<u8>, String> {
        self.read(f, 0, n)
    }
}

fn walk(root: &Path, d: &Path, out: &mut Vec<RomFile>) -> Result<(), String> {
    let rd = std::fs::read_dir(d).map_err(|e| format!("{}: {e}", d.display()))?;
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(root, &p, out)?;
        } else if let Ok(m) = p.metadata() {
            let rel = p.strip_prefix(root).map_err(|e| e.to_string())?;
            out.push(RomFile {
                path: format!("/{}", rel.to_string_lossy()),
                size: m.len(),
                at: Place::Disk(p),
            });
        }
    }
    Ok(())
}

/// The folder of a path (`os.path.dirname`).
pub fn dirname(p: &str) -> &str {
    p.rfind('/').map_or("", |i| &p[..i])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_lists_its_files_by_path_and_reads_by_range() {
        let d = std::env::temp_dir().join(format!("nxb-rom-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("a")).unwrap();
        std::fs::write(d.join("a/x.bin"), b"0123456789").unwrap();
        std::fs::write(d.join("b.bin"), b"zz").unwrap();
        let r = Rom::open(&d).unwrap();
        let paths: Vec<_> = r.files().iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["/a/x.bin", "/b.bin"]);
        assert_eq!(r.read(&r.files()[0], 8, 100).unwrap(), b"89");
        assert_eq!(r.head(&r.files()[0], 3).unwrap(), b"012");
        assert!(r.read(&r.files()[0], 20, 4).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_folder_of_a_path() {
        assert_eq!(dirname("/Data/x.pak"), "/Data");
        assert_eq!(dirname("/x.pak"), "");
    }
}
