//! Capcom's formats: MT Framework archives and shader packages (`mtarc_members`, `mtspk_shaders`, `mtarc_shaders`) and
//! RE Engine SDF shader definitions (`sdf_shaders`).

use crate::SplitError;
use crate::rd::{red, sl, u16le, u32le, u64le, zlib_all};
use crate::shaders::{CTL_MAGIC, Found, GFX_MAGIC, check, memfind, span, sph_stage};

type R<T> = Result<T, SplitError>;

/// `mtarc_members`: (path, data) of each entry of an MT Framework archive.
pub fn mtarc_members(d: &[u8], label: &str) -> R<Vec<(String, Vec<u8>)>> {
    let (ver, n) = (u16le(d, 4)?, u16le(d, 6)?);
    if ver != 7 {
        return red(format!(
            "{label}: ARC version {ver}: only version 7 is known"
        ));
    }
    if n == 0 {
        return Ok(Vec::new());
    }
    // the version does not name the width: every member must fit its candidate directory and the archive
    let rank = |width: usize| -> R<i32> {
        let step = width + 16;
        let table_end = 8 + n * step;
        if table_end > d.len() {
            return Ok(-1);
        }
        let mut has_data = false;
        for o in (8..table_end).step_by(step) {
            let (stored, size, offset) = (
                u32le(d, o + width + 4)?,
                u32le(d, o + width + 8)?,
                u32le(d, o + width + 12)?,
            );
            if stored == 0 && size & 0x1fff_ffff == 0 && offset == 0 {
                continue;
            }
            if offset < table_end || offset + stored > d.len() {
                return Ok(-1);
            }
            has_data |= stored != 0;
        }
        Ok(i32::from(has_data))
    };
    let wide = rank(128)?;
    let width = if wide == 1 {
        128
    } else {
        let short = rank(64)?;
        if wide < 0 && short < 0 {
            return red(format!(
                "{label}: ARC v7 has no complete directory with in-bounds members"
            ));
        }
        if wide >= short { 128 } else { 64 }
    };
    let mut out = Vec::new();
    for i in 0..n {
        let o = 8 + i * (width + 16);
        let name = sl(d, o, o + width);
        let name = name.split(|&b| b == 0).next().unwrap_or(&[]);
        let path: String = name.iter().map(|&b| b as char).collect();
        let (csz, usz, off) = (
            u32le(d, o + width + 4)?,
            u32le(d, o + width + 8)? & 0x1fff_ffff,
            u32le(d, o + width + 12)?,
        );
        let mut data = sl(d, off, off + csz).to_vec();
        if csz != usz {
            data = zlib_all(&data, &format!("{label}#{path}: invalid ARC zlib stream"))?;
        }
        if data.len() != usz {
            return red(format!(
                "{label}#{path}: {:#x} bytes, the entry says {usz:#x}",
                data.len()
            ));
        }
        out.push((path.replace('\\', "/"), data));
    }
    Ok(out)
}

