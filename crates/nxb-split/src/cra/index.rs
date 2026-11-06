//! The file table of a CRA archive, as the game's NRO holds it (`index_of` and `Index` of `cra.py`). The table is
//! found by the code that reads it:
//! * `adrp` + `add` xA, table, then `mov w, #0x12`, `ldrsh [xA, wI, sxtw #1]`, `madd`, `mov w, #(2 x count)`: the
//!   one place that indexes it with records of 18 bytes.
//! * `adrp` + `add` x, checksums, then `mov x8, #'UseU...'`, in the read's completion.
//! * `adrp` + `add` x, suffixes, then `ldr x, [x, x, lsl #3]`, right after the `ldrsb` of a record's +3.
//!
//! Records sit at `table + i16[i] + i x 18 + 2 x count`. Each has `+0` u16 how far back the record with the path's
//! prefix is, `+2` u8 the prefix's length, `+3` i8 a suffix's index (-1 none), `+4` u32 size xor 0xff00ff00, `+8`
//! u40 offset xor 0xff00ff00ff and `+0xd` the rest of the path up to a zero. A character is stored as `(i - c) & 0x7f`
//! with `i` the index of the record that holds it. A `$` in a path stands for the folder that holds it.

use crate::SplitError;

fn bad(m: &str) -> SplitError {
    SplitError::Format(format!("a CRA file table: {m}"))
}

/// One file of the archive: `(index, path, size, offset, checksum)` of `cra.py`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Position in the table.
    pub index: usize,
    /// The path, in the bytes the key is made from (Latin-1).
    pub path: Vec<u8>,
    /// Size in the archive.
    pub size: u32,
    /// Offset in the archive.
    pub off: u64,
    /// The checksum `read` compares with.
    pub sum: u32,
}

impl Entry {
    /// The path as text (every byte a character, as Python's `latin1` decode).
    pub fn label(&self) -> String {
        self.path.iter().map(|&b| char::from(b)).collect()
    }
}

/// Where the table, its checksums and its suffixes are (`index_of`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parts {
    table: i64,
    count: i64,
    sums: i64,
    names: Option<i64>,
}

/// The address `adrp` and `add` build in `reg` at instructions `i` and `i + 1`.
fn adrp_add(w: &[u32], i: usize, reg: u32) -> Option<i64> {
    let (a, b) = (*w.get(i)?, *w.get(i + 1)?);
    if a & 0x9f00_001f != 0x9000_0000 | reg || b & 0xffc0_03ff != 0x9100_0000 | reg << 5 | reg {
        return None;
    }
    let mut hi = i64::from((a >> 5 & 0x7_ffff) << 2 | (a >> 29 & 3));
    if hi & 1 << 20 != 0 {
        hi -= 1 << 21;
    }
    Some(((i as i64 * 4) & !0xfff) + (hi << 12) + i64::from(b >> 10 & 0xfff))
}

/// `index_of`: `Ok(None)` for an NRO without the table (`cra.Error` of the reference: try the next NRO).
pub fn index_of(nro: &[u8]) -> Result<Option<Parts>, SplitError> {
    if nro.get(0x10..0x14) != Some(b"NRO0") {
        return Ok(None);
    }
    let end = nro
        .get(0x24..0x28)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize & !3)
        .ok_or_else(|| bad("a short NRO header"))?;
    let text = nro.get(..end).ok_or_else(|| bad("text past the end"))?;
    let w: Vec<u32> = text
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| u32::from_le_bytes(*b))
        .collect();
    let (mut table, mut sums, mut names, mut count) = (None, None, None, 0);
    for i in 3..w.len().saturating_sub(3) {
        let c = w[i];
        if table.is_none()
            && c & 0xffe0_fc00 == 0x78e0_d800
            && w[i - 1] & 0xffff_ffe0 == 0x5280_0240
            && w[i + 1] & 0xffe0_8000 == 0x1b00_0000
            && w[i + 2] & 0xffe0_0000 == 0x5280_0000
        {
            if let Some(a) = adrp_add(&w, i - 3, c >> 5 & 31) {
                table = Some(a);
                count = i64::from((w[i + 2] >> 5 & 0xffff) / 2);
            }
        } else if sums.is_none()
            && c & 0xffff_ffe0 == 0xd28e_6aa0
            && w[i + 1] & 0xffff_ffe0 == 0xf2aa_aca0
        {
            for k in i - 3..i {
                for reg in 0..31 {
                    if let Some(a) = adrp_add(&w, k, reg) {
                        sums = Some(a);
                    }
                }
            }
        }
    }
    let (Some(table), Some(sums)) = (table, sums) else {
        return Ok(None);
    };
    for i in 0..w.len().saturating_sub(4) {
        if w[i] & 0xffff_fc00 == 0x39c0_0c00
            && w[i + 1] & 0xff00_0000 == 0x3700_0000
            && w[i + 2] & 0xffff_fc00 == 0x9240_1c00
        {
            for reg in 0..31 {
                if let Some(a) = adrp_add(&w, i + 3, reg) {
                    let w5 = w.get(i + 5).ok_or_else(|| bad("code past the end"))?;
                    if w5 & 0xffe0_fc00 == 0xf860_7800 {
                        names = Some(a);
                    }
                }
            }
        }
    }
    Ok(Some(Parts {
        table,
        count,
        sums,
        names,
    }))
}

