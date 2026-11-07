//! Unreal Engine IoStore containers (`uepak.py`: `Toc`), read by ranges with no key.

use std::collections::HashMap;
use std::sync::Arc;

use super::R;
use super::uepak::{Rd, ve};
use crate::SplitError;
use crate::rd::{sl, u32le};
use crate::src::Src;

pub(super) struct Toc {
    rel: String,
    version: u8,
    pub(super) ids: Vec<[u8; 12]>,
    pub(super) ranges: Vec<(u64, u64)>,
    blocks: Vec<(u64, u64, u64, u8)>,
    methods: Vec<String>,
    block_size: u64,
    parts: u32,
    part_size: u64,
    pub(super) encrypted: bool,
    dir_index: Vec<u8>,
}

const TOC_MAGIC: &[u8; 16] = b"-==--==--==--==-";

impl Toc {
    pub(super) fn open(b: &[u8], rel: &str) -> R<Toc> {
        if b.get(..16) != Some(&TOC_MAGIC[..]) {
            return ve(format!("{rel}: not an IoStore TOC"));
        }
        let version = b[16];
        let w: Vec<usize> = (0..9).map(|i| u32le(b, 20 + 4 * i)).collect::<R<_>>()?;
        let (hsz, n, nblk, bsz, nmeth, mlen, block_size, dsz, nparts) =
            (w[0], w[1], w[2], w[3], w[4], w[5], w[6], w[7], w[8]);
        if bsz != 12 {
            return ve(format!("{rel}: compression block entries of {bsz} bytes"));
        }
        let flags = *b
            .get(80)
            .ok_or_else(|| SplitError::Red("IndexError".into()))?;
        let nseeds = if version >= 4 { u32le(b, 84)? } else { 0 };
        let part_size = if version >= 3 && nparts > 1 {
            crate::rd::u64le(b, 88)?
        } else {
            u64::MAX
        };
        let nover = if version >= 5 { u32le(b, 96)? } else { 0 };
        let mut o = hsz;
        let be = |a: usize, n: usize| {
            sl(b, a, a + n)
                .iter()
                .fold(0u64, |x, &y| x << 8 | u64::from(y))
        };
        let lle = |a: usize, n: usize| {
            sl(b, a, a + n)
                .iter()
                .rev()
                .fold(0u64, |x, &y| x << 8 | u64::from(y))
        };
        let ids: Vec<[u8; 12]> = (0..n)
            .map(|i| {
                let mut x = [0u8; 12];
                x.copy_from_slice(
                    sl(b, o + 12 * i, o + 12 * i + 12)
                        .get(..12)
                        .unwrap_or(&[0; 12]),
                );
                x
            })
            .collect();
        o += 12 * n;
        let ranges = (0..n)
            .map(|i| (be(o + 10 * i, 5), be(o + 10 * i + 5, 5)))
            .collect();
        o += 10 * n + 4 * nseeds + 4 * nover;
        let blocks = (0..nblk)
            .map(|i| {
                (
                    lle(o + 12 * i, 5),
                    lle(o + 12 * i + 5, 3),
                    lle(o + 12 * i + 8, 3),
                    *b.get(o + 12 * i + 11).unwrap_or(&0),
                )
            })
            .collect();
        o += 12 * nblk;
        let methods = (0..nmeth)
            .map(|i| {
                let s = sl(b, o + mlen * i, o + mlen * (i + 1));
                let end = s.iter().rposition(|&c| c != 0).map_or(0, |p| p + 1);
                s[..end].iter().map(|&c| c as char).collect()
            })
            .collect();
        o += mlen * nmeth;
        let dir_index = if flags & 8 != 0 && flags & 6 == 0 {
            sl(b, o, o + dsz).to_vec()
        } else {
            Vec::new()
        };
        Ok(Toc {
            rel: rel.to_string(),
            version,
            ids,
            ranges,
            blocks,
            methods,
            block_size: block_size as u64,
            parts: nparts as u32,
            part_size,
            encrypted: (flags >> 1) & 1 == 1,
            dir_index,
        })
    }

    pub(super) fn export_bundles(&self) -> Vec<usize> {
        if self.version < 4 {
            (0..self.ids.len())
                .filter(|&i| self.ids[i][11] == 2)
                .collect()
        } else {
            Vec::new()
        }
    }

