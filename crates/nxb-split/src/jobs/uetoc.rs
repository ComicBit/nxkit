//! Unreal Engine IoStore containers (`uepak.py`: `Toc`, `mount_order`), read by ranges. An encrypted container
//! needs the AES-256 key of the game's paks.

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
    key: Option<[u8; 32]>,
}

const TOC_MAGIC: &[u8; 16] = b"-==--==--==--==-";

/// The largest chunk read: a longer one is damage.
const CHUNK_MAX: u64 = 1 << 32;
/// The largest TOC read.
const TOC_MAX: u64 = 1 << 30;
/// `EIoChunkType` of a shader library and of the cooked packages' exports (TOC version 4 and later), and of the
/// exports in Unreal Engine 4.27's numbering (TOC version 3 and before).
pub(super) const IO_SHADER_LIBRARY: u8 = 8;
const IO_EXPORT_BUNDLE: u8 = 1;
const IO_EXPORT_BUNDLE_427: u8 = 2;

/// The folder of a path relative to the RomFS root (`""` at the top).
pub(super) fn dirname(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |x| x.0)
}

/// `inline_in_containers`: whether the IoStore containers in a pak's folder (`tocs`) hold its cooked packages with
/// their shaders inline. They do when they have export bundles and no shader libraries.
pub(super) fn inline_in_containers<'a>(
    tocs: impl IntoIterator<Item = (&'a str, &'a dyn Src)>,
) -> bool {
    let (mut exports, mut libs) = (false, false);
    for (rel, s) in tocs {
        if let Ok(t) = Toc::load(s, rel) {
            exports |= !t.export_bundles().is_empty();
            libs |= t.of_type(IO_SHADER_LIBRARY);
        }
    }
    exports && !libs
}

/// `mount_order`: a container's place in the mount stack. The game's containers come first, then the numbered
/// patch chunks (`<name>_<n>_P.utoc`, or `<name>_P.utoc` as 0) in order.
pub(super) fn mount_order(rel: &str) -> (u8, u64, String) {
    let base = rel.rsplit('/').next().unwrap_or(rel);
    let l = base.to_lowercase();
    if let Some(stem) = l.strip_suffix("_p.utoc") {
        let digits: String = stem
            .chars()
            .rev()
            .take_while(char::is_ascii_digit)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let pre = &stem[..stem.len() - digits.len()];
        let n = if !digits.is_empty() && pre.ends_with('_') {
            digits.parse().unwrap_or(0)
        } else {
            0
        };
        return (1, n, rel.to_string());
    }
    (0, 0, rel.to_string())
}

impl Toc {
    /// A TOC read from its file.
    pub(super) fn load(s: &dyn Src, rel: &str) -> R<Toc> {
        if s.len() > TOC_MAX {
            return ve(format!("{rel}: a TOC of {} bytes", s.len()));
        }
        Toc::open(&s.all()?, rel)
    }

