//! The NRO modules a game registers (`nro_collect.py`): every SHA-256 the `.nrr` files of the RomFS list, found as
//! loose files or among the current files of a `data.arc` (Smash ARC format). Writes `nro<i>.nro` and `nro.tsv`. Cases the script
//! solves by scanning (no archive index, hashes left over) are [`Fail::Unsupported`].

mod image;
mod sdk;

use std::collections::BTreeMap;
use std::path::Path;

use image::Image;
use nxb_model::sha256;
use nxb_split::jobs::smash;
use nxb_split::src::Src;

/// Why a collection did not happen.
pub enum Fail {
    /// The script does it (exit code 4).
    Unsupported(String),
    /// The script's `[RED]` or exit 1 case.
    Error(String),
}

fn un<T: ToString>(e: T) -> Fail {
    Fail::Unsupported(e.to_string())
}

/// What a collection found.
pub struct Summary {
    /// NROs written.
    pub written: usize,
    /// NROs of online SDKs, listed but not written.
    pub refused: usize,
    /// Bytes of every registered NRO found.
    pub bytes: usize,
}

fn nro_size(h: &[u8]) -> u64 {
    if h.len() < 0x40 || &h[0x10..0x14] != b"NRO0" {
        return 0;
    }
    let n = u64::from(u32::from_le_bytes([h[0x18], h[0x19], h[0x1a], h[0x1b]]));
    if n > 0x40 && n < 1 << 30 { n } else { 0 }
}

/// `smash_arc.read`.
fn read(s: &dyn Src, e: &smash::Entry) -> Result<Vec<u8>, Fail> {
    let d = s.read_up(e.1, e.2 as usize).map_err(un)?;
    if d.get(..4) == Some(&[0x28, 0xb5, 0x2f, 0xfd][..]) {
        nxb_split::zstd::decompress(&d).map_err(un)
    } else {
        Ok(d[..d.len().min(e.3 as usize)].to_vec())
    }
}

/// `nro_collect.py <RomFS image> <out dir> [<nrr name>]`, the output folder is written whole or not at all.
pub fn collect(romfs: &Path, out: &Path, only: Option<&str>) -> Result<Summary, Fail> {
    let img = Image::open(romfs).map_err(un)?;
    let mut want: BTreeMap<[u8; 32], String> = BTreeMap::new();
    for it in &img.files {
        let name = it.rel.rsplit('/').next().unwrap_or("");
        if !name.ends_with(".nrr") || only.is_some_and(|o| o != name) {
            continue;
        }
        let d = img.src(it).all().map_err(un)?;
        if d.get(..4) != Some(b"NRR0") {
            continue;
        }
        let rd = |o: usize| {
            d.get(o..o + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        let (Some(off), Some(cnt)) = (rd(0x340), rd(0x344)) else {
            return Err(un("truncated NRR"));
        };
        for i in 0..cnt as usize {
            let a = off as usize + 32 * i;
            let h: [u8; 32] = d
                .get(a..a + 32)
                .and_then(|s| s.try_into().ok())
                .ok_or_else(|| un("truncated NRR hash list"))?;
            want.entry(h).or_insert_with(|| name.to_string());
        }
    }
    if want.is_empty() {
        return Err(Fail::Error(format!(
            "no NRR file with hashes in {}",
            romfs.display()
        )));
    }
    let mut found: BTreeMap<[u8; 32], (String, &str, Vec<u8>)> = BTreeMap::new();
    let (mut indexed, mut big) = (0usize, 0usize);
    let nro_ext = hash40(b"nro");
    for it in &img.files {
        let name = it.rel.rsplit('/').next().unwrap_or("");
        if name.ends_with(".nrr") || it.size < 0x40 {
            continue;
        }
        let src = img.src(it);
        let head = src.read_up(0, 0x40).map_err(un)?;
        if nro_size(&head) == it.size {
            let body = src.all().map_err(un)?;
            let h = sha256(&body).0;
            if want.contains_key(&h) {
                found.insert(h, (it.rel.clone(), "loose", body));
            }
        } else if head.get(..8) == Some(&smash::MAGIC[..]) {
            indexed += 1;
            for e in smash::files(src.as_ref(), false)
                .map_err(un)?
                .unwrap_or_default()
            {
                if e.4 != nro_ext {
                    continue;
                }
                let body = read(src.as_ref(), &e)?;
                let h = sha256(&body).0;
                if want.contains_key(&h) {
                    found.insert(h, (format!("{}#{:010x}", it.rel, e.0), "index", body));
                } else {
                    eprintln!(
                        "[RED] {}#{:010x}: an NRO that no NRR registers",
                        it.rel, e.0
                    );
                }
            }
        } else {
            big += 1;
        }
    }
    if indexed > 0 {
        println!(
            "{} NROs among the current files of {indexed} indexed archive(s); {} registered hashes are superseded copies, left out",
            found.len(),
            want.len() - found.len()
        );
        want.retain(|h, _| found.contains_key(h));
    }
    if want.keys().any(|h| !found.contains_key(h)) {
        return Err(un(format!(
            "registered NROs left over ({big} files to scan): the script scans for them"
        )));
    }
    let tmp = out.with_extension("partial");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(un)?;
    let (mut rows, mut i, mut refused, mut bytes) = (String::new(), 0, 0, 0);
    for (h, (wher, how, body)) in &found {
        bytes += body.len();
        let hex = nxb_model::Digest(*h).to_hex();
        let nrr = &want[h];
        if let Some(what) = sdk::online(body) {
            rows += &format!(
                "refused\t{hex}\t{}\t{nrr}\t{how} {wher} ({what})\n",
                body.len()
            );
            refused += 1;
            continue;
        }
        let name = format!("nro{i}");
        i += 1;
        std::fs::write(tmp.join(format!("{name}.nro")), body).map_err(un)?;
        rows += &format!("{name}\t{hex}\t{}\t{nrr}\t{how} {wher}\n", body.len());
    }
    std::fs::write(
        tmp.join("nro.tsv"),
        format!("# name\tsha256\tsize\tnrr\twhere\n{rows}"),
    )
    .map_err(un)?;
    let _ = std::fs::remove_dir_all(out);
    std::fs::rename(&tmp, out).map_err(un)?;
    Ok(Summary {
        written: i,
        refused,
        bytes,
    })
}

/// `smash_arc.hash40`: CRC-32 and length.
fn hash40(s: &[u8]) -> u64 {
    u64::from(crc32(s)) | (s.len() as u64) << 32
}

fn crc32(d: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in d {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                c >> 1 ^ 0xedb8_8320
            } else {
                c >> 1
            };
        }
    }
    !c
}

#[cfg(test)]
mod tests {
    #[test]
    fn hash40_of_nro() {
        assert_eq!(
            super::hash40(b"nro"),
            u64::from(super::crc32(b"nro")) | 3 << 32
        );
        assert_eq!(super::crc32(b"123456789"), 0xcbf4_3926);
    }
}
