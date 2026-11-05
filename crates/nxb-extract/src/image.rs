//! The host's `DMROMFS1` RomFS image (`WriteImage` in `nx_extract`): a 32-byte header, every file's bytes in entry order
//! without padding, then the index (24 bytes per entry) and the name table. Entries are breadth-first from the root,
//! each directory's children sorted by name bytes. All offsets follow from the tables, so the plan is complete before
//! any data is read and workers can write files at their offsets in any order.

use std::collections::BTreeMap;

use crate::error::{ExtractError, Result};
use crate::romfs::RomEntry;

/// One entry of the image index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanEntry {
    /// Full path (`/` for the root).
    pub path: Vec<u8>,
    /// Name bytes (empty for the root).
    pub name: Vec<u8>,
    /// Directory or file.
    pub dir: bool,
    /// File: offset in the image, directory: index of the first child.
    pub a: u64,
    /// File: size, directory: number of children.
    pub b: u64,
    /// File: data offset in the source RomFS.
    pub src: u64,
    /// The damaged flag (always false without `--allow-damaged`).
    pub damaged: bool,
}

/// The complete layout of an image.
#[derive(Clone, Debug)]
pub struct ImagePlan {
    /// Entries in index order.
    pub entries: Vec<PlanEntry>,
    /// Offset of the index (= 32 + all file bytes).
    pub index_offset: u64,
}

impl ImagePlan {
    /// Plans the image of a RomFS's entries.
    pub fn plan(rom: &[RomEntry]) -> Result<ImagePlan> {
        let mut children: BTreeMap<Vec<u8>, Vec<usize>> = BTreeMap::new();
        for (i, e) in rom.iter().enumerate() {
            let cut = e.path.iter().rposition(|&c| c == b'/').unwrap_or(0);
            let name_len = e.path.len() - cut - 1;
            if name_len == 0 || name_len > 0x300 {
                return Err(ExtractError::format(format!(
                    "invalid RomFS name length: {}",
                    String::from_utf8_lossy(&e.path)
                )));
            }
            let parent = if cut == 0 {
                b"/".to_vec()
            } else {
                e.path[..cut].to_vec()
            };
            children.entry(parent).or_default().push(i);
        }
        let mut entries = vec![PlanEntry {
            path: b"/".to_vec(),
            name: Vec::new(),
            dir: true,
            a: 0,
            b: 0,
            src: 0,
            damaged: false,
        }];
        let mut i = 0;
        while i < entries.len() {
            if entries[i].dir {
                let mut kids: Vec<PlanEntry> = children
                    .get(&entries[i].path)
                    .map(|v| {
                        v.iter()
                            .map(|&k| {
                                let e = &rom[k];
                                let cut = e.path.iter().rposition(|&c| c == b'/').unwrap_or(0);
                                PlanEntry {
                                    path: e.path.clone(),
                                    name: e.path[cut + 1..].to_vec(),
                                    dir: e.dir,
                                    a: 0,
                                    b: e.size,
                                    src: e.offset,
                                    damaged: false,
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                kids.sort_by(|x, y| x.name.cmp(&y.name));
                entries[i].a = entries.len() as u64;
                entries[i].b = kids.len() as u64;
                entries.extend(kids);
            }
            i += 1;
        }
        let mut pos = 32u64;
        for e in entries.iter_mut().filter(|e| !e.dir) {
            e.a = pos;
            pos += e.b;
        }
        Ok(ImagePlan {
            entries,
            index_offset: pos,
        })
    }

    /// Files and bytes of file data.
    pub fn totals(&self) -> (u64, u64) {
        let files = self.entries.iter().filter(|e| !e.dir).count() as u64;
        (files, self.index_offset - 32)
    }

    /// The index and name table (written at `index_offset`).
    pub fn index_bytes(&self) -> Result<Vec<u8>> {
        let mut idx = Vec::with_capacity(self.entries.len() * 24);
        let mut names = Vec::new();
        for e in &self.entries {
            let no = u32::try_from(names.len())
                .map_err(|_| ExtractError::format("image name table over 4 GiB"))?;
            let nl = u16::try_from(e.name.len())
                .map_err(|_| ExtractError::format("image name too long"))?;
            idx.extend_from_slice(&e.a.to_le_bytes());
            idx.extend_from_slice(&e.b.to_le_bytes());
            idx.extend_from_slice(&no.to_le_bytes());
            idx.extend_from_slice(&nl.to_le_bytes());
            idx.push(if e.dir { 0 } else { 1 });
            idx.push(u8::from(e.damaged));
            names.extend_from_slice(&e.name);
        }
        idx.extend_from_slice(&names);
        Ok(idx)
    }

    /// The 32-byte header for an index of `index_len` bytes.
    pub fn header(&self, index_len: u64) -> [u8; 32] {
        let mut h = [0u8; 32];
        h[..8].copy_from_slice(b"DMROMFS1");
        h[8..16].copy_from_slice(&self.index_offset.to_le_bytes());
        h[16..24].copy_from_slice(&index_len.to_le_bytes());
        h[24..32].copy_from_slice(&(self.entries.len() as u64).to_le_bytes());
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(p: &str, size: u64) -> RomEntry {
        RomEntry {
            path: p.as_bytes().to_vec(),
            dir: false,
            size,
            offset: 0,
        }
    }
    fn d(p: &str) -> RomEntry {
        RomEntry {
            path: p.as_bytes().to_vec(),
            dir: true,
            size: 0,
            offset: 0,
        }
    }

    #[test]
    fn breadth_first_sorted() {
        let rom = vec![f("/b", 3), d("/a"), f("/a/z", 2), f("/a/Y", 1), f("/C", 5)];
        let p = ImagePlan::plan(&rom).unwrap_or_else(|_| ImagePlan {
            entries: vec![],
            index_offset: 0,
        });
        let names: Vec<&[u8]> = p.entries.iter().map(|e| e.name.as_slice()).collect();
        assert_eq!(names, vec![&b""[..], b"C", b"a", b"b", b"Y", b"z"]);
        assert_eq!((p.entries[0].a, p.entries[0].b), (1, 3));
        assert_eq!((p.entries[2].a, p.entries[2].b), (4, 2));
        // file data in entry order: C(5) b(3) Y(1) z(2)
        assert_eq!(p.entries[1].a, 32);
        assert_eq!(p.entries[3].a, 37);
        assert_eq!(p.entries[4].a, 40);
        assert_eq!(p.index_offset, 43);
        let idx = p.index_bytes().unwrap_or_default();
        assert_eq!(idx.len(), 6 * 24 + 5);
    }
}
