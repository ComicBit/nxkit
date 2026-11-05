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