    pub(super) fn ucas_paths(&self) -> Vec<String> {
        let stem = &self.rel[..self.rel.len() - 5];
        (0..self.parts.max(1))
            .map(|k| {
                if k == 0 {
                    format!("{stem}.ucas")
                } else {
                    format!("{stem}_s{k}.ucas")
                }
            })
            .collect()
    }

    pub(super) fn names(&self) -> R<HashMap<usize, String>> {
        if self.dir_index.is_empty() {
            return Ok(HashMap::new());
        }
        let mut r = Rd::new(
            &self.dir_index,
            0,
            format!("{}: the directory index", self.rel),
        );
        r.fstr()?;
        let nd = r.i32()?.max(0) as usize;
        let dirs: Vec<[u64; 4]> = (0..nd)
            .map(|_| Ok([r.le(4)?, r.le(4)?, r.le(4)?, r.le(4)?]))
            .collect::<R<_>>()?;
        let nf = r.i32()?.max(0) as usize;
        let files: Vec<[u64; 3]> = (0..nf)
            .map(|_| Ok([r.le(4)?, r.le(4)?, r.le(4)?]))
            .collect::<R<_>>()?;
        let ns = r.i32()?.max(0) as usize;
        let strs: Vec<String> = (0..ns).map(|_| r.fstr()).collect::<R<_>>()?;
        let (mut out, none) = (HashMap::new(), 0xffff_ffffu64);
        let mut todo = if dirs.is_empty() {
            Vec::new()
        } else {
            vec![(0usize, String::new())]
        };
        let s = |i: u64| {
            strs.get(i as usize)
                .cloned()
                .ok_or_else(|| SplitError::Red("IndexError".into()))
        };
        while let Some((d, at)) = todo.pop() {
            let [name, child, sib, mut f] = *dirs
                .get(d)
                .ok_or_else(|| SplitError::Red("IndexError".into()))?;
            let here = if name != none {
                format!("{at}{}/", s(name)?)
            } else {
                at.clone()
            };
            if sib != none {
                todo.push((sib as usize, at));
            }
            if child != none {
                todo.push((child as usize, here.clone()));
            }
            while f != none {
                let [nm, nx, chunk] = *files
                    .get(f as usize)
                    .ok_or_else(|| SplitError::Red("IndexError".into()))?;
                out.insert(chunk as usize, format!("{here}{}", s(nm)?));
                f = nx;
            }
        }
        Ok(out)
    }

    pub(super) fn read(&self, i: usize, ucas: &[Arc<dyn Src>]) -> R<Vec<u8>> {
        if self.encrypted {
            return ve(format!(
                "{}: the container is encrypted and no key was given",
                self.rel
            ));
        }
        let (off, ln) = *self
            .ranges
            .get(i)
            .ok_or_else(|| SplitError::Red("IndexError".into()))?;
        let bs = self.block_size.max(1);
        let first = off / bs;
        let last = if ln != 0 {
            (off + ln - 1) / bs + 1
        } else {
            first
        };
        let mut out = Vec::new();
        for k in first..last {
            let (at, size, raw, m) = *self
                .blocks
                .get(k as usize)
                .ok_or_else(|| SplitError::Red("IndexError".into()))?;
            let src = ucas
                .get((at / self.part_size) as usize)
                .ok_or_else(|| SplitError::Red("FileNotFoundError: a .ucas partition".into()))?;
            let d = src.as_ref().read_up(at % self.part_size, size as usize)?;
            let meth = if m != 0 {
                self.methods
                    .get(m as usize - 1)
                    .cloned()
                    .ok_or_else(|| SplitError::Red("IndexError".into()))?
            } else {
                String::new()
            };
            match meth.as_str() {
                "" => out.extend_from_slice(&d[..d.len().min(raw as usize)]),
                "Oodle" => out.extend(crate::oodle::decode(
                    &d,
                    raw as usize,
                    &format!("{}: chunk {i}", self.rel),
                )?),
                "Zlib" => out.extend(crate::rd::zlib_all(&d, &self.rel)?),
                "LZ4" => out.extend(crate::lz4::block_upto(&d, raw as usize)?),
                m => return ve(format!("{}: compression method {m} is not read", self.rel)),
            }
        }
        let s = (off - first * bs) as usize;
        Ok(sl(&out, s, s + ln as usize).to_vec())
    }
}
