//! Game Freak's Trinity packs (`trpfs_jobs`, `trinity_shaders`): `data.trpfs` holds packs of files, `data.trpfd` is
//! their index, both FlatBuffers. Oodle files decode with the game's own decoder ([`crate::oodle`]).

use std::collections::HashMap;
use std::sync::Arc;

use super::{Ctx, File, Job, JobOut, Plan, R, ranges};
use crate::bahs::BahsVar;
use crate::rd::red;
use crate::shaders::{Found, scan_blob};
use crate::src::Src;

/// `ONEPACK`: the file system's magic.
pub const ONEPACK: &[u8; 8] = b"ONEPACK\0";

/// FlatBuffers over a file read by ranges.
pub(crate) struct Fb<'a>(pub &'a dyn Src);

impl Fb<'_> {
    fn b<const N: usize>(&self, at: u64) -> R<[u8; N]> {
        let mut v = [0u8; N];
        self.0.read_at(at, &mut v)?;
        Ok(v)
    }
    pub(crate) fn u16(&self, at: u64) -> R<u64> {
        Ok(u64::from(u16::from_le_bytes(self.b(at)?)))
    }
    pub(crate) fn u32(&self, at: u64) -> R<u64> {
        Ok(u64::from(u32::from_le_bytes(self.b(at)?)))
    }
    pub(crate) fn u64(&self, at: u64) -> R<u64> {
        Ok(u64::from_le_bytes(self.b(at)?))
    }
    fn i8(&self, at: u64) -> R<i8> {
        Ok(i8::from_le_bytes(self.b(at)?))
    }
    /// `fb_field`: the absolute offset of field `i` of the table at `t`, `None` when the field is absent.
    pub(crate) fn field(&self, t: u64, i: u64) -> R<Option<u64>> {
        let vt = t.wrapping_sub(i64::from(i32::from_le_bytes(self.b(t)?)) as u64);
        if 4 + 2 * i >= self.u16(vt)? {
            return Ok(None);
        }
        let off = self.u16(vt + 4 + 2 * i)?;
        Ok((off != 0).then_some(t + off))
    }
    /// `fb_vec`: (start, count) of the vector in field `i`.
    pub(crate) fn vec(&self, t: u64, i: u64) -> R<(u64, u64)> {
        let Some(p) = self.field(t, i)? else {
            return Ok((0, 0));
        };
        let p = p + self.u32(p)?;
        Ok((p + 4, self.u32(p)?))
    }
    /// `fb_refs`: the tables or strings a vector of offsets points at.
    pub(crate) fn refs(&self, t: u64, i: u64) -> R<Vec<u64>> {
        let (st, n) = self.vec(t, i)?;
        (0..n)
            .map(|k| Ok(st + 4 * k + self.u32(st + 4 * k)?))
            .collect()
    }
    /// `fb_root`.
    pub(crate) fn root(&self, at: u64) -> R<u64> {
        Ok(at + self.u32(at)?)
    }
    fn u64_field(&self, t: u64, i: u64) -> R<Option<u64>> {
        self.field(t, i)?.map(|f| self.u64(f)).transpose()
    }
}

/// A pack of a job: (offset, name).
type Pack = (u64, String);

/// `trinity_shaders`: each file of a range of packs scanned like a loose one.
fn shaders(s: &dyn Src, rel: &str, packs: &[Pack]) -> R<JobOut> {
    let d = Fb(s);
    let (mut out, mut var) = (Vec::<Found>::new(), BahsVar::new());
    for (off, name) in packs {
        let pr = d.root(*off)?;
        let hs = d.vec(pr, 0)?;
        for (k, t) in d.refs(pr, 1)?.into_iter().enumerate() {
            let comp = match d.field(t, 1)? {
                Some(c) => d.i8(c)?,
                None => 0,
            };
            let n = d.u64_field(t, 3)?.unwrap_or(0);
            let (st, sz) = d.vec(t, 4)?;
            let raw = s.read_up(st, sz as usize)?;
            let data = if comp == 3 {
                crate::oodle::decode(&raw, n as usize, &format!("/{rel}#{name}: file {k}"))?
            } else if sz == n {
                raw
            } else {
                return red(format!(
                    "/{rel}#{name}: file {k} has compression {comp} ({sz} → {n} bytes), which is not known"
                ));
            };
            let h = d.u64(hs.0 + 8 * k as u64)?;
            scan_blob(&data, &format!("/{rel}#{name}#{h:016x}"), &mut out, &mut var, 0)?;
        }
    }
    Ok(JobOut { found: out, var })
}

