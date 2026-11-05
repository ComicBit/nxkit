//! RomFS tables (LibHac's `RomFsFileSystem`): the directory and file metadata tables, walked from the root.

use crate::error::{ExtractError, Result};
use crate::nca::{u32le, u64le};
use crate::storage::Storage;

const NONE: u32 = 0xFFFF_FFFF;

/// A file or directory of a RomFS.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RomEntry {
    /// Full path from the root, starting with `/` (raw name bytes, UTF-8 in practice).
    pub path: Vec<u8>,
    /// Whether it is a directory.
    pub dir: bool,
    /// File size (0 for directories).
    pub size: u64,
    /// File data offset from the RomFS start (0 for directories).
    pub offset: u64,
}

/// Walks the tables of the RomFS in `s` and returns every entry except the root (depth-first, unordered: the image plan
/// sorts).
pub fn read_tables(s: &dyn Storage) -> Result<Vec<RomEntry>> {
    let h = s.read_vec(0, 0x50)?;
    let hs = u64le(&h, 0);
    if hs != 0x50 {
        return Err(ExtractError::format("RomFS header size"));
    }
    let (dm_off, dm_size) = (u64le(&h, 0x18), u64le(&h, 0x20));
    let (fm_off, fm_size) = (u64le(&h, 0x38), u64le(&h, 0x40));
    let data = u64le(&h, 0x48);
    if dm_size > (512 << 20) || fm_size > (512 << 20) {
        return Err(ExtractError::format("RomFS tables too large"));
    }
    let dirs = s.read_vec(dm_off, dm_size as usize)?;
    let files = s.read_vec(fm_off, fm_size as usize)?;
    let mut out = Vec::new();
    // (dir offset, path)
    let mut stack: Vec<(u32, Vec<u8>)> = vec![(0, Vec::new())];
    let mut guard = 0usize;
    let limit = dirs.len() / 0x18 + files.len() / 0x20 + 2;
    while let Some((d, path)) = stack.pop() {
        let de = d as usize;
        if de + 0x18 > dirs.len() {
            return Err(ExtractError::format(
                "RomFS directory entry outside its table",
            ));
        }
        let mut child = u32le(&dirs, de + 8);
        while child != NONE {
            guard += 1;
            if guard > limit {
                return Err(ExtractError::format("RomFS directory loop"));
            }
            let c = child as usize;
            let name = entry_name(&dirs, c, 0x18)?;
            let mut p = path.clone();
            p.push(b'/');
            p.extend_from_slice(name);
            out.push(RomEntry {
                path: p.clone(),
                dir: true,
                size: 0,
                offset: 0,
            });
            stack.push((child, p));
            child = u32le(&dirs, c + 4);
        }
        let mut f = u32le(&dirs, de + 12);
        while f != NONE {
            guard += 1;
            if guard > limit {
                return Err(ExtractError::format("RomFS file loop"));
            }
            let fe = f as usize;
            let name = entry_name(&files, fe, 0x20)?;
            let mut p = path.clone();
            p.push(b'/');
            p.extend_from_slice(name);
            let (offset, size) = (u64le(&files, fe + 8), u64le(&files, fe + 16));
            let abs = data
                .checked_add(offset)
                .filter(|a| a.checked_add(size).is_some_and(|e| e <= s.size()));
            let abs =
                abs.ok_or_else(|| ExtractError::format("RomFS file data outside the RomFS"))?;
            out.push(RomEntry {
                path: p,
                dir: false,
                size,
                offset: abs,
            });
            f = u32le(&files, fe + 4);
        }
    }
    Ok(out)
}

fn entry_name(table: &[u8], at: usize, fixed: usize) -> Result<&[u8]> {
    let len = u32le(table, at + fixed - 4) as usize;
    table
        .get(at + fixed..at + fixed + len)
        .ok_or_else(|| ExtractError::format("RomFS name outside its table"))
}

/// Reads one file by path (control data: `/control.nacp`, icons).
pub fn read_file(s: &dyn Storage, entries: &[RomEntry], path: &str) -> Result<Option<Vec<u8>>> {
    match entries.iter().find(|e| !e.dir && e.path == path.as_bytes()) {
        Some(e) => Ok(Some(s.read_vec(e.offset, e.size as usize)?)),
        None => Ok(None),
    }
}
