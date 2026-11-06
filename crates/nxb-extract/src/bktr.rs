//! BKTR (LibHac's `BucketTree`, `IndirectStorage`, `AesCtrCounterExtendedStorage`): the update's RomFS section is a
//! relocation table over the base section and the patch's own data, which is AES-CTR encrypted with a generation per
//! physical range. Both tables sit at the end of the patch section, encrypted with the section's plain counter.

use std::sync::Arc;

use crate::aes::Aes128;
use crate::cache::BlockCache;
use crate::error::{ExtractError, Result};
use crate::nca::{FsHeader, u32le, u64le};
use crate::storage::{Counter, CtrSection, IndirectStorage, Relocation, Storage};

const NODE: usize = 0x4000;

/// The fixed-size entries of a bucket tree with one offset node (L1) and its entry-set nodes.
pub(crate) fn bucket_entries(
    table: &[u8],
    entry_size: usize,
    expected: u32,
) -> Result<(Vec<&[u8]>, u64)> {
    if table.len() < NODE {
        return Err(ExtractError::format("BKTR table shorter than one node"));
    }
    let sets = u32le(table, 4) as usize;
    let end = u64le(table, 8);
    if sets > (NODE - 16) / 8 {
        return Err(ExtractError::Unsupported(
            "BKTR tables with a second offset level".into(),
        ));
    }
    let mut out = Vec::with_capacity(expected as usize);
    for s in 0..sets {
        let node = table
            .get(NODE * (1 + s)..NODE * (2 + s))
            .ok_or_else(|| ExtractError::format("BKTR entry set outside the table"))?;
        let count = u32le(node, 4) as usize;
        if 16 + count * entry_size > NODE {
            return Err(ExtractError::format("BKTR entry count"));
        }
        for i in 0..count {
            out.push(&node[16 + i * entry_size..16 + (i + 1) * entry_size]);
        }
    }
    if out.len() != expected as usize {
        return Err(ExtractError::format(format!(
            "BKTR: {} entries, header says {expected}",
            out.len()
        )));
    }
    Ok((out, end))
}

/// Builds the patched section of an update: `base` is the base NCA's decrypted data section, `raw` the patch NCA's
/// raw (encrypted) data section, `ctr_base` its offset in the patch NCA.
pub fn patched_section(
    base: Arc<dyn Storage>,
    raw: Arc<dyn Storage>,
    ctr_base: u64,
    fs: &FsHeader,
    key: impl Fn() -> Result<Aes128>,
    cache: Arc<BlockCache>,
) -> Result<(IndirectStorage, Arc<CtrSection>)> {
    let h = &fs.raw;
    let (ind_off, ind_size) = (u64le(h, 0x100), u64le(h, 0x108));
    let (ex_off, ex_size) = (u64le(h, 0x120), u64le(h, 0x128));
    if h.get(0x110..0x114) != Some(b"BKTR") || h.get(0x130..0x134) != Some(b"BKTR") {
        return Err(ExtractError::format("patch section without BKTR headers"));
    }
    let (ind_count, ex_count) = (u32le(h, 0x118), u32le(h, 0x138));
    if ind_size > (256 << 20) || ex_size > (256 << 20) {
        return Err(ExtractError::format("BKTR tables too large"));
    }
    // the tables, decrypted with the section's plain counter
    let plain = CtrSection::new(
        raw.clone(),
        key()?,
        ctr_base,
        Counter::Upper(fs.upper),
        cache.clone(),
    );
    let ind_table = plain.read_vec(ind_off, ind_size as usize)?;
    let ex_table = plain.read_vec(ex_off, ex_size as usize)?;

    let (ex, ex_end) = bucket_entries(&ex_table, 0x10, ex_count)?;
    let mut ranges = Vec::with_capacity(ex.len() + 1);
    for e in ex {
        let off = u64le(e, 0);
        let encryption = e[8];
        let generation = u32le(e, 12);
        let mut upper = fs.upper;
        upper[4..8].copy_from_slice(&generation.to_be_bytes());
        ranges.push((
            off,
            if encryption == 1 {
                Counter::Plain
            } else {
                Counter::Upper(upper)
            },
        ));
    }
    if ranges.first().map(|r| r.0) != Some(0) || ranges.windows(2).any(|w| w[0].0 >= w[1].0) {
        return Err(ExtractError::format("BKTR counter table not sorted from 0"));
    }
    // past the counter table's end (the tables themselves): the plain counter
    ranges.push((ex_end, Counter::Upper(fs.upper)));
    let patch = Arc::new(CtrSection::with_ranges(
        raw,
        key()?,
        ctr_base,
        ranges,
        cache,
    ));

    let (ind, ind_end) = bucket_entries(&ind_table, 0x14, ind_count)?;
    let entries = ind
        .iter()
        .map(|e| Relocation {
            virt: u64le(e, 0),
            phys: u64le(e, 8),
            source: u32le(e, 16),
        })
        .collect();
    let patch_dyn: Arc<dyn Storage> = patch.clone();
    Ok((
        IndirectStorage::new(entries, ind_end, base, patch_dyn)?,
        patch,
    ))
}
