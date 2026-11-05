//! Thin 64-bit Mach-O parsing and the layout `codesign_allocate` gives a signed file.
//!
//! Only what signing needs is read: the `__TEXT` and `__LINKEDIT` segments, `LC_CODE_SIGNATURE`,
//! `LC_UUID`, the platform and minimum OS version, and the section offsets (to know whether a
//! new load command fits in the header padding). Universal ("fat") files and 32-bit images are
//! [`Error::Unsupported`].

use crate::error::{Error, Result, format_err};
use crate::hash::HashType;

const MH_MAGIC_64: u32 = 0xfeed_facf;
const MH_CIGAM_64: u32 = 0xcffa_edfe;
const MH_MAGIC: u32 = 0xfeed_face;
const FAT_MAGIC: u32 = 0xcafe_babe;
const FAT_MAGIC_64: u32 = 0xcafe_babf;
const MH_EXECUTE: u32 = 2;
const LC_SEGMENT_64: u32 = 0x19;
const LC_UUID: u32 = 0x1b;
const LC_CODE_SIGNATURE: u32 = 0x1d;
const LC_VERSION_MIN_MACOSX: u32 = 0x24;
const LC_VERSION_MIN_IPHONEOS: u32 = 0x25;
const LC_VERSION_MIN_TVOS: u32 = 0x2f;
const LC_VERSION_MIN_WATCHOS: u32 = 0x30;
const LC_BUILD_VERSION: u32 = 0x32;
const CPU_TYPE_ARM64: u32 = 0x0100_000c;

/// `LC_BUILD_VERSION` platform numbers that matter here.
pub mod platform {
    /// macOS.
    pub const MACOS: u32 = 1;
    /// iOS (device).
    pub const IOS: u32 = 2;
    /// tvOS (device).
    pub const TVOS: u32 = 3;
    /// watchOS (device).
    pub const WATCHOS: u32 = 4;
    /// bridgeOS.
    pub const BRIDGEOS: u32 = 5;
    /// Mac Catalyst.
    pub const MACCATALYST: u32 = 6;
    /// iOS simulator.
    pub const IOSSIMULATOR: u32 = 7;
    /// tvOS simulator.
    pub const TVOSSIMULATOR: u32 = 8;
    /// watchOS simulator.
    pub const WATCHOSSIMULATOR: u32 = 9;
    /// visionOS (device).
    pub const VISIONOS: u32 = 11;
}

/// A segment's file range and where its load command is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment {
    /// Offset of the `segment_command_64` in the file.
    pub cmd_offset: usize,
    /// `fileoff`.
    pub fileoff: u64,
    /// `filesize`.
    pub filesize: u64,
    /// `vmsize`.
    pub vmsize: u64,
}

/// What signing needs to know about a thin 64-bit Mach-O image.
#[derive(Clone, Debug)]
pub struct MachO {
    /// `cputype`.
    pub cputype: u32,
    /// `filetype` (`MH_EXECUTE` = 2, `MH_DYLIB` = 6, ...).
    pub filetype: u32,
    /// `ncmds`.
    pub ncmds: u32,
    /// `sizeofcmds`.
    pub sizeofcmds: u32,
    /// The `__TEXT` segment.
    pub text: Option<Segment>,
    /// The `__LINKEDIT` segment.
    pub linkedit: Segment,
    /// `LC_CODE_SIGNATURE`: (command offset, `dataoff`, `datasize`).
    pub code_signature: Option<(usize, u32, u32)>,
    /// `LC_UUID`.
    pub uuid: Option<[u8; 16]>,
    /// Platform from `LC_BUILD_VERSION` or an `LC_VERSION_MIN_*` command.
    pub platform: Option<u32>,
    /// Minimum OS version, `xxxx.yy.zz` nibble-packed as in the load command.
    pub minos: Option<u32>,
    /// SDK version from `LC_BUILD_VERSION` or `LC_VERSION_MIN_*` (the hardened runtime version).
    pub sdk: Option<u32>,
    /// SHA-1 of the 28-byte `mach_header` prefix and the load commands, `codesign`'s
    /// identification of an image without `LC_UUID`.
    pub commands_sha1: [u8; 20],
    /// Lowest file offset of any section with file content (end of the header padding).
    pub first_section_offset: Option<u64>,
    /// Whether a `__TEXT,__info_plist` section exists (an embedded Info.plist).
    pub has_embedded_info_plist: bool,
}