/// The table of one NRO.
pub struct Index<'a> {
    d: &'a [u8],
    p: Parts,
}

impl<'a> Index<'a> {
    /// `Index(nro)`: `Ok(None)` when the NRO holds no table.
    pub fn new(nro: &'a [u8]) -> Result<Option<Index<'a>>, SplitError> {
        Ok(index_of(nro)?.map(|p| Index { d: nro, p }))
    }

    /// The parts of the table found in the code.
    pub fn parts(&self) -> Parts {
        self.p
    }

    fn pos(o: i64) -> Result<usize, SplitError> {
        usize::try_from(o).map_err(|_| bad("an offset before the start"))
    }

    fn u8_at(&self, o: i64) -> Result<u8, SplitError> {
        self.d
            .get(Self::pos(o)?)
            .copied()
            .ok_or_else(|| bad("a read past the end"))
    }

    fn le<const N: usize>(&self, o: i64) -> Result<[u8; N], SplitError> {
        let a = Self::pos(o)?;
        self.d
            .get(a..a.saturating_add(N))
            .and_then(|b| b.try_into().ok())
            .ok_or_else(|| bad("a read past the end"))
    }

    fn u16_at(&self, o: i64) -> Result<i64, SplitError> {
        Ok(i64::from(u16::from_le_bytes(self.le(o)?)))
    }

    fn u32_at(&self, o: i64) -> Result<u32, SplitError> {
        Ok(u32::from_le_bytes(self.le(o)?))
    }

    fn rec(&self, i: i64) -> Result<i64, SplitError> {
        let s = i64::from(i16::from_le_bytes(self.le(self.p.table + i * 2)?));
        Ok(self.p.table + s + i * 18 + self.p.count * 2)
    }

    /// The path of record `i` (`name`).
    pub fn name(&self, i: i64) -> Result<Vec<u8>, SplitError> {
        let r = self.rec(i)?;
        let mut need = usize::from(self.u8_at(r + 2)?);
        let mut out = vec![0u8; need];
        if need > 0 {
            let mut k = i - self.u16_at(r)?;
            loop {
                let s = self.rec(k)?;
                let have = need.min(usize::from(self.u8_at(s + 2)?));
                for j in 0..need - have {
                    let c = self.u8_at(s + 0xd + j as i64)?;
                    out[have + j] = (k - i64::from(c)) as u8 & 0x7f;
                }
                need = have;
                if need == 0 {
                    break;
                }
                let back = self.u16_at(s)?;
                if back == 0 {
                    return Err(bad("a prefix that points at itself"));
                }
                k -= back;
            }
        }
        let mut o = r + 0xd;
        while self.u8_at(o)? != 0 {
            out.push((i - i64::from(self.u8_at(o)?)) as u8 & 0x7f);
            o += 1;
        }
        let sfx = self.u8_at(r + 3)?;
        if sfx < 0x80
            && let Some(names) = self.p.names
        {
            let mut o = i64::from_le_bytes(self.le(names + i64::from(sfx) * 8)?);
            while self.u8_at(o)? != 0 {
                out.push((i64::from(sfx) - i64::from(self.u8_at(o)?)) as u8 & 0x7f);
                o += 1;
            }
        }
        // `$` stands for the part before it
        let mut parts: Vec<Vec<u8>> = Vec::new();
        for c in out.split(|&b| b == b'/') {
            let part = match parts.last() {
                Some(last) => c
                    .iter()
                    .flat_map(|&b| if b == b'$' { last.clone() } else { vec![b] })
                    .collect(),
                None => c.to_vec(),
            };
            parts.push(part);
        }
        Ok(parts.join(&b'/'))
    }

    /// `files()`: the entries, the empty records left out.
    pub fn files(&self) -> Result<Vec<Entry>, SplitError> {
        let mut out = Vec::new();
        for i in 0..self.p.count {
            let r = self.rec(i)?;
            let size = self.u32_at(r + 4)? ^ 0xff00_ff00;
            if size == 0 {
                continue;
            }
            let off = (u64::from_le_bytes(self.le(r + 8)?) & 0xff_ffff_ffff) ^ 0xff_00ff_00ff;
            out.push(Entry {
                index: i as usize,
                path: self.name(i)?,
                size,
                off,
                sum: self.u32_at(self.p.sums + i * 4)?,
            });
        }
        Ok(out)
    }
}
