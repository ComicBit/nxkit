//! Content metas (`*.cnmt` in Meta NCAs) and `OpenNsp`'s filter: a merged package (base game + update + DLC in one
//! NSP, a cartridge dump with its update) is opened once as the base game (its Application meta's NCAs) and once as
//! the update (its Patch meta's NCAs).

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::cache::BlockCache;
use crate::error::{ExtractError, Result};
use crate::ivfc::read_sha256_section;
use crate::keys::KeySet;
use crate::nca::{ContentType, Nca, read_pfs0, u32le, u64le};
use crate::storage::{MemStorage, Storage};
use crate::version::plain_section;

/// Content meta types used here.
pub const APPLICATION: u8 = 0x80;
/// Patch (update).
pub const PATCH: u8 = 0x81;

/// One content meta.
#[derive(Clone, Debug)]
pub struct Cnmt {
    /// Meta type (0x80 application, 0x81 patch, 0x82 add-on content, ...).
    pub kind: u8,
    /// Title ID.
    pub title: u64,
    /// Title version.
    pub version: u32,
    /// Content IDs (lower-case hex) of the NCAs it lists.
    pub contents: Vec<String>,
}

fn parse(d: &[u8]) -> Result<Cnmt> {
    let ext = d
        .get(0xE..0x10)
        .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)
        .ok_or_else(|| ExtractError::format("cnmt header"))?;
    let n = d
        .get(0x10..0x12)
        .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)
        .unwrap_or(0);
    let mut contents = Vec::with_capacity(n);
    for i in 0..n {
        let e = 0x20 + ext + 0x38 * i;
        let id = d
            .get(e + 0x20..e + 0x30)
            .ok_or_else(|| ExtractError::format("cnmt content entry outside the file"))?;
        contents.push(nxb_model::to_hex(id));
    }
    Ok(Cnmt {
        kind: d.get(0xC).copied().unwrap_or(0),
        title: u64le(d, 0),
        version: u32le(d, 8),
        contents,
    })
}

/// The content metas of a package's Meta NCAs, in NCA order.
pub fn metas(ks: &KeySet, ncas: &[Nca], cache: &Arc<BlockCache>) -> Result<Vec<Cnmt>> {
    let mut out = Vec::new();
    for n in ncas.iter().filter(|n| n.content_type == ContentType::Meta) {
        let (sec, fs) = plain_section(ks, n, 0, cache)?;
        let data = MemStorage(read_sha256_section(sec.as_ref(), &fs)?);
        for e in read_pfs0(&data)? {
            if e.name.ends_with(".cnmt") {
                out.push(parse(&data.read_vec(e.offset, e.size as usize)?)?);
            }
        }
    }
    Ok(out)
}

/// `OpenNsp` with a content meta type: keeps the NCAs the package's metas of that type (and title) list, all of them
/// when no meta of the type lists any, or when none of the listed ones is a Program (renamed NCA files). Returns the
/// log lines `nx_extract` prints.
pub fn filter(
    ks: &KeySet,
    ncas: Vec<Nca>,
    kind: u8,
    title: Option<u64>,
    file: &str,
    cache: &Arc<BlockCache>,
) -> Result<(Vec<Nca>, String)> {
    let what = if kind == APPLICATION {
        "Application"
    } else {
        "Patch"
    };
    let mut listed = BTreeSet::new();
    let mut titles = Vec::new();
    for c in metas(ks, &ncas, cache)? {
        if c.kind != kind {
            continue;
        }
        titles.push(c.title);
        if title.is_none_or(|t| t == c.title) {
            listed.extend(c.contents);
        }
    }
    if let Some(t) = title
        && !titles.is_empty()
        && !titles.contains(&t)
    {
        let names: Vec<String> = titles.iter().map(|x| format!("{x:016x}")).collect();
        return Err(ExtractError::format(format!(
            "{file} has no {what} content meta of title {t:016x}; it has {}",
            names.join(", ")
        )));
    }
    if listed.is_empty() {
        return Ok((ncas, String::new()));
    }
    let keep = |n: &Nca| listed.contains(&n.id);
    if !ncas
        .iter()
        .any(|n| keep(n) && n.content_type == ContentType::Program)
    {
        let log = format!(
            "[RED] no NCA of {file} matches its {what} content meta by name (renamed NCA files?); reading all {}\n",
            ncas.len()
        );
        return Ok((ncas, log));
    }
    let other = ncas
        .iter()
        .filter(|n| !keep(n) && n.content_type != ContentType::Meta)
        .count();
    let kept: Vec<Nca> = ncas.into_iter().filter(|n| keep(n)).collect();
    let log = if other > 0 {
        format!(
            "leaving out {other} NCAs of {file} that its {what} content meta does not list (another title in the package: an update on the cartridge, or a merged NSP)\n"
        )
    } else {
        String::new()
    };
    Ok((kept, log))
}
