//! The `nn::jit` image (port of the engine's `dm/tools/jit_capture.py`, byte-identical output).
//!
//! A host that runs no code made at run time (the iPhone) serves `nn::jit` requests from an image. The Mac host
//! captures each request of the game's JIT plugin into a `<key>.jit` file (format `DMJ5`, `hle_jit.c`). [`build()`](crate::jit::build)
//! makes `jit.img` (the code in one canonical layout), `jit.out` (the data memory bytes and the output buffers) and
//! `jit.tsv` (the layout, the modules and one row per request) from the records of one or more capture folders.
//!
//! The code is made position-independent. Every four-word address sequence (`movz`, three `movk`) that points into
//! the code memory or a module becomes `adrp` + `add` to the same place in the canonical layout. Requests that every
//! capture made at the same code offset keep that offset. The others go after them, and their branches out of the
//! request move with them. Output words that differ between two captures of a request are host addresses. They get a
//! relocation row.

mod a64;
mod build;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use crate::RewriteError;
use crate::error::fail;

/// The page size of the image (`PAGE`).
pub const PAGE: u64 = 0x4000;

/// One region of the layout a request was made in: a module, or the code (`jit_rx`), data (`jit_ro`) or transfer
/// memory (`jit_tmem`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    /// The name (`main`, `jit_rx`).
    pub name: String,
    /// The base address.
    pub base: u64,
    /// The size.
    pub size: u64,
    /// The size of the patch area in front of the base.
    pub pre: u64,
}

/// One captured request (a `DMJ5` file).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    /// The command.
    pub cmd: u32,
    /// The size of the request data.
    pub dsize: u64,
    /// The SHA-256 of the input.
    pub insha: [u8; 32],
    /// The request data with host words masked.
    pub data: [u8; 32],
    /// The request data as it was.
    pub raw: [u8; 32],
    /// The return value.
    pub ret: i32,
    /// The code the plugin wrote: offset in the code memory and size.
    pub o0: (u64, u64),
    /// The data memory bytes: offset and size.
    pub o1: (u64, u64),
    /// The regions in file order. A repeated name replaces the earlier value in its place (a Python dict).
    pub regs: Vec<Region>,
    /// The code.
    pub code: Vec<u8>,
    /// The data memory bytes.
    pub ro: Vec<u8>,
    /// The output buffer.
    pub out: Vec<u8>,
    /// The session: the base of the code memory (`jit_rx`).
    pub session: u64,
}

struct Reader<'a> {
    b: &'a [u8],
    o: usize,
    label: &'a str,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: u64) -> Result<&'a [u8], RewriteError> {
        let end = usize::try_from(n)
            .ok()
            .and_then(|n| self.o.checked_add(n))
            .filter(|&e| e <= self.b.len());
        let Some(end) = end else {
            return fail(format!("{}: truncated", self.label));
        };
        let s = &self.b[self.o..end];
        self.o = end;
        Ok(s)
    }

    fn arr<const N: usize>(&mut self) -> Result<[u8; N], RewriteError> {
        let s = self.take(N as u64)?;
        let mut a = [0u8; N];
        a.copy_from_slice(s);
        Ok(a)
    }

    fn u32(&mut self) -> Result<u32, RewriteError> {
        Ok(u32::from_le_bytes(self.arr()?))
    }

    fn u64(&mut self) -> Result<u64, RewriteError> {
        Ok(u64::from_le_bytes(self.arr()?))
    }
}

impl Record {
    /// Parses a capture file (`read`). `label` names it in errors. A short file is an error, and so is code whose size is
    /// not a multiple of 4.
    pub fn parse(label: &str, b: &[u8]) -> Result<Record, RewriteError> {
        if b.get(..4) != Some(b"DMJ5".as_slice()) {
            return fail(format!(
                "{label}: not a capture of this format (rewrite it with the current host)"
            ));
        }
        let mut r = Reader { b, o: 4, label };
        let cmd = r.u32()?;
        let _c0 = (r.u64()?, r.u64()?);
        let _insz = r.u64()?;
        let dsize = r.u64()?;
        let insha = r.arr()?;
        let data = r.arr()?;
        let raw = r.arr()?;
        let outsz = r.u64()?;
        let ret = i32::from_le_bytes(r.arr()?);
        let o0 = (r.u64()?, r.u64()?);
        let o1 = (r.u64()?, r.u64()?);
        let nr = r.u32()?;
        let mut regs: Vec<Region> = Vec::new();
        for _ in 0..nr {
            let raw_name: [u8; 16] = r.arr()?;
            let len = raw_name.iter().position(|&c| c == 0).unwrap_or(16);
            let Ok(name) = std::str::from_utf8(&raw_name[..len]) else {
                return fail(format!("{label}: a region name is not UTF-8"));
            };
            let reg = Region {
                name: name.to_string(),
                base: r.u64()?,
                size: r.u64()?,
                pre: r.u64()?,
            };
            match regs.iter_mut().find(|x| x.name == reg.name) {
                Some(x) => *x = reg,
                None => regs.push(reg),
            }
        }
        let code = r.take(o0.1)?.to_vec();
        if code.len() % 4 != 0 {
            return fail(format!(
                "{label}: the code size {:#x} is no multiple of 4",
                code.len()
            ));
        }
        let ro = r.take(o1.1)?.to_vec();
        let out = r.take(outsz)?.to_vec();
        let Some(session) = regs.iter().find(|x| x.name == "jit_rx").map(|x| x.base) else {
            return fail(format!("{label}: the capture has no region jit_rx"));
        };
        Ok(Record {
            cmd,
            dsize,
            insha,
            data,
            raw,
            ret,
            o0,
            o1,
            regs,
            code,
            ro,
            out,
            session,
        })
    }