fn u32le(b: &[u8], o: usize) -> Result<u32> {
    b.get(o..o + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| format_err("Mach-O truncated"))
}

fn u64le(b: &[u8], o: usize) -> Result<u64> {
    b.get(o..o + 8)
        .map(|s| {
            let mut a = [0u8; 8];
            a.copy_from_slice(s);
            u64::from_le_bytes(a)
        })
        .ok_or_else(|| format_err("Mach-O truncated"))
}

fn name16(b: &[u8], o: usize) -> Result<&[u8]> {
    let s = b
        .get(o..o + 16)
        .ok_or_else(|| format_err("Mach-O truncated"))?;
    Ok(&s[..s.iter().position(|c| *c == 0).unwrap_or(16)])
}

/// Whether `data` starts like a Mach-O or universal file (any width or byte order).
pub fn is_macho(data: &[u8]) -> bool {
    if data.len() < 4 {
        return false;
    }
    let be = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    let le = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    matches!(be, FAT_MAGIC | FAT_MAGIC_64)
        || matches!(le, MH_MAGIC_64 | MH_MAGIC | MH_CIGAM_64)
        || matches!(be, MH_MAGIC_64 | MH_MAGIC)
}

impl MachO {
    /// Parses the header and load commands of a thin 64-bit little-endian image.
    pub fn parse(data: &[u8]) -> Result<MachO> {
        if data.len() < 32 {
            return Err(format_err("file too short for a Mach-O header"));
        }
        let be = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        if matches!(be, FAT_MAGIC | FAT_MAGIC_64) {
            return Err(Error::Unsupported(
                "universal (fat) Mach-O: use crate::fat".into(),
            ));
        }
        let magic = u32le(data, 0)?;
        if magic != MH_MAGIC_64 {
            return Err(if magic == MH_MAGIC {
                Error::Unsupported("32-bit Mach-O".into())
            } else {
                format_err("not a little-endian 64-bit Mach-O")
            });
        }
        let cputype = u32le(data, 4)?;
        let filetype = u32le(data, 12)?;
        let ncmds = u32le(data, 16)?;
        let sizeofcmds = u32le(data, 20)?;
        let mut m = MachO {
            cputype,
            filetype,
            ncmds,
            sizeofcmds,
            text: None,
            linkedit: Segment {
                cmd_offset: 0,
                fileoff: 0,
                filesize: 0,
                vmsize: 0,
            },
            code_signature: None,
            uuid: None,
            platform: None,
            minos: None,
            sdk: None,
            commands_sha1: [0; 20],
            first_section_offset: None,
            has_embedded_info_plist: false,
        };
        let mut have_linkedit = false;
        let end = 32 + sizeofcmds as usize;
        if end > data.len() {
            return Err(format_err("load commands run past the end of the file"));
        }
        // `sizeof(struct mach_header)` (28) of the header, as Security's MachORep hashes it.
        let mut h = crate::hash::Sha1::new();
        h.update(&data[..28]);
        h.update(&data[32..end]);
        m.commands_sha1 = h.finalize();
        let mut off = 32;
        for _ in 0..ncmds {
            let cmd = u32le(data, off)?;
            let size = u32le(data, off + 4)? as usize;
            if size < 8 || off + size > end {
                return Err(format_err("bad load command size"));
            }
            match cmd {
                LC_SEGMENT_64 => {
                    let seg = Segment {
                        cmd_offset: off,
                        vmsize: u64le(data, off + 32)?,
                        fileoff: u64le(data, off + 40)?,
                        filesize: u64le(data, off + 48)?,
                    };
                    let segname = name16(data, off + 8)?;
                    let nsects = u32le(data, off + 64)? as usize;
                    for s in 0..nsects {
                        let so = off + 72 + s * 80;
                        if so + 80 > off + size {
                            return Err(format_err("section table past its segment command"));
                        }
                        let sectname = name16(data, so)?;
                        let flags = u32le(data, so + 64)?;
                        let offset = u32le(data, so + 48)? as u64;
                        let kind = flags & 0xff;
                        // Zero-fill sections (S_ZEROFILL 1, S_GB_ZEROFILL 0xc, S_THREAD_LOCAL_ZEROFILL 0x12)
                        // have no file content.
                        if offset > 0 && !matches!(kind, 0x1 | 0xc | 0x12) {
                            m.first_section_offset =
                                Some(m.first_section_offset.map_or(offset, |o| o.min(offset)));
                        }
                        if segname == b"__TEXT" && sectname == b"__info_plist" {
                            m.has_embedded_info_plist = true;
                        }
                    }
                    if segname == b"__TEXT" {
                        m.text = Some(seg);
                    } else if segname == b"__LINKEDIT" {
                        m.linkedit = seg;
                        have_linkedit = true;
                    }
                }
                LC_CODE_SIGNATURE => {
                    m.code_signature = Some((off, u32le(data, off + 8)?, u32le(data, off + 12)?));
                }
                LC_UUID => {
                    let u = data
                        .get(off + 8..off + 24)
                        .ok_or_else(|| format_err("short LC_UUID"))?;
                    let mut a = [0u8; 16];
                    a.copy_from_slice(u);
                    m.uuid = Some(a);
                }
                LC_BUILD_VERSION => {
                    m.platform = Some(u32le(data, off + 8)?);
                    m.minos = Some(u32le(data, off + 12)?);
                    m.sdk = Some(u32le(data, off + 16)?);
                }
                LC_VERSION_MIN_MACOSX
                | LC_VERSION_MIN_IPHONEOS
                | LC_VERSION_MIN_TVOS
                | LC_VERSION_MIN_WATCHOS => {
                    m.platform = Some(match cmd {
                        LC_VERSION_MIN_MACOSX => platform::MACOS,
                        LC_VERSION_MIN_IPHONEOS => platform::IOS,
                        LC_VERSION_MIN_TVOS => platform::TVOS,
                        _ => platform::WATCHOS,
                    });
                    m.minos = Some(u32le(data, off + 8)?);
                    m.sdk = Some(u32le(data, off + 12)?);
                }
                _ => {}
            }
            off += size;
        }
        if !have_linkedit {
            return Err(format_err("no __LINKEDIT segment"));
        }
        Ok(m)
    }