/// `mtspk_shaders`: the shaders of MT Framework's package, back to back, code first or control first.
pub fn mtspk_shaders(d: &[u8], label: &str) -> R<Vec<Found>> {
    let mut out = Vec::new();
    let mut end: Option<usize> = None;
    let Some(mut c) = memfind(d, &CTL_MAGIC) else {
        return red(format!("{label} SPK: no shader control section"));
    };
    let ncode = u32le(d, c + 0x704)?;
    let control_first = c < ncode || sl(d, c - ncode, c - ncode + 4) != GFX_MAGIC;
    loop {
        if c + 0x708 > d.len() || sl(d, c, c + 4) != CTL_MAGIC {
            return red(format!("{label} SPK: no complete control header at {c:#x}"));
        }
        let (nctl, ncode) = (u32le(d, c + 0x20)?, u32le(d, c + 0x704)?);
        let g = if control_first {
            Some(c + nctl)
        } else {
            c.checked_sub(ncode)
        };
        let bad = nctl < 0x708
            || g.is_none_or(|g| {
                c + nctl > d.len() || g + ncode > d.len() || sl(d, g, g + 4) != GFX_MAGIC
            });
        if bad {
            return red(format!(
                "{label} SPK: no matching graphics code area for control at {c:#x}"
            ));
        }
        let g = g.unwrap_or(0);
        let start = if control_first { c } else { g };
        if let Some(e) = end
            && start != e
        {
            return red(format!(
                "{label} SPK: noncontiguous shader at {start:#x} after {e:#x}"
            ));
        }
        let (ctl, code) = (d[c..c + nctl].to_vec(), d[g..g + ncode].to_vec());
        let st = sph_stage(&code);
        let why = match st {
            Some(s) => check(s, &ctl, &code).map(str::to_string),
            None => Some("no stage in the SPH".into()),
        };
        if let Some(w) = why {
            return red(format!("{label} SPK @{start:#x}: {w}"));
        }
        out.push(Found {
            place: format!("{label}#MTSPK@{start:#x}"),
            stage: st.unwrap_or("vs"),
            ctl,
            code,
        });
        let e = if control_first { g + ncode } else { c + nctl };
        end = Some(e);
        if e == d.len() {
            return Ok(out);
        }
        match if control_first {
            Some(e)
        } else {
            memfind(&d[e.min(d.len())..], &CTL_MAGIC).map(|p| p + e)
        } {
            Some(n) => c = n,
            None => break,
        }
    }
    red(format!(
        "{label} SPK: last shader ends at {}, before the end of the package",
        end.unwrap_or(0)
    ))
}

/// `mtarc_shaders`: the SPK members of an MT Framework archive.
pub fn mtarc_shaders(d: &[u8], label: &str) -> R<Vec<Found>> {
    let mut out = Vec::new();
    for (path, m) in mtarc_members(d, label)? {
        if sl(&m, 0, 4) == b"SPK\0" {
            out.extend(mtspk_shaders(&m, &format!("{label}#{path}"))?);
        }
    }
    Ok(out)
}

/// `sdf_shaders`: RE Engine shader definitions (`SDF\0`, 0x108-byte entries from +0x10 with six stage offsets).
pub fn sdf_shaders(d: &[u8], label: &str) -> R<Vec<Found>> {
    let field = |at: usize, n: usize, what: &str| -> R<()> {
        if at + n > d.len() {
            return red(format!(
                "{what}: {n:#x} bytes at {at:#x} lie outside the {:#x} bytes there are",
                d.len()
            ));
        }
        Ok(())
    };
    field(4, 4, &format!("{label} SDF header"))?;
    let (ver, n) = (u16le(d, 4)?, u16le(d, 6)?);
    let mut out = Vec::new();
    if ver == 0 && n == 0 {
        return Ok(out);
    }
    if ver != 1 {
        return red(format!("{label}: SDF version {ver}"));
    }
    for i in 0..n {
        let at = 0x10 + 0x108 * i + 0x10;
        field(at, 48, &format!("{label} SDF entry {i}"))?;
        for (k, st) in ["vs", "hs", "ds", "gs", "ps", "cs"].into_iter().enumerate() {
            let o = u64le(d, at + 8 * k)? as usize;
            if o == 0 {
                continue;
            }
            let what = format!("{label} SDF entry {i} {st}");
            field(o, 8, &what)?;
            let (nctl, ncode) = (u32le(d, o)?, u32le(d, o + 4)?);
            let b = span(d, o + 8, nctl + ncode, &what)?;
            let (ctl, code) = (b[..nctl].to_vec(), b[nctl..].to_vec());
            let mut why = check(st, &ctl, &code).map(str::to_string);
            if why.is_none() && st != "cs" && sph_stage(&code) != Some(st) {
                why = Some("the SPH stage does not match the entry slot".into());
            }
            if let Some(w) = why {
                return red(format!("{label} SDF entry {i} {st}: {w}"));
            }
            out.push(Found {
                place: format!("{label}#SDF#var{i}"),
                stage: st,
                ctl,
                code,
            });
        }
    }
    Ok(out)
}
