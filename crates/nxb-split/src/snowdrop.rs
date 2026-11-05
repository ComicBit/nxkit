//! Ubisoft Snowdrop: BERG data slices with compressed shader records (`snowdrop_wrapped`, `snowdrop_shaders`).

use crate::SplitError;
use crate::glslc::stage;
use crate::rd::{red, sl, u32le, zlib_all};
use crate::shaders::{CTL_MAGIC, Found, check, find_all};

type R<T> = Result<T, SplitError>;

const RECORD: [u8; 4] = [0xc5, 0x00, 0xcd, 0xab];

fn is_record(d: &[u8], at: usize) -> bool {
    matches!(d.get(at), Some(0xc5 | 0xc6)) && sl(d, at + 1, at + 4) == &RECORD[1..]
}

fn records(d: &[u8]) -> Vec<usize> {
    // `re.finditer(b'[\xc5\xc6]\x00\xcd\xab', d)`: four bytes, matches cannot overlap
    let mut out = Vec::new();
    let mut i = 0;
    while i + 4 <= d.len() {
        if is_record(d, i) {
            out.push(i);
            i += 4;
        } else {
            i += 1;
        }
    }
    out
}

/// (start, end, bytes) of the zstd frames in `d` that start with a cache record.
fn wrapped(d: &[u8]) -> Vec<(usize, usize, Vec<u8>)> {
    let mut out = Vec::new();
    for h in find_all(d, &[0x28, 0xb5, 0x2f, 0xfd]) {
        let mut buf = Vec::new();
        let Ok((_, used)) = crate::zstd::decompress_one(&d[h..], 1 << 31, &mut |b| {
            buf.extend_from_slice(b);
            Ok(())
        }) else {
            continue; // a magic inside another compressed asset
        };
        if buf.len() < 20 || !is_record(&buf, 0) {
            continue;
        }
        out.push((h, h + used, buf));
    }
    out
}

/// `snowdrop_shaders`: records with a 20-byte header and zlib or zstd data, found by their header.
pub fn snowdrop_shaders(d: &[u8], label: &str, wrap: bool) -> R<Vec<Found>> {
    let chunks = if wrap { wrapped(d) } else { Vec::new() };
    let mut out = Vec::new();
    let mut ci = 0;
    for h in records(d) {
        while ci < chunks.len() && h >= chunks[ci].1 {
            ci += 1;
        }
        if ci < chunks.len() && h >= chunks[ci].0 {
            continue;
        }
        if h + 24 > d.len() {
            continue;
        }
        let zst = d[h] == 0xc6;
        if zst {
            if d[h + 20..h + 24] != [0x28, 0xb5, 0x2f, 0xfd] {
                continue;
            }
        } else if d[h + 20] & 0xf != 8
            || (usize::from(d[h + 20]) << 8 | usize::from(d[h + 21])) % 31 != 0
        {
            continue;
        }
        let (nraw, nstored) = (u32le(d, h + 12)?, u32le(d, h + 16)?);
        if nstored == 0 || h + 20 + nstored > d.len() {
            return red(format!(
                "{label} Snowdrop record @{h:#x}: stored bytes out of bounds"
            ));
        }
        let stored = &d[h + 20..h + 20 + nstored];
        let raw = if zst {
            crate::zstd::decompress(stored)?
        } else {
            zlib_all(stored, label)?
        };
        if raw.len() != nraw {
            return red(format!(
                "{label} Snowdrop record @{h:#x}: expected {nraw} raw bytes, got {}",
                raw.len()
            ));
        }
        for c in find_all(&raw, &CTL_MAGIC) {
            if c + 0x708 > raw.len() {
                continue;
            }
            let (nc, nk) = (u32le(&raw, c + 0x20)?, u32le(&raw, c + 0x704)?);
            if nc < 0x760 || nc & 3 != 0 || nk < 0x34 || c + nc + nk > raw.len() {
                continue;
            }
            let (ctl, code) = (&raw[c..c + nc], &raw[c + nc..c + nc + nk]);
            if let Some(st) = stage(ctl, code)
                && check(st, ctl, code).is_none()
            {
                out.push(Found {
                    place: format!("{label}#SNOWDROP@{h:#x}:{c:#x}"),
                    stage: st,
                    ctl: ctl.to_vec(),
                    code: code.to_vec(),
                });
            }
        }
    }
    for (h, _, raw) in &chunks {
        out.extend(snowdrop_shaders(
            raw,
            &format!("{label}#SDFZSTD@{h:#x}"),
            false,
        )?);
    }
    Ok(out)
}
