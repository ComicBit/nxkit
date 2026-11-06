//! The Unreal paks of a RomFS that `profile_guess.py` `build_sources` counts (`gfx/tools/uepak.py` `unreal_paks`):
//! those whose shaders are in shader code archives (the game decodes them) and those that keep them inline in the
//! cooked packages (the shader split reads them file by file). A pak that is too short or has no Unreal footer
//! counts for neither.

use super::rom::{Rom, RomFile, dirname};

const PAK_MAGIC: u32 = 0x5A6F_12E1;
const TOC_MAGIC: &[u8; 16] = b"-==--==--==--==-";
/// `EIoChunkType` of a shader library (TOC version 4 and later) and of the cooked packages' exports (TOC version 4
/// and later, and Unreal Engine 4.27's numbering before).
const IO_SHADER_LIBRARY: u8 = 8;
const IO_EXPORT_BUNDLE: u8 = 1;
const IO_EXPORT_BUNDLE_427: u8 = 2;

/// How many paks keep their shaders in archives and how many inline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PakCounts {
    /// Paks with shader code archives.
    pub archives: u64,
    /// Paks with the shaders inline in the cooked packages.
    pub inline: u64,
}

/// `unreal_paks(romfs, inline=False)` and `unreal_paks(romfs, inline=True)`.
pub fn count_paks(rom: &Rom) -> PakCounts {
    let mut c = PakCounts::default();
    for f in rom.files().iter().filter(|f| f.path.ends_with(".pak")) {
        match inline_shaders(rom, f) {
            Some(true) => c.inline += 1,
            Some(false) => c.archives += 1,
            None => {}
        }
    }
    c
}

fn le(b: &[u8], o: usize, n: usize) -> Option<u64> {
    let s = b.get(o..o.checked_add(n)?)?;
    Some(s.iter().rev().fold(0u64, |a, &x| a << 8 | u64::from(x)))
}

/// A range of a file, checked against its size.
fn range(rom: &Rom, f: &RomFile, off: u64, n: u64) -> Option<Vec<u8>> {
    if off.checked_add(n)? > f.size {
        return None;
    }
    rom.read(f, off, n).ok()
}

/// `footer`: `(index is encrypted, index offset, index size, version)`.
fn footer(rom: &Rom, f: &RomFile) -> Option<(bool, u64, u64, u32)> {
    let size = f.size;
    let (back, ver) = [(0x2cu64, 0x2du64), (0xc8, 0xc8 + 21), (0xcc, 0xcc + 21)]
        .into_iter()
        .filter(|&(_, foot)| size >= foot)
        .find_map(|(back, _)| {
            let h = rom.read(f, size - back, 8).ok()?;
            (le(&h, 0, 4)? as u32 == PAK_MAGIC).then(|| (back, le(&h, 4, 4).unwrap_or(0) as u32))
        })?;
    if ((ver == 4 || ver == 11) && ((ver == 4) != (back == 0x2c))) || (ver == 8 && back == 0x2c) {
        return None;
    }
    let (enc, ioff, isz) = match ver {
        4 => {
            let ft = rom.read(f, size - 0x2d, 0x2d).ok()?;
            (ft[0] != 0, le(&ft, 9, 8)?, le(&ft, 17, 8)?)
        }
        8 | 11 => {
            let ft = rom.read(f, size - (back + 21), back + 21).ok()?;
            (ft[20] != 0, le(&ft, 29, 8)?, le(&ft, 37, 8)?)
        }
        _ => return None,
    };
    (ioff.checked_add(isz)? <= size).then_some((enc, ioff, isz, ver))
}

/// A reader over a table: a read past its end is a damaged file (`class R`).
struct Table<'a>(&'a [u8], usize);

