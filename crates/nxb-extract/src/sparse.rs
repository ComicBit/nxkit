//! Sparse NCA sections (a nonzero sparse generation in the section's FS header, LibHac's `ExistsSparseLayer`).
//! The NCA stores only the ranges of the section that hold data. A bucket tree in the NCA maps the section's offsets
//! to those ranges (storage 0) or to zeros (storage 1). The mapped bytes are still encrypted as if the section were
//! complete, so the view returned here replaces the raw section window and the usual decryption goes on top of it.
//! The patch NCA of an update reads the base this way. A sparse layer carries no hash check of its own here. The
//! layer below it, the section's integrity tree, checks the data.

use std::sync::Arc;

use crate::bktr::bucket_entries;
use crate::error::{ExtractError, Result};
use crate::keys::KeySet;
use crate::nca::{Nca, u32le, u64le};
use crate::storage::{IndirectStorage, Relocation, Storage, SubStorage};

/// The largest sparse table that is read into memory.
const TABLE_MAX: u64 = 256 << 20;

/// A section whose bucket tree has no entries: all zeros.
struct ZeroStorage(u64);

impl Storage for ZeroStorage {
    fn size(&self) -> u64 {
        self.0
    }
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<()> {
        match offset.checked_add(buf.len() as u64) {
            Some(end) if end <= self.0 => {
                buf.fill(0);
                Ok(())
            }
            _ => Err(ExtractError::format("sparse section: read past the end")),
        }
    }
}

/// The raw (still encrypted) view of the sparse section `i` of `nca`, in the section's own offsets.
pub fn open(ks: &KeySet, nca: &Nca, i: usize) -> Result<Arc<dyn Storage>> {
    let (start, end, fs) = nca
        .sections
        .get(i)
        .and_then(|s| s.clone())
        .ok_or_else(|| ExtractError::format(format!("{}: no section {i}", nca.id)))?;
    let h = &fs.raw;
    let (bucket_off, bucket_size, phys_off) = (u64le(h, 0x148), u64le(h, 0x150), u64le(h, 0x168));
    let generation = u32le(h, 0x170) & 0xffff;
    // the bucket tree header: BKTR, a version of 1 at most and an entry count that is not negative
    let count = u32le(h, 0x160);
    if h.get(0x158..0x15c) != Some(b"BKTR") || u32le(h, 0x15c) > 1 || count > i32::MAX as u32 {
        return Err(ExtractError::format(format!(
            "{}: section {i} has an invalid sparse bucket tree header",
            nca.id
        )));
    }
    if count == 0 {
        return Ok(Arc::new(ZeroStorage(end.saturating_sub(start))));
    }
    let phys_size = bucket_off
        .checked_add(bucket_size)
        .filter(|_| bucket_size <= TABLE_MAX)
        .ok_or_else(|| ExtractError::format("sparse table outside its section"))?;
    // the physical storage: the data ranges, then the table that maps them
    let body: Arc<dyn Storage> = Arc::new(SubStorage::new(nca.raw.clone(), phys_off, phys_size)?);
    let mut table = body.read_vec(bucket_off, bucket_size as usize)?;
    // the table is encrypted with the section's counter and the sparse generation in its low 32 bits
    let upper = (u64::from_be_bytes(fs.upper) & !0xffff_ffff) | u64::from(generation) << 16;
    nca.ctr_key(ks)?
        .ctr(upper.to_be_bytes(), phys_off + bucket_off, &mut table);
    let (entries, table_end) = bucket_entries(&table, 0x14, count)?;
    let entries = entries
        .iter()
        .map(|e| Relocation {
            virt: u64le(e, 0),
            phys: u64le(e, 8),
            source: u32le(e, 16),
        })
        .collect();
    let zeros: Arc<dyn Storage> = Arc::new(ZeroStorage(u64::MAX));
    Ok(Arc::new(IndirectStorage::new(
        entries, table_end, body, zeros,
    )?))
}