    /// Whether the image is a main executable (`MH_EXECUTE`).
    pub fn is_main_binary(&self) -> bool {
        self.filetype == MH_EXECUTE
    }

    /// The code page size `codesign` uses, as a power of two.
    ///
    /// Measured: 16 KiB (shift 14) for arm64 images, except iOS before 16.0 and tvOS (every
    /// version up to 26), which get 4 KiB, 4 KiB (shift 12) for x86_64 and other CPUs. Checked
    /// on macOS, Mac Catalyst, iOS 10.3 to 17 and 26, the iOS simulator, tvOS 14 to 26,
    /// watchOS 9 and 26 (arm64), visionOS 1 and 26. bridgeOS is not measured (4 KiB assumed).
    pub fn page_shift(&self) -> u8 {
        if self.cputype != CPU_TYPE_ARM64 {
            return 12;
        }
        let small = match (self.platform, self.minos) {
            (Some(platform::IOS), Some(v)) => v < 0x0010_0000,
            (Some(platform::IOS), None) => true,
            (Some(platform::TVOS | platform::BRIDGEOS), _) => true,
            _ => false,
        };
        if small { 12 } else { 14 }
    }

    /// The CodeDirectory digest types `codesign` picks by default, primary first.
    ///
    /// SHA-1 is added (as the primary CodeDirectory) when the deployment target predates
    /// SHA-256-only signatures: macOS before 10.11.4, iOS/tvOS before 11, watchOS before 4
    /// (macOS and iOS boundaries measured), or when no minimum version is recorded.
    pub fn digest_types(&self) -> Vec<HashType> {
        let old = match (self.platform, self.minos) {
            (Some(platform::MACOS), Some(v)) => v < 0x000a_0b04,
            (
                Some(
                    platform::IOS
                    | platform::TVOS
                    | platform::IOSSIMULATOR
                    | platform::TVOSSIMULATOR,
                ),
                Some(v),
            ) => v < 0x000b_0000,
            (Some(platform::WATCHOS | platform::WATCHOSSIMULATOR), Some(v)) => v < 0x0004_0000,
            (Some(_), Some(_)) => false,
            _ => true,
        };
        if old {
            vec![HashType::Sha1, HashType::Sha256]
        } else {
            vec![HashType::Sha256]
        }
    }

