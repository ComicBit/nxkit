//! Standard zstd dictionaries (RFC 8878 section 5).

use super::huf::Huf;
use super::{DEFAULT_LIMIT, Fse, Fwd, R, SplitError, err, frames};

/// A zstd dictionary in the standard format (magic 0xEC30A437, ID, entropy tables, repeat offsets, content).
pub(super) struct Dict {
    pub(super) huf: Option<Huf>,
    pub(super) ll: Fse,
    pub(super) of: Fse,
    pub(super) ml: Fse,
    pub(super) rep: [usize; 3],
    pub(super) content: Vec<u8>,
}

/// Dictionaries by ID, for frames that name one (one title ships them in `Pack/ZsDic.pack.zs`).
#[derive(Default)]
pub struct Dicts(pub(super) std::collections::HashMap<u32, Dict>);

impl Dicts {
    /// No dictionary.
    pub fn new() -> Dicts {
        Dicts::default()
    }

    /// Whether the dictionary `id` was added.
    pub fn contains(&self, id: u32) -> bool {
        self.0.contains_key(&id)
    }

    /// Whether no dictionary was added.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Parses and adds a dictionary file. Returns its ID.
    pub fn add(&mut self, raw: &[u8]) -> R<u32> {
        if raw.get(..4) != Some(&[0x37, 0xa4, 0x30, 0xec]) {
            return Err(err("zstd dictionary magic"));
        }
        let id = u32::from_le_bytes(
            raw.get(4..8)
                .and_then(|b| b.try_into().ok())
                .ok_or_else(|| err("zstd dictionary header"))?,
        );
        let mut at = 8usize;
        let (huf, used) = Huf::read(raw.get(at..).unwrap_or(&[]))?;
        at += used;
        let mut fse = |max_al: u32, max_sym: usize| -> R<Fse> {
            let mut f = Fwd {
                d: raw.get(at..).unwrap_or(&[]),
                pos: 0,
            };
            let t = Fse::read(&mut f, max_al, max_sym)?;
            at += f.bytes();
            Ok(t)
        };
        let of = fse(8, 31)?;
        let ml = fse(9, 52)?;
        let ll = fse(9, 35)?;
        let r = raw
            .get(at..at + 12)
            .ok_or_else(|| err("zstd dictionary repeat offsets"))?;
        let rd = |i: usize| u32::from_le_bytes([r[i], r[i + 1], r[i + 2], r[i + 3]]) as usize;
        let content = raw[at + 12..].to_vec();
        let rep = [rd(0), rd(4), rd(8)];
        if rep.iter().any(|&x| x == 0 || x > content.len()) {
            return Err(err("zstd dictionary repeat offsets"));
        }
        self.0.insert(
            id,
            Dict {
                huf: Some(huf),
                ll,
                of,
                ml,
                rep,
                content,
            },
        );
        Ok(id)
    }
}

/// Like [`super::decompress`] with dictionaries: a frame that names one that `dicts` lacks is a `Red` error (the text of the
/// reference step), as is output over [`DEFAULT_LIMIT`] an `Unsupported`.
pub fn decompress_dict(d: &[u8], dicts: &Dicts) -> R<Vec<u8>> {
    frames(d, DEFAULT_LIMIT, dicts, |n| {
        SplitError::Unsupported(format!("zstd output over {n} bytes"))
    })
}

/// The dictionary ID the first frame of `d` names (0: none, or not a zstd frame).
pub fn dict_id(d: &[u8]) -> u32 {
    if d.get(..4) != Some(&[0x28, 0xb5, 0x2f, 0xfd]) {
        return 0;
    }
    let Some(&fhd) = d.get(4) else { return 0 };
    let n = [0usize, 1, 2, 4][(fhd & 3) as usize];
    let o = 6 - usize::from(fhd >> 5 & 1 == 1);
    d.get(o..o + n).map_or(0, |b| {
        b.iter().rev().fold(0u32, |a, &x| a << 8 | u32::from(x))
    })
}