    pub(super) fn open(b: &[u8], rel: &str) -> R<Toc> {
        if b.get(..16) != Some(&TOC_MAGIC[..]) {
            return ve(format!("{rel}: not an IoStore TOC"));
        }
        let Some(&version) = b.get(16) else {
            return ve(format!("{rel}: the TOC header ends early"));
        };
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
        let need = [
            22usize.checked_mul(n),
            12usize.checked_mul(nblk),
            mlen.checked_mul(nmeth),
        ]
        .into_iter()
        .chain([Some(4 * nseeds), Some(4 * nover)])
        .try_fold(hsz, |a, x| a.checked_add(x?));
        if need.is_none_or(|x| x > b.len()) {
            return ve(format!("{rel}: the TOC ends before its tables"));
        }
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
            key: None,
        })
    }

    /// The key that decrypts an encrypted container.
    pub(super) fn set_key(&mut self, key: Option<[u8; 32]>) {
        self.key = key;
    }

    /// The TOC's path, relative to the RomFS root.
    pub(super) fn rel(&self) -> &str {
        &self.rel
    }

    /// The chunks of the type `t` (`of_type`, Unreal Engine 5 numbering: TOC version 4 and later).
    pub(super) fn chunks_of(&self, t: u8) -> Vec<usize> {
        if self.version < 4 {
            return Vec::new();
        }
        (0..self.ids.len())
            .filter(|&i| self.ids[i][11] == t)
            .collect()
    }

    /// Whether a chunk has the type `t` (`of_type`, Unreal Engine 5 numbering: TOC version 4 and later).
    pub(super) fn of_type(&self, t: u8) -> bool {
        self.version >= 4 && self.ids.iter().any(|c| c[11] == t)
    }

    /// The cooked packages' export chunks (`export_bundles`).
    pub(super) fn export_bundles(&self) -> Vec<usize> {
        let t = if self.version >= 4 {
            IO_EXPORT_BUNDLE
        } else {
            IO_EXPORT_BUNDLE_427
        };
        (0..self.ids.len())
            .filter(|&i| self.ids[i][11] == t)
            .collect()
    }

    pub(super) fn ucas_paths(&self) -> Vec<String> {
        let stem = self.rel.strip_suffix(".utoc").unwrap_or(&self.rel);
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
        // a tree visits each folder and file once, more steps mean the links loop
        let mut steps = dirs.len() + files.len();
        let mut step = || {
            steps = steps.checked_sub(1).ok_or_else(|| {
                SplitError::Red(format!("{}: the directory index loops", self.rel))
            })?;
            Ok::<(), SplitError>(())
        };
        while let Some((d, at)) = todo.pop() {
            step()?;
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
                step()?;
                let [nm, nx, chunk] = *files
                    .get(f as usize)
                    .ok_or_else(|| SplitError::Red("IndexError".into()))?;
                out.insert(chunk as usize, format!("{here}{}", s(nm)?));
                f = nx;
            }
        }
        Ok(out)
    }

    /// Chunk `i` out of the `.ucas` partitions (`ucas`, in the order of [`Toc::ucas_paths`], `None` for a missing
    /// one).
    pub(super) fn read(&self, i: usize, ucas: &[Option<Arc<dyn Src>>]) -> R<Vec<u8>> {
        let key = match (self.encrypted, &self.key) {
            (false, _) => None,
            (true, Some(k)) => Some(crate::aes256::Aes256::new(k)),
            (true, None) => {
                return ve(format!(
                    "{}: the container is encrypted and no key was given",
                    self.rel
                ));
            }
        };
        let (off, ln) = *self
            .ranges
            .get(i)
            .ok_or_else(|| SplitError::Red("IndexError".into()))?;
        if ln > CHUNK_MAX {
            return ve(format!("{}: chunk {i} is {ln} bytes long", self.rel));
        }
        if self.block_size == 0 || self.part_size == 0 {
            return ve(format!("{}: a block or partition size of 0", self.rel));
        }
        let bs = self.block_size;
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
                .get(usize::try_from(at / self.part_size).unwrap_or(usize::MAX))
                .and_then(Option::as_ref)
                .ok_or_else(|| SplitError::Red("FileNotFoundError: a .ucas partition".into()))?;
            let d = match &key {
                None => src.as_ref().read_up(at % self.part_size, size as usize)?,
                Some(aes) => {
                    let mut d = src
                        .as_ref()
                        .read_up(at % self.part_size, (size as usize + 15) & !15)?;
                    if d.len() < size as usize {
                        return ve(format!("{}: block {k} runs past its partition", self.rel));
                    }
                    aes.decrypt_ecb(&mut d);
                    d.truncate(size as usize);
                    d
                }
            };
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
                "Zlib" => {
                    out.extend(crate::zlibx::zlib_stream(&d, raw as usize).ok_or_else(|| {
                        SplitError::Red(format!(
                            "zlib.error: {}: damaged or truncated zlib stream",
                            self.rel
                        ))
                    })?)
                }
                "LZ4" => out.extend(crate::lz4::block_upto(&d, raw as usize)?),
                m => return ve(format!("{}: compression method {m} is not read", self.rel)),
            }
        }
        let s = (off - first * bs) as usize;
        Ok(sl(&out, s, s + ln as usize).to_vec())
    }
}

#[cfg(test)]
pub(super) mod tests;
