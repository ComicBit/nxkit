//! `romfs.py` (the marker check of the iOS flow) and `romfs_image.py --identity`: reads of a `DMROMFS1` image.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const MAGIC: &[u8; 8] = b"DMROMFS1";
const HEAD: usize = 32;
const ENT: usize = 24;

fn le64(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap_or([0; 8]))
}

/// A validated image index (`romfs.Image`).
struct Image {
    index: Vec<u8>,
    pool: usize,
}

impl Image {
    fn ent(&self, k: usize) -> (u64, u64, usize, usize, u8, u8) {
        let e = &self.index[k * ENT..];
        (
            le64(e, 0),
            le64(e, 8),
            u32::from_le_bytes([e[16], e[17], e[18], e[19]]) as usize,
            u16::from_le_bytes([e[20], e[21]]) as usize,
            e[22],
            e[23],
        )
    }
    fn name(&self, k: usize) -> &[u8] {
        let (_, _, no, nl, _, _) = self.ent(k);
        &self.index[self.pool + no..self.pool + no + nl]
    }

    fn open(path: &Path) -> Result<Image, String> {
        let bad = |m: &str| format!("{m}: {}", path.display());
        let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let total = f.metadata().map_err(|e| e.to_string())?.len();
        let mut head = [0u8; HEAD];
        f.read_exact(&mut head)
            .map_err(|_| bad("invalid RomFS image header"))?;
        let (off, size, count) = (le64(&head, 8), le64(&head, 16), le64(&head, 24));
        if &head[..8] != MAGIC
            || count == 0
            || count > 0xffff_ffff
            || off < HEAD as u64
            || off > total
            || size != total - off
            || count > size / ENT as u64
        {
            return Err(bad("invalid RomFS image header"));
        }
        f.seek(SeekFrom::Start(off)).map_err(|e| e.to_string())?;
        let mut index = vec![0u8; size as usize];
        f.read_exact(&mut index)
            .map_err(|_| bad("incomplete RomFS image"))?;
        let count = count as usize;
        let im = Image {
            index,
            pool: count * ENT,
        };
        let size = size as usize;
        let mut next_child = 1u64;
        for k in 0..count {
            let (a, b, no, nl, t, flags) = im.ent(k);
            if no + nl > size - im.pool || t > 1 || flags > t || (k == 0 && (t != 0 || nl != 0)) {
                return Err(bad(&format!("invalid RomFS image entry {k}")));
            }
            let name = im.name(k);
            if k != 0
                && (nl == 0
                    || nl > 0x300
                    || name.contains(&0)
                    || name.contains(&b'/')
                    || name == b"."
                    || name == b"..")
            {
                return Err(bad("invalid RomFS image name"));
            }
            if t != 0 {
                if a < HEAD as u64 || a > off || b > off - a {
                    return Err(bad("invalid RomFS image file range"));
                }
            } else {
                if a != next_child || a <= k as u64 || a > count as u64 || b > count as u64 - a {
                    return Err(bad("invalid RomFS image tree"));
                }
                next_child += b;
            }
        }
        if next_child != count as u64 {
            return Err(bad("unreachable RomFS image entries"));
        }
        for k in 0..count {
            let (a, b, _, _, t, _) = im.ent(k);
            if t == 0 && (a + 1..a + b).any(|j| im.name(j as usize - 1) >= im.name(j as usize)) {
                return Err(bad("unsorted or duplicate RomFS image names"));
            }
        }
        Ok(im)
    }