/// The packs of one file system with its descriptor: (offset, name, size), in offset order.
fn packs(fs: &dyn Src, fd: &[u8], rel: &str) -> R<Vec<(u64, String, u64)>> {
    let m = crate::src::Mem(fd);
    let f = Fb(&m);
    let fr = f.root(0)?;
    let (hst, hn) = f.vec(fr, 0)?;
    let hashes: Vec<u64> = (0..hn).map(|k| f.u64(hst + 8 * k)).collect::<R<_>>()?;
    let names: Vec<String> = f
        .refs(fr, 1)?
        .into_iter()
        .map(|s| {
            let n = f.u32(s)? as usize;
            let a = s as usize + 4;
            Ok(String::from_utf8_lossy(fd.get(a..a + n).unwrap_or_default()).into_owned())
        })
        .collect::<R<_>>()?;
    let mut pack_of = HashMap::new();
    for (h, t) in hashes.iter().zip(f.refs(fr, 2)?) {
        pack_of.insert(*h, f.u64_field(t, 0)?.unwrap_or(0));
    }
    let sizes: Vec<u64> = f
        .refs(fr, 3)?
        .into_iter()
        .map(|t| match f.u64_field(t, 0)? {
            Some(v) => Ok(v),
            None => red(format!("{rel}d: a pack info without a size")),
        })
        .collect::<R<_>>()?;
    let d = Fb(fs);
    let sr = d.root(d.u64(8)?)?;
    let (st, n) = d.vec(sr, 1)?;
    let mut out = Vec::new();
    for k in 0..n {
        let off = d.u64(st + 8 * k)?;
        let pr = d.root(off)?;
        let h0 = d.vec(pr, 0)?;
        let first = if h0.1 > 0 { Some(d.u64(h0.0)?) } else { None };
        let Some(&i) = first.and_then(|h| pack_of.get(&h)) else {
            return red(format!(
                "{rel}: the pack at {off:#x} has no file the descriptor lists"
            ));
        };
        let i = i as usize;
        match (names.get(i), sizes.get(i)) {
            (Some(nm), Some(&sz)) => out.push((off, nm.clone(), sz)),
            _ => return red(format!("{rel}: pack index {i} is past the descriptor's lists")),
        }
    }
    out.sort();
    Ok(out)
}

/// `trpfs_jobs`.
pub fn plan(files: &[File], p: &mut Plan) -> R<()> {
    let by_rel: HashMap<&str, usize> = files
        .iter()
        .enumerate()
        .map(|(i, f)| (f.rel.as_str(), i))
        .collect();
    for (i, f) in files.iter().enumerate() {
        if !f.live || !f.rel.ends_with(".trpfs") {
            continue;
        }
        let fd_rel = format!("{}d", &f.rel[..f.rel.len() - 1]);
        let Some(&j) = by_rel.get(fd_rel.as_str()) else {
            continue;
        };
        if f.head(8)? != ONEPACK {
            continue;
        }
        let fd = files[j].src.as_ref().whole()?;
        let packs = packs(f.src.as_ref(), &fd, &f.rel)?;
        let step = packs.iter().map(|x| x.2).sum::<u64>() as f64 / 64.0;
        for chunk in ranges(&packs, |x| x.2, step) {
            let chunk: Vec<Pack> = chunk.into_iter().map(|(o, n, _)| (o, n)).collect();
            let (src, rel): (Arc<dyn Src>, String) = (f.src.clone(), f.rel.clone());
            let r2 = rel.clone();
            p.jobs.push(Job {
                rel,
                run: Box::new(move |_: &Ctx<'_>, _| shaders(src.as_ref(), &r2, &chunk)),
            });
        }
        p.taken.insert(i);
        p.taken.insert(j);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
