//! Unreal Engine 3 shader caches and packages (`ue3_package`, `ue3_body`, `ue3_shaders` of `shader_split.py`).
//! Chunk blocks that are not zlib need the game's own Oodle decoder: `Unsupported`.

use crate::SplitError;
use crate::shaders::{
    CS_MAGIC, CTL_MAGIC, Found, GFX_MAGIC, check, find_all, memfind, span, sph_stage, u32le,
};

const TAG: [u8; 4] = [0xc1, 0x83, 0x2a, 0x9e];

fn red(m: String) -> SplitError {
    SplitError::Red(m)
}

/// Whether the first bytes of a file start an Unreal Engine 3 package.
pub fn is_package(head: &[u8]) -> bool {
    head.starts_with(&TAG)
        && head.len() >= 6
        && matches!(u16::from_le_bytes([head[4], head[5]]), 1..0x8000)
}

/// Whether a file is a shader cache or a package (`BMSG` or a package tag).
pub fn is_ue3(d: &[u8]) -> bool {
    d.starts_with(b"BMSG") || is_package(d)
}

/// `ue3_body`: a package with its compressed chunks decoded (a package without chunks as it is).
pub fn body(d: &[u8], label: &str) -> Result<Vec<u8>, SplitError> {
    let Some(first) = memfind(&d[4.min(d.len())..], &TAG).map(|x| x + 4) else {
        return Ok(d.to_vec());
    };
    let mut out = d[..first].to_vec();
    let mut i = first;
    while i < d.len() {
        if d.get(i..i + 4) != Some(&TAG[..]) || i + 16 > d.len() {
            return Err(red(format!(
                "{label}: no compressed chunk at {i:#x} (the chunks do not run to the end of the package)"
            )));
        }
        let (bs, us) = (
            u32le(d, i + 4).unwrap_or(0) as usize,
            u32le(d, i + 12).unwrap_or(0) as usize,
        );
        if bs == 0 {
            return Err(red(format!("{label}: chunk at {i:#x} has block size 0")));
        }
        let nb = us.div_ceil(bs);
        let mut o = i + 16 + 8 * nb;
        for k in 0..nb {
            let c = u32le(d, i + 16 + 8 * k)
                .ok_or_else(|| SplitError::Format("chunk table past the end".into()))?
                as usize;
            let u = u32le(d, i + 20 + 8 * k).unwrap_or(0) as usize;
            let blk = span(d, o, c, &format!("{label} chunk at {i:#x} block {k}"))?;
            if c == u {
                out.extend_from_slice(blk);
            } else if blk.first() == Some(&0x78) {
                let z = crate::zlibx::zlib_stream(blk, 1 << 30).ok_or_else(|| {
                    SplitError::Format(format!(
                        "{label}: damaged zlib block {k} of the chunk at {i:#x}"
                    ))
                })?;
                out.extend(z);
            } else {
                out.extend(crate::oodle::decode(
                    blk,
                    u,
                    &format!("{label}: chunk at {i:#x} block {k}"),
                )?);
            }
            o += c;
        }
        i = o;
    }
    Ok(out)
}

/// `ue3_shaders`: records of a shader cache.
pub fn shaders(d: &[u8], label: &str) -> Result<Vec<Found>, SplitError> {
    let mut out = Vec::new();
    for c in find_all(d, &CTL_MAGIC) {
        if c + 0x708 > d.len() {
            continue;
        }
        let n = u32le(d, c + 0x704).unwrap_or(0) as usize;
        let Some(a) = c.checked_sub(n).filter(|&a| a >= 12) else {
            continue;
        };
        let head = d.get(a..a + 4);
        if head != Some(&GFX_MAGIC[..]) && head != Some(&CS_MAGIC[..]) {
            continue;
        }
        let nctl = [4usize, 8].into_iter().find_map(|k| {
            let at = a.checked_sub(k + 4)?;
            let (ln, lc) = (u32le(d, at)? as usize, u32le(d, at + 4)? as usize);
            (ln == n && (0x760..=0x1000).contains(&lc)).then_some(lc)
        });
        let Some(nctl) = nctl.filter(|&k| c + k <= d.len()) else {
            continue;
        };
        let (ctl, code) = (d[c..c + nctl].to_vec(), d[a..c].to_vec());
        let st = if code.starts_with(&CS_MAGIC) {
            Some("cs")
        } else {
            sph_stage(&code)
        };
        let why = match st {
            Some(s) => check(s, &ctl, &code).map(str::to_string),
            None => Some("no stage in the code area header".to_string()),
        };
        if let Some(w) = why {
            return Err(red(format!(
                "{label}: shader record at {c:#x} ({}): {w}",
                st.unwrap_or("None")
            )));
        }
        out.push(Found {
            place: format!("{label}#UE3@{c:x}"),
            stage: st.unwrap_or("vs"),
            ctl,
            code,
        });
    }
    Ok(out)
}