    /// `Image.find`: whether the relative path names an entry.
    fn exists(&self, relative: &str) -> bool {
        let mut k = 0;
        for part in relative.split('/') {
            if part.is_empty() || part == "." {
                continue;
            }
            let (a, b, _, _, t, _) = self.ent(k);
            if t != 0 {
                return false;
            }
            let (mut lo, mut hi) = (a as usize, (a + b) as usize);
            while lo < hi {
                let mid = (lo + hi) / 2;
                if self.name(mid) < part.as_bytes() {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }
            if lo == (a + b) as usize || self.name(lo) != part.as_bytes() {
                return false;
            }
            k = lo;
        }
        true
    }
}

/// `romfs.py <root> <marker>`: a RomFS folder, or an image (`*.img`) that holds the marker.
pub fn check(root: &Path, marker: &str) -> Result<(), String> {
    let miss = || format!("missing RomFS marker '{marker}' in {}", root.display());
    if root.is_dir() {
        return if root.join(marker).exists() {
            Ok(())
        } else {
            Err(miss())
        };
    }
    let is_img = root.is_file()
        && root.extension().is_some_and(|e| e == "img")
        && std::fs::File::open(root)
            .and_then(|mut f| {
                let mut m = [0u8; 8];
                f.read_exact(&mut m).map(|_| &m == MAGIC)
            })
            .unwrap_or(false);
    if is_img && Image::open(root)?.exists(marker) {
        return Ok(());
    }
    Err(miss())
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x100_0000_01b3;

fn clean(text: &str) -> String {
    let s: String = text
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || "._-+".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() { "-".into() } else { s }
}

/// `romfs_image.identity`: size and the FNV-1a 64 hash of the header and index of a complete image.
pub fn identity(img: &Path, title: &str, version: &str, build: &str) -> Result<String, String> {
    let mut f = std::fs::File::open(img).map_err(|e| format!("{}: {e}", img.display()))?;
    let size = f.metadata().map_err(|e| e.to_string())?.len();
    let mut head = [0u8; HEAD];
    let bad = || format!("not a RomFS image: {}", img.display());
    f.read_exact(&mut head).map_err(|_| bad())?;
    let (ioff, isz) = (le64(&head, 8), le64(&head, 16));
    if &head[..8] != MAGIC || ioff < HEAD as u64 || ioff > size || isz != size - ioff {
        return Err(format!(
            "incomplete or invalid RomFS image: {}",
            img.display()
        ));
    }
    let mut h = FNV_OFFSET;
    let mut mix = |data: &[u8]| {
        for w in data.chunks(8) {
            let mut b = [0u8; 8];
            b[..w.len()].copy_from_slice(w);
            h = (h ^ u64::from_le_bytes(b)).wrapping_mul(FNV_PRIME);
        }
    };
    mix(&head);
    f.seek(SeekFrom::Start(ioff)).map_err(|e| e.to_string())?;
    let mut rest: Vec<u8> = vec![];
    let mut left = isz as usize;
    let mut buf = vec![0u8; 8 << 20];
    while left > 0 {
        let n = f
            .read(&mut buf[..left.min(8 << 20)])
            .map_err(|e| e.to_string())?;
        if n == 0 {
            return Err(format!("incomplete RomFS image: {}", img.display()));
        }
        left -= n;
        rest.extend_from_slice(&buf[..n]);
        let keep = if left > 0 { rest.len() % 8 } else { 0 };
        let tail = rest.split_off(rest.len() - keep);
        mix(&rest);
        rest = tail;
    }
    Ok(format!(
        "DMROMFSID1 title={} version={} build={} size={size} index={h:016x}",
        clean(title),
        clean(version),
        clean(build)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ent(a: u64, b: u64, no: u32, nl: u16, t: u8) -> Vec<u8> {
        let mut e = vec![];
        e.extend(a.to_le_bytes());
        e.extend(b.to_le_bytes());
        e.extend(no.to_le_bytes());
        e.extend(nl.to_le_bytes());
        e.extend([t, 0]);
        e
    }

    /// root { Data { x }, y }: the entries breadth-first, children sorted.
    fn image(path: &Path) {
        let data = b"hello";
        let names = b"Datayx";
        let mut idx = vec![];
        idx.extend(ent(1, 2, 0, 0, 0)); // root: Data, y
        idx.extend(ent(3, 1, 0, 4, 0)); // Data: x
        idx.extend(ent(32, 5, 4, 1, 1)); // y
        idx.extend(ent(32, 5, 5, 1, 1)); // x
        let mut f = vec![];
        f.extend(MAGIC);
        f.extend(((HEAD + data.len()) as u64).to_le_bytes());
        f.extend(((idx.len() + names.len()) as u64).to_le_bytes());
        f.extend(4u64.to_le_bytes());
        f.extend(data);
        f.extend(idx);
        f.extend(names);
        std::fs::write(path, f).unwrap();
    }

    #[test]
    fn marker_in_an_image_and_its_identity() {
        let d = std::env::temp_dir().join(format!("nxb-ios-romfscheck-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("RomFS.img");
        image(&p);
        assert!(check(&p, "Data/x").is_ok());
        assert!(check(&p, "y").is_ok());
        assert!(check(&p, "Data/y").is_err());
        let id = identity(&p, "A game", "1.0", "").unwrap();
        assert!(id.starts_with("DMROMFSID1 title=A_game version=1.0 build=- size="));
        std::fs::create_dir_all(d.join("dir/Data")).unwrap();
        std::fs::write(d.join("dir/Data/x"), b"").unwrap();
        assert!(check(&d.join("dir"), "Data/x").is_ok());
        assert!(check(&d.join("dir"), "Nope").is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Oracle: `NXB_ORACLE_ENGINE`, `NXB_ORACLE_PYTHON`, `NXB_ORACLE_ROMFS_IMG` (a `RomFS.img`): the identity line.
    #[test]
    fn identity_equals_the_reference_script() {
        let (Some(e), Some(py), Some(img)) = (
            std::env::var_os("NXB_ORACLE_ENGINE"),
            std::env::var_os("NXB_ORACLE_PYTHON"),
            std::env::var_os("NXB_ORACLE_ROMFS_IMG"),
        ) else {
            return;
        };
        let o = std::process::Command::new(py)
            .arg("-I")
            .arg(Path::new(&e).join("dm/tools/romfs_image.py"))
            .arg("--identity")
            .arg(&img)
            .args(["Sample", "1.2.1", "pkg"])
            .output()
            .unwrap();
        assert_eq!(
            identity(Path::new(&img), "Sample", "1.2.1", "pkg").unwrap(),
            String::from_utf8(o.stdout).unwrap().trim_end()
        );
    }
}
