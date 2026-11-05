//! Local time for zip's DOS timestamps (`time.localtime` in `pack_ipa.py`), from the system's TZif file
//! (`/etc/localtime`), std only.

use std::sync::OnceLock;

struct Zone {
    /// (transition time, offset index), sorted.
    transitions: Vec<(i64, usize)>,
    /// UTC offsets in seconds.
    offsets: Vec<i64>,
}

fn be32(b: &[u8], o: usize) -> Option<i64> {
    b.get(o..o + 4)
        .map(|s| i32::from_be_bytes([s[0], s[1], s[2], s[3]]) as i64)
}

fn be64(b: &[u8], o: usize) -> Option<i64> {
    let s = b.get(o..o + 8)?;
    let mut a = [0u8; 8];
    a.copy_from_slice(s);
    Some(i64::from_be_bytes(a))
}

fn parse(b: &[u8]) -> Option<Zone> {
    if b.get(..4) != Some(b"TZif") {
        return None;
    }
    let counts = |at: usize| -> Option<[usize; 6]> {
        let mut c = [0usize; 6];
        for (i, x) in c.iter_mut().enumerate() {
            *x = be32(b, at + 20 + 4 * i)? as usize;
        }
        Some(c)
    };
    let c1 = counts(0)?;
    let [isutc, isstd, leap, timecnt, typecnt, charcnt] = c1;
    let v1_len = 44 + timecnt * 5 + typecnt * 6 + charcnt + leap * 8 + isstd + isutc;
    let version = *b.get(4)?;
    let (base, wide) = if version >= b'2' {
        (v1_len, true)
    } else {
        (0, false)
    };
    let c = if wide { counts(base)? } else { c1 };
    let [_, _, _, timecnt, typecnt, _] = c;
    let tsz = if wide { 8 } else { 4 };
    let mut p = base + 44;
    let mut times = Vec::with_capacity(timecnt);
    for _ in 0..timecnt {
        times.push(if wide { be64(b, p)? } else { be32(b, p)? });
        p += tsz;
    }
    let idx: Vec<usize> = b.get(p..p + timecnt)?.iter().map(|&x| x as usize).collect();
    p += timecnt;
    let mut offsets = Vec::with_capacity(typecnt);
    for _ in 0..typecnt {
        offsets.push(be32(b, p)?);
        p += 6;
    }
    Some(Zone {
        transitions: times.into_iter().zip(idx).collect(),
        offsets,
    })
}

fn zone() -> Option<&'static Zone> {
    static Z: OnceLock<Option<Zone>> = OnceLock::new();
    Z.get_or_init(|| std::fs::read("/etc/localtime").ok().and_then(|b| parse(&b)))
        .as_ref()
}

/// UTC offset in seconds at `t` (0 without a zone file).
pub fn offset(t: i64) -> i64 {
    let Some(z) = zone() else { return 0 };
    let i = z.transitions.partition_point(|(tt, _)| *tt <= t);
    let k = if i == 0 { 0 } else { z.transitions[i - 1].1 };
    z.offsets.get(k).copied().unwrap_or(0)
}

/// `time.localtime(t)[:6]`: (year, month, day, hour, minute, second).
pub fn local(t: i64) -> (i64, u32, u32, u32, u32, u32) {
    let l = t + offset(t);
    let days = l.div_euclid(86_400);
    let secs = l.rem_euclid(86_400);
    // civil from days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (
        y,
        m,
        d,
        (secs / 3600) as u32,
        ((secs % 3600) / 60) as u32,
        (secs % 60) as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates() {
        let o = offset(0);
        let (y, m, d, ..) = local(-o);
        assert_eq!((y, m, d), (1970, 1, 1));
        let t = 1_700_000_000i64;
        let (y, m, d, h, mi, s) = local(t - offset(t));
        assert_eq!((y, m, d, h, mi, s), (2023, 11, 14, 22, 13, 20));
    }
}
