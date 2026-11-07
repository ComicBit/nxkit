//! Shader records of cooked Unreal packages (`uepak.lslg`, `ue_inline_shaders`).

use super::R;
use crate::rd::{red, sl, u32le};
use crate::shaders::{CTL_MAGIC, Found, check, find_all};

/// `uepak.lslg`: (tag, control section, code area) of a decoded Switch shader record.
pub(super) fn lslg(s: &[u8]) -> Option<(String, &[u8], &[u8])> {
    if s.get(..4) != Some(b"LSLG") {
        return None;
    }
    let k = crate::shaders::memfind(s, &CTL_MAGIC)?;
    if k < 8 || k + 0x708 > s.len() {
        return None;
    }
    let (nctl, ncode) = (u32le(s, k - 4).ok()?, u32le(s, k + 0x704).ok()?);
    let c = (k - 4).checked_sub(ncode)?;
    if c < 4 || u32le(s, c - 4).ok()? != ncode || k + nctl > s.len() {
        return None;
    }
    Some((
        s[4..6].iter().map(|&b| b as char).collect(),
        &s[k..k + nctl],
        &s[c..c + ncode],
    ))
}

pub(super) fn ue_tag(t: &str) -> Option<&'static str> {
    Some(match t {
        "SV" => "vs",
        "SH" => "hs",
        "SD" => "ds",
        "SG" => "gs",
        "SP" => "ps",
        "SC" => "cs",
        _ => return None,
    })
}