    /// `codesign`'s unique suffix for ad-hoc identifiers: `"UUID"` followed by the `LC_UUID`
    /// bytes, or without `LC_UUID` the SHA-1 of the 28-byte header prefix and the load commands.
    pub fn identification(&self) -> Vec<u8> {
        match self.uuid {
            Some(u) => {
                let mut v = b"UUID".to_vec();
                v.extend_from_slice(&u);
                v
            }
            None => self.commands_sha1.to_vec(),
        }
    }

    /// Offset where the signature goes: the old `dataoff`, or the end of `__LINKEDIT` aligned to 16.
    pub fn signature_offset(&self) -> u64 {
        match self.code_signature {
            Some((_, dataoff, _)) => dataoff as u64,
            None => (self.linkedit.fileoff + self.linkedit.filesize).next_multiple_of(16),
        }
    }

    /// Builds the file image with room for a signature of `size` bytes at
    /// [`signature_offset`](Self::signature_offset), the way `codesign_allocate` lays it out:
    /// the old bytes are kept up to the new end (an old, longer signature leaves its tail
    /// inside the new reserved area, as with `codesign`), the file is cut or zero-extended to
    /// `offset + size`, `LC_CODE_SIGNATURE` is updated or appended, and `__LINKEDIT` grows to
    /// the new end with `vmsize` rounded up to 16 KiB.
    pub fn layout_for_signature(&self, data: &[u8], size: u32) -> Result<Vec<u8>> {
        let offset = self.signature_offset();
        let end = offset + size as u64;
        let end_us = usize::try_from(end).map_err(|_| format_err("file too large"))?;
        if offset > u32::MAX as u64 || end > u32::MAX as u64 {
            return Err(Error::Unsupported("signature offset beyond 4 GiB".into()));
        }
        if self.linkedit.fileoff > offset {
            return Err(format_err("__LINKEDIT starts after the signature offset"));
        }
        let mut out = data[..data.len().min(end_us)].to_vec();
        out.resize(end_us, 0);
        let cs_cmd = match self.code_signature {
            Some((cmd_off, _, _)) => cmd_off,
            None => {
                let at = 32 + self.sizeofcmds as usize;
                let limit = self.first_section_offset.unwrap_or(self.linkedit.fileoff);
                if (at + 16) as u64 > limit {
                    return Err(Error::Unsupported(
                        "no header room for LC_CODE_SIGNATURE".into(),
                    ));
                }
                if out[at..at + 16].iter().any(|b| *b != 0) {
                    return Err(format_err("header padding is not empty"));
                }
                out[at..at + 4].copy_from_slice(&LC_CODE_SIGNATURE.to_le_bytes());
                out[at + 4..at + 8].copy_from_slice(&16u32.to_le_bytes());
                out[16..20].copy_from_slice(&(self.ncmds + 1).to_le_bytes());
                out[20..24].copy_from_slice(&(self.sizeofcmds + 16).to_le_bytes());
                at
            }
        };
        out[cs_cmd + 8..cs_cmd + 12].copy_from_slice(&(offset as u32).to_le_bytes());
        out[cs_cmd + 12..cs_cmd + 16].copy_from_slice(&size.to_le_bytes());
        let le = self.linkedit;
        let filesize = end - le.fileoff;
        let vmsize = filesize.next_multiple_of(0x4000);
        out[le.cmd_offset + 32..le.cmd_offset + 40].copy_from_slice(&vmsize.to_le_bytes());
        out[le.cmd_offset + 48..le.cmd_offset + 56].copy_from_slice(&filesize.to_le_bytes());
        Ok(out)
    }
}

#[cfg(test)]
pub(crate) mod tests_support {
    use super::*;