    /// The region `name`.
    pub fn reg(&self, name: &str) -> Option<&Region> {
        self.regs.iter().find(|x| x.name == name)
    }

    /// `rekey`: the request key as `hle_jit.c` makes it, the first 32 hex digits of the SHA-256 of the command, the data
    /// size, the data and the input's hash. With a mask for the command, the data is the raw bytes under the mask.
    pub fn key(&self, keys: &Keys) -> String {
        let data = match keys.masks.get(&i128::from(self.cmd)) {
            Some(m) => {
                let mut d = [0u8; 32];
                for (i, x) in d.iter_mut().enumerate() {
                    let v = if (i as u64) < self.dsize {
                        self.raw[i]
                    } else {
                        0
                    };
                    *x = v & m[i];
                }
                d
            }
            None => self.data,
        };
        let mut b = Vec::with_capacity(112);
        b.extend_from_slice(&u64::from(self.cmd).to_le_bytes());
        b.extend_from_slice(&self.dsize.to_le_bytes());
        b.extend_from_slice(&data);
        b.extend_from_slice(&self.insha);
        let mut h = nxb_model::sha256(&b).to_hex();
        h.truncate(32);
        h
    }
}

/// The profile keys of the image: `jit_key_data` (command to a 32-byte mask of the request bytes that make the key)
/// and `jit_out_code` (command to the offsets of the entries, the count and the size in its output).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Keys {
    /// `jit_key_data`.
    pub masks: BTreeMap<i128, [u8; 32]>,
    /// `jit_out_code`: (entries, count, size).
    pub out_codes: BTreeMap<i128, (i128, i128, i128)>,
}

/// The image files and the tool's report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    /// `jit.img`.
    pub img: Vec<u8>,
    /// `jit.out`.
    pub out: Vec<u8>,
    /// `jit.tsv`.
    pub tsv: String,
    /// The summary line (the tool's standard output).
    pub summary: String,
    /// The keys of the requests that differ between captures after relocation (the tool's exit code 1).
    pub disagree: Vec<String>,
}

/// `align(v, a)`.
pub(crate) fn align(v: i128, a: i128) -> i128 {
    (v + a - 1).div_euclid(a) * a
}

/// `f'{v:#x}'` of a Python int.
pub(crate) fn hx(v: i128) -> String {
    if v < 0 {
        format!("-{:#x}", v.unsigned_abs())
    } else {
        format!("{v:#x}")
    }
}

/// `f'{v:x}'` of a Python int.
pub(crate) fn hn(v: i128) -> String {
    if v < 0 {
        format!("-{:x}", v.unsigned_abs())
    } else {
        format!("{v:x}")
    }
}

/// `build`: the image from `records` in the order of the capture folders and of the file names in each. `notes`
/// gets the tool's `[RED]` lines (its standard error) in order, also when the build fails. A session whose code
/// refers to code memory that none of its records holds is left out, and the rest builds.
pub fn build(
    records: Vec<Record>,
    keys: &Keys,
    notes: &mut Vec<String>,
) -> Result<Image, RewriteError> {
    let mut recs: Vec<(String, Vec<Record>)> = Vec::new();
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    for r in records {
        let k = r.key(keys);
        let i = *index.entry(k.clone()).or_insert_with(|| {
            recs.push((k, Vec::new()));
            recs.len() - 1
        });
        let rl = &mut recs[i].1;
        if !rl.iter().any(|x| x.session == r.session) {
            rl.push(r);
        }
    }
    loop {
        if recs.is_empty() {
            return fail("no captures");
        }
        match build::build_from(&recs, keys, notes) {
            Ok(img) => return Ok(img),
            Err(build::Fail::Other(m)) => return fail(m),
            Err(build::Fail::Unplaced { session, off }) => {
                let n = recs
                    .iter()
                    .flat_map(|(_, rl)| rl)
                    .filter(|r| r.session == session)
                    .count();
                let only = recs
                    .iter()
                    .filter(|(_, rl)| rl.iter().all(|r| r.session == session))
                    .count();
                notes.push(format!(
                    "[RED] capture session {session:#x} left out ({}): its {n} requests, {only} of them captured in no other session",
                    build::unplaced(session, off)
                ));
                recs = recs
                    .into_iter()
                    .filter_map(|(k, rl)| {
                        let rl: Vec<Record> =
                            rl.into_iter().filter(|r| r.session != session).collect();
                        (!rl.is_empty()).then_some((k, rl))
                    })
                    .collect();
            }
        }
    }
}