/// `ue_inline_shaders`: LSLG records in cooked packages, as zlib streams, stored, or LZ4 blocks.
pub(super) fn inline_records(d: &[u8], label: &str, out: &mut Vec<Found>) -> R<()> {
    let take = |s: &[u8], at: usize, out: &mut Vec<Found>| -> R<()> {
        let Some((tag, ctl, code)) = lslg(s) else {
            return red(format!(
                "{label}: the shader record at {at:#x} is not an LSLG record of the known layout"
            ));
        };
        let Some(st) = ue_tag(&tag) else {
            return red(format!(
                "{label}: the shader record at {at:#x} has stage tag {tag}"
            ));
        };
        if let Some(why) = check(st, ctl, code) {
            return red(format!("{label}: shader at {at:#x} ({st}): {why}"));
        }
        out.push(Found {
            place: format!("{label}#UE{at:x}"),
            stage: st,
            ctl: ctl.to_vec(),
            code: code.to_vec(),
        });
        Ok(())
    };
    let len32 = |i: usize| {
        if i >= 4 {
            u32le(d, i - 4).unwrap_or(0)
        } else {
            0
        }
    };
    let mut i = 0;
    while i + 2 <= d.len() {
        if d[i] == 0x78 && matches!(d[i + 1], 0x01 | 0x5e | 0x9c | 0xda) {
            let n = len32(i);
            if (2..=d.len() - i).contains(&n)
                && let Some((s, used)) = crate::zlibx::zlib_stream_used(&d[i..i + n], 1 << 31)
                && used == n
                && s.get(..4) == Some(b"LSLG")
            {
                take(&s, i, out)?;
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    for i in find_all(d, b"LSLG") {
        let n = len32(i);
        if (2..=d.len() - i).contains(&n) && lslg(&d[i..i + n]).is_some() {
            take(&d[i..i + n], i, out)?;
            continue;
        }
        for j in (i.saturating_sub(5).max(4)..i).rev() {
            let (t, ext) = (d[j] >> 4, &d[j + 1..i]);
            let ok_ext = ext.last().is_some_and(|&l| l < 0xff)
                && ext[..ext.len() - 1].iter().all(|&x| x == 0xff);
            if (t < 15 && !ext.is_empty()) || (t == 15 && !ok_ext) {
                continue;
            }
            let n = if j >= 4 {
                u32le(d, j - 4).unwrap_or(0)
            } else {
                0
            };
            if !(8..=d.len().saturating_sub(j + 5)).contains(&n) {
                continue;
            }
            let raw = u32le(d, j + n).unwrap_or(0);
            let Ok(s) = crate::lz4::block_upto(sl(d, j, j + n), raw) else {
                continue;
            };
            if s.len() == raw && s.get(..4) == Some(b"LSLG") {
                take(&s, j, out)?;
                break;
            }
        }
    }
    Ok(())
}

/// `EShaderFrequency` of a shader code archive entry.
fn freq(f: u8) -> Option<&'static str> {
    Some(match f {
        0 => "vs",
        1 => "hs",
        2 => "ds",
        3 => "ps",
        4 => "gs",
        5 => "cs",
        _ => return None,
    })
}

/// A shader of a code archive: (offset from the code start, size, raw size, frequency).
type ArcShader = (u64, u32, u32, u8);

/// `uepak.archive`: (the shaders, the code start) of a version 2 shader code archive.
fn archive(b: &[u8], what: &str) -> R<(Vec<ArcShader>, usize)> {
    let mut o = 0usize;
    let need = |o: usize, n: usize| -> R<()> {
        if o.checked_add(n).is_none_or(|e| e > b.len()) {
            return red(format!(
                "{what}: {n:#x} bytes at {o:#x} do not fit in its {:#x}",
                b.len()
            ));
        }
        Ok(())
    };
    let u32_ = |o: &mut usize| -> R<usize> {
        need(*o, 4)?;
        let v = u32le(b, *o)?;
        *o += 4;
        Ok(v)
    };
    if u32_(&mut o)? != 2 {
        return red(format!("{what}: not a version 2 shader code archive"));
    }
    for elem in [20usize, 20, 16] {
        let n = u32_(&mut o)?;
        let len = n.checked_mul(elem).unwrap_or(usize::MAX);
        need(o, len)?;
        o += len;
    }
    let n = u32_(&mut o)?;
    need(o, n.checked_mul(17).unwrap_or(usize::MAX))?;
    let mut sh = Vec::with_capacity(n);
    for _ in 0..n {
        let off = u64::from_le_bytes(sl(b, o, o + 8).try_into().unwrap_or_default());
        sh.push((
            off,
            u32le(b, o + 8)? as u32,
            u32le(b, o + 12)? as u32,
            b[o + 16],
        ));
        o += 17;
    }
    let np = u32_(&mut o)?;
    need(o, np.checked_mul(16).unwrap_or(usize::MAX))?;
    o += 16 * np;
    let ni = u32_(&mut o)?;
    need(o, ni.checked_mul(4).unwrap_or(usize::MAX))?;
    o += 4 * ni;
    Ok((sh, o))
}

/// `ue_shaders`: an Unreal shader code archive with every shader decoded (written by the build's Unreal decode), each
/// shader an LSLG record of one stage. Labels: `<label>#UE<index>`.
pub fn ue_shaders(d: &[u8], label: &str) -> R<Vec<Found>> {
    let (shaders, code) = archive(d, label)?;
    let mut out = Vec::new();
    for (i, (off, size, raw, fr)) in shaders.into_iter().enumerate() {
        if size != raw {
            return red(format!(
                "{label}: shader {i} is still compressed (an archive the build has not decoded)"
            ));
        }
        let a = usize::try_from(off)
            .unwrap_or(usize::MAX)
            .saturating_add(code);
        let Some(s) = d.get(a..a.saturating_add(size as usize)) else {
            return red(format!("{label}: shader {i} runs past the archive"));
        };
        let Some((tag, ctl, cd)) = lslg(s) else {
            return red(format!("{label}: shader {i} is not an LSLG record"));
        };
        let st = ue_tag(&tag);
        if st.is_none() || freq(fr) != st {
            return red(format!(
                "{label}: shader {i} has tag {tag} but frequency {fr}"
            ));
        }
        let st = st.unwrap_or("vs");
        if let Some(why) = check(st, ctl, cd) {
            return red(format!("{label}: shader {i} ({st}): {why}"));
        }
        out.push(Found {
            place: format!("{label}#UE{i}"),
            stage: st,
            ctl: ctl.to_vec(),
            code: cd.to_vec(),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arc(shaders: &[(u64, u32, u32, u8)], code: &[u8]) -> Vec<u8> {
        let mut b = 2u32.to_le_bytes().to_vec();
        for (n, e) in [(1u32, 20usize), (0, 20), (0, 16)] {
            b.extend(n.to_le_bytes());
            b.extend(vec![0u8; n as usize * e]);
        }
        b.extend((shaders.len() as u32).to_le_bytes());
        for (o, s, r, f) in shaders {
            b.extend(o.to_le_bytes());
            b.extend(s.to_le_bytes());
            b.extend(r.to_le_bytes());
            b.push(*f);
        }
        b.extend(0u32.to_le_bytes());
        b.extend(1u32.to_le_bytes());
        b.extend(0u32.to_le_bytes());
        b.extend_from_slice(code);
        b
    }

    #[test]
    fn archives_read_and_refuse() {
        let a = arc(&[(0, 4, 4, 0), (4, 2, 2, 3)], b"abcdef");
        let (sh, code) = archive(&a, "x").ok().expect("archive");
        assert_eq!(sh, vec![(0, 4, 4, 0), (4, 2, 2, 3)]);
        assert_eq!(&a[code..], b"abcdef");
        let e = ue_shaders(&a, "/ue_shaders/a").err().map(|e| e.to_string());
        assert!(e.is_some_and(|e| e.contains("shader 0 is not an LSLG record")));
        let c = arc(&[(0, 4, 9, 0)], b"abcd");
        let e = ue_shaders(&c, "/u").err().map(|e| e.to_string());
        assert!(e.is_some_and(|e| e.contains("still compressed")));
        assert!(archive(&a[..20], "x").is_err());
        assert!(archive(&3u32.to_le_bytes(), "x").is_err());
        assert_eq!(
            ue_shaders(&arc(&[], b""), "/u").ok().map(|v| v.len()),
            Some(0)
        );
    }
}