    /// A tiny synthetic arm64 executable: header, __TEXT with one section, __LINKEDIT, LC_UUID.
    pub(crate) fn synthetic(with_sig: bool) -> Vec<u8> {
        let mut cmds = Vec::new();
        let seg = |name: &[u8], fileoff: u64, filesize: u64, nsects: u32| {
            let mut c = Vec::new();
            c.extend_from_slice(&LC_SEGMENT_64.to_le_bytes());
            c.extend_from_slice(&(72 + 80 * nsects).to_le_bytes());
            let mut n = [0u8; 16];
            n[..name.len()].copy_from_slice(name);
            c.extend_from_slice(&n);
            c.extend_from_slice(&0u64.to_le_bytes()); // vmaddr
            c.extend_from_slice(&filesize.next_multiple_of(0x4000).to_le_bytes());
            c.extend_from_slice(&fileoff.to_le_bytes());
            c.extend_from_slice(&filesize.to_le_bytes());
            c.extend_from_slice(&[5, 0, 0, 0, 5, 0, 0, 0]);
            c.extend_from_slice(&nsects.to_le_bytes());
            c.extend_from_slice(&0u32.to_le_bytes());
            c
        };
        let mut text = seg(b"__TEXT", 0, 0x4000, 1);
        let mut sect = [0u8; 80];
        sect[..6].copy_from_slice(b"__text");
        sect[16..22].copy_from_slice(b"__TEXT");
        sect[48..52].copy_from_slice(&0x1000u32.to_le_bytes());
        text.extend_from_slice(&sect);
        cmds.extend_from_slice(&text);
        cmds.extend_from_slice(&seg(b"__LINKEDIT", 0x4000, 0x100, 0));
        cmds.extend_from_slice(&LC_UUID.to_le_bytes());
        cmds.extend_from_slice(&24u32.to_le_bytes());
        cmds.extend_from_slice(&[7u8; 16]);
        let mut ncmds = 3u32;
        if with_sig {
            cmds.extend_from_slice(&LC_CODE_SIGNATURE.to_le_bytes());
            cmds.extend_from_slice(&16u32.to_le_bytes());
            cmds.extend_from_slice(&0x4100u32.to_le_bytes());
            cmds.extend_from_slice(&0x40u32.to_le_bytes());
            ncmds += 1;
        }
        let mut f = Vec::new();
        f.extend_from_slice(&MH_MAGIC_64.to_le_bytes());
        f.extend_from_slice(&CPU_TYPE_ARM64.to_le_bytes());
        f.extend_from_slice(&0u32.to_le_bytes());
        f.extend_from_slice(&MH_EXECUTE.to_le_bytes());
        f.extend_from_slice(&ncmds.to_le_bytes());
        f.extend_from_slice(&(cmds.len() as u32).to_le_bytes());
        f.extend_from_slice(&0u32.to_le_bytes());
        f.extend_from_slice(&0u32.to_le_bytes());
        f.extend_from_slice(&cmds);
        f.resize(0x4100 + if with_sig { 0x40 } else { 0 }, 0xab);
        for b in &mut f[32 + cmds.len()..0x1000] {
            *b = 0;
        }
        f
    }

    #[test]
    fn parse_and_layout() {
        let f = synthetic(true);
        let m = MachO::parse(&f).unwrap();
        assert!(m.is_main_binary());
        assert_eq!(m.text.unwrap().filesize, 0x4000);
        assert_eq!(m.code_signature.unwrap().1, 0x4100);
        assert_eq!(m.page_shift(), 14);
        assert_eq!(m.digest_types(), vec![HashType::Sha1, HashType::Sha256]); // no minos recorded
        assert_eq!(m.identification().len(), 20);
        let out = m.layout_for_signature(&f, 0x200).unwrap();
        assert_eq!(out.len(), 0x4300);
        let m2 = MachO::parse(&out).unwrap();
        assert_eq!(
            (m2.code_signature.unwrap().1, m2.code_signature.unwrap().2),
            (0x4100, 0x200)
        );
        assert_eq!(m2.linkedit.filesize, 0x300);
        // Old signature bytes stay where the new area overlaps them.
        assert_eq!(out[0x4100], 0xab);
        assert_eq!(out[0x4140], 0);
    }

    #[test]
    fn appends_code_signature_command() {
        let f = synthetic(false);
        let m = MachO::parse(&f).unwrap();
        assert!(m.code_signature.is_none());
        let out = m.layout_for_signature(&f, 0x40).unwrap();
        let m2 = MachO::parse(&out).unwrap();
        assert_eq!(m2.ncmds, m.ncmds + 1);
        assert_eq!(m2.code_signature.unwrap().1, 0x4100);
    }

    #[test]
    fn rejects_fat_and_32bit() {
        assert!(matches!(
            MachO::parse(&[0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 0].repeat(4)),
            Err(Error::Unsupported(_))
        ));
        let mut h = MH_MAGIC.to_le_bytes().to_vec();
        h.resize(32, 0);
        assert!(matches!(MachO::parse(&h), Err(Error::Unsupported(_))));
        assert!(is_macho(&synthetic(false)));
        assert!(!is_macho(b"hello"));
    }
}