impl Table<'_> {
    fn raw(&mut self, n: u64) -> Option<&[u8]> {
        let n = usize::try_from(n).ok()?;
        let s = self.0.get(self.1..self.1.checked_add(n)?)?;
        self.1 += n;
        Some(s)
    }
    fn le(&mut self, n: usize) -> Option<u64> {
        le(self.raw(n as u64)?, 0, n)
    }
    fn i32(&mut self) -> Option<i32> {
        Some(self.le(4)? as u32 as i32)
    }
    /// `fstr`: a length-prefixed string, UTF-16 when the length is negative. A bad UTF-16 string is a damaged file.
    fn fstr(&mut self) -> Option<()> {
        let n = self.i32()?;
        if n < 0 {
            let b = self.raw(u64::from(n.unsigned_abs()) * 2)?;
            let u: Vec<u16> = b
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .collect();
            String::from_utf16(&u).ok()?;
        } else {
            self.raw(n as u64)?;
        }
        Some(())
    }
}

fn has(d: &[u8], pat: &[u8]) -> bool {
    d.windows(pat.len()).any(|w| w == pat)
}

/// `inline_shaders`: whether the pak keeps its shaders inline in its cooked packages. `None`: too short, or no pak
/// footer, or an index that does not parse.
fn inline_shaders(rom: &Rom, f: &RomFile) -> Option<bool> {
    let (enc, ioff, isz, ver) = footer(rom, f)?;
    if ver == 4 || ver == 8 {
        return Some(true);
    }
    if enc {
        return Some(false);
    }
    let ib = range(rom, f, ioff, isz)?;
    let mut r = Table(&ib, 0);
    r.fstr()?;
    r.raw(12)?;
    if r.i32()? != 0 {
        r.raw(16)?;
        r.raw(20)?;
    }
    if r.i32()? == 0 {
        return Some(false);
    }
    let (doff, dsz) = (r.le(8)?, r.le(8)?);
    let d = range(rom, f, doff, dsz)?;
    Some(
        !has(&d, b".ushaderbytecode\0")
            && (has(&d, b".uexp\0")
                || (has(&d, b"GlobalShaderCache-") && inline_in_containers(rom, f))),
    )
}

/// `inline_in_containers`: the IoStore containers in the pak's folder hold its cooked packages with their shaders
/// inline: export bundles, and no shader libraries.
fn inline_in_containers(rom: &Rom, pak: &RomFile) -> bool {
    let folder = dirname(&pak.path);
    let (mut exports, mut libs) = (false, false);
    for t in rom
        .files()
        .iter()
        .filter(|t| t.path.ends_with(".utoc") && dirname(&t.path) == folder)
    {
        if let Some((e, l)) = toc_chunks(rom, t) {
            exports |= e;
            libs |= l;
        }
    }
    exports && !libs
}

