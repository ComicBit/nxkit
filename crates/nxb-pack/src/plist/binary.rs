//! Binary property lists (`bplist00`), read only.

use super::Value;
use crate::error::{Error, Result, format_err};

/// Parses a binary property list (`bplist00`).
pub fn parse_binary(b: &[u8]) -> Result<Value> {
    let bad = |m: &str| format_err(format!("binary plist: {m}"));
    if b.len() < 8 + 32 {
        return Err(bad("too short"));
    }
    let t = &b[b.len() - 32..];
    let off_size = t[6] as usize;
    let ref_size = t[7] as usize;
    let be = |s: &[u8]| s.iter().fold(0u64, |a, x| a << 8 | *x as u64);
    let num = be(&t[8..16]) as usize;
    let top = be(&t[16..24]) as usize;
    let table = be(&t[24..32]) as usize;
    if !(1..=8).contains(&off_size) || !(1..=8).contains(&ref_size) || top >= num {
        return Err(bad("bad trailer"));
    }
    let offset = |i: usize| -> Result<usize> {
        let o = table + i * off_size;
        b.get(o..o + off_size)
            .map(|s| be(s) as usize)
            .ok_or_else(|| bad("offset table"))
    };
    struct Ctx<'a> {
        b: &'a [u8],
        ref_size: usize,
        num: usize,
    }
    fn obj(
        c: &Ctx<'_>,
        offset: &dyn Fn(usize) -> Result<usize>,
        i: usize,
        depth: usize,
    ) -> Result<Value> {
        let bad = |m: &str| format_err(format!("binary plist: {m}"));
        if i >= c.num || depth > 64 {
            return Err(bad("bad object reference"));
        }
        let be = |s: &[u8]| s.iter().fold(0u64, |a, x| a << 8 | *x as u64);
        let mut p = offset(i)?;
        let marker = *c.b.get(p).ok_or_else(|| bad("object offset"))?;
        p += 1;
        let (kind, low) = (marker >> 4, (marker & 15) as usize);
        let get = |from: usize, n: usize| {
            c.b.get(from..from + n)
                .ok_or_else(|| bad("object past the end"))
        };
        // Length: the low nibble, or an integer object after the marker when it is 0xf.
        let mut len = low;
        if low == 15 && matches!(kind, 0x4 | 0x5 | 0x6 | 0xa | 0xd) {
            let m = *c.b.get(p).ok_or_else(|| bad("length"))?;
            if m >> 4 != 1 {
                return Err(bad("length is not an integer"));
            }
            let n = 1usize << (m & 15);
            len = be(get(p + 1, n)?) as usize;
            p += 1 + n;
        }
        Ok(match kind {
            0x0 => match marker {
                0x08 => Value::Bool(false),
                0x09 => Value::Bool(true),
                _ => return Err(Error::Unsupported("binary plist null or fill".into())),
            },
            0x1 => {
                let n = 1usize << low;
                let raw = get(p, n)?;
                Value::Integer(if n == 8 {
                    i64::from_be_bytes(fixed::<8>(raw)?)
                } else {
                    be(raw) as i64
                })
            }
            0x2 => {
                let n = 1usize << low;
                let raw = get(p, n)?;
                Value::Real(match n {
                    4 => f32::from_be_bytes(fixed::<4>(raw)?) as f64,
                    8 => f64::from_be_bytes(fixed::<8>(raw)?),
                    _ => return Err(bad("real size")),
                })
            }
            0x3 => Value::Date(format!("{}", f64::from_be_bytes(fixed::<8>(get(p, 8)?)?))),
            0x4 => Value::Data(get(p, len)?.to_vec()),
            0x5 => Value::String(get(p, len)?.iter().map(|c| *c as char).collect()),
            0x6 => {
                let units: Vec<u16> = get(p, len * 2)?
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|u| u16::from_be_bytes(*u))
                    .collect();
                Value::String(String::from_utf16(&units).map_err(|_| bad("bad UTF-16"))?)
            }
            0xa => {
                let refs = get(p, len * c.ref_size)?;
                Value::Array(
                    refs.chunks_exact(c.ref_size)
                        .map(|r| obj(c, offset, be(r) as usize, depth + 1))
                        .collect::<Result<_>>()?,
                )
            }
            0xd => {
                let refs = get(p, 2 * len * c.ref_size)?;
                let (keys, vals) = refs.split_at(len * c.ref_size);
                let mut entries = Vec::with_capacity(len);
                for (k, v) in keys
                    .chunks_exact(c.ref_size)
                    .zip(vals.chunks_exact(c.ref_size))
                {
                    let Value::String(key) = obj(c, offset, be(k) as usize, depth + 1)? else {
                        return Err(bad("dictionary key is not a string"));
                    };
                    entries.push((key, obj(c, offset, be(v) as usize, depth + 1)?));
                }
                Value::Dict(entries)
            }
            _ => {
                return Err(Error::Unsupported(format!(
                    "binary plist object type {kind:#x}"
                )));
            }
        })
    }
    let c = Ctx { b, ref_size, num };
    obj(&c, &offset, top, 0)
}

fn fixed<const N: usize>(raw: &[u8]) -> Result<[u8; N]> {
    raw.try_into()
        .map_err(|_| Error::Format("binary plist: number of the wrong size".into()))
}