/// Whether an IoStore TOC has export chunks and whether it has shader library chunks. `None`: not a TOC, or a
/// damaged one.
fn toc_chunks(rom: &Rom, t: &RomFile) -> Option<(bool, bool)> {
    let h = rom.head(t, 100).ok()?;
    if h.get(..16)? != TOC_MAGIC {
        return None;
    }
    let version = h[16];
    let w: Vec<u64> = (0..9)
        .map(|i| le(&h, 20 + 4 * i, 4))
        .collect::<Option<_>>()?;
    let (hsz, n, bsz, nparts) = (w[0], w[1], w[3], w[8]);
    if bsz != 12 {
        return None;
    }
    h.get(80)?;
    if version >= 4 {
        le(&h, 84, 4)?;
    }
    if version >= 3 && nparts > 1 {
        le(&h, 88, 8)?;
    }
    if version >= 5 {
        le(&h, 96, 4)?;
    }
    let ids = rom.read(t, hsz, n.checked_mul(12)?).ok()?;
    let export = if version >= 4 {
        IO_EXPORT_BUNDLE
    } else {
        IO_EXPORT_BUNDLE_427
    };
    let kind = |k: u8| ids.as_chunks::<12>().0.iter().any(|c| c[11] == k);
    Some((kind(export), version >= 4 && kind(IO_SHADER_LIBRARY)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A version 11 pak: the index (mount point, entry count, seed, no path hash index, a directory index at `dir`).
    fn pak11(dir_names: &[u8]) -> Vec<u8> {
        let mut b = vec![0u8; 16];
        let ioff = b.len() as u64;
        let mut idx = Vec::new();
        idx.extend(1i32.to_le_bytes());
        idx.push(0);
        idx.extend([0u8; 12]);
        idx.extend(0i32.to_le_bytes());
        idx.extend(1i32.to_le_bytes());
        let doff = ioff + idx.len() as u64 + 16;
        idx.extend(doff.to_le_bytes());
        idx.extend((dir_names.len() as u64).to_le_bytes());
        b.extend(&idx);
        b.extend(dir_names);
        let isz = idx.len() as u64;
        let mut ft = vec![0u8; 21];
        ft.extend(PAK_MAGIC.to_le_bytes());
        ft.extend(11u32.to_le_bytes());
        ft.extend(ioff.to_le_bytes());
        ft.extend(isz.to_le_bytes());
        ft.extend([0u8; 20]);
        ft.extend(vec![0u8; 0xc8 - 8 - 16 - 20]);
        b.extend(ft);
        b
    }

    fn toc(version: u8, types: &[u8]) -> Vec<u8> {
        let mut b = vec![0u8; 144];
        b[..16].copy_from_slice(TOC_MAGIC);
        b[16] = version;
        let w = [144u32, types.len() as u32, 0, 12, 0, 0, 0, 0, 1];
        for (i, v) in w.iter().enumerate() {
            b[20 + 4 * i..24 + 4 * i].copy_from_slice(&v.to_le_bytes());
        }
        for t in types {
            let mut id = [0u8; 12];
            id[11] = *t;
            b.extend(id);
        }
        b
    }

    #[test]
    fn an_index_that_names_shader_code_archives_is_a_plain_pak() {
        let r = Rom::in_memory(vec![("/a.pak", pak11(b"x/.ushaderbytecode\0"))]);
        assert_eq!(
            count_paks(&r),
            PakCounts {
                archives: 1,
                inline: 0
            }
        );
    }

    #[test]
    fn cooked_packages_without_archives_are_inline() {
        let r = Rom::in_memory(vec![("/a.pak", pak11(b"x/m.uexp\0"))]);
        assert_eq!(
            count_paks(&r),
            PakCounts {
                archives: 0,
                inline: 1
            }
        );
    }

    #[test]
    fn the_global_shader_cache_is_inline_only_with_export_bundles_and_no_libraries() {
        let g = || pak11(b"GlobalShaderCache-SF_METAL.bin\0");
        let with = |toc_types: &[u8]| {
            count_paks(&Rom::in_memory(vec![
                ("/d/a.pak", g()),
                ("/d/a.utoc", toc(4, toc_types)),
            ]))
        };
        assert_eq!(
            with(&[1, 3]),
            PakCounts {
                archives: 0,
                inline: 1
            }
        );
        assert_eq!(
            with(&[1, 8]),
            PakCounts {
                archives: 1,
                inline: 0
            }
        );
        assert_eq!(
            with(&[3]),
            PakCounts {
                archives: 1,
                inline: 0
            }
        );
        let elsewhere = Rom::in_memory(vec![("/d/a.pak", g()), ("/e/a.utoc", toc(4, &[1]))]);
        assert_eq!(
            count_paks(&elsewhere),
            PakCounts {
                archives: 1,
                inline: 0
            }
        );
    }

    #[test]
    fn a_version_3_toc_numbers_its_export_chunks_as_unreal_4_27_does() {
        let r = Rom::in_memory(vec![
            ("/d/a.pak", pak11(b"GlobalShaderCache-SF_METAL.bin\0")),
            ("/d/a.utoc", toc(3, &[2, 8])),
        ]);
        assert_eq!(
            count_paks(&r),
            PakCounts {
                archives: 0,
                inline: 1
            }
        );
    }

    #[test]
    fn a_file_without_an_unreal_footer_counts_for_neither() {
        let r = Rom::in_memory(vec![("/a.pak", vec![0u8; 300]), ("/b.pak", vec![1u8; 10])]);
        assert_eq!(count_paks(&r), PakCounts::default());
    }
}
