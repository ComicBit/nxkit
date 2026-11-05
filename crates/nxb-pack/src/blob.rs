//! Code signing blobs: CodeDirectory, requirements, entitlements, the empty CMS wrapper and the
//! embedded-signature superblob. All fields are big-endian.
//!
//! CodeDirectory layout written here (version `0x20400`, 88-byte header):
//!
//! | offset | field | value written |
//! |---|---|---|
//! | 0 | magic | `0xfade0c02` |
//! | 4 | length | whole blob |
//! | 8 | version | `0x20400` |
//! | 12 | flags | `0x2` (adhoc) |
//! | 16 | hashOffset | offset of code slot 0 |
//! | 20 | identOffset | 88 |
//! | 24 | nSpecialSlots | highest special slot used |
//! | 28 | nCodeSlots | pages up to codeLimit |
//! | 32 | codeLimit | signature offset |
//! | 36 | hashSize, hashType, platform, pageSize | 20/32, 1/2, 0, log2 |
//! | 40 | spare2 | 0 |
//! | 44 | scatterOffset, teamOffset, spare3 | 0 |
//! | 56 | codeLimit64 | 0 |
//! | 64 | execSegBase, execSegLimit, execSegFlags | `__TEXT` fileoff, filesize, 1 for `MH_EXECUTE` |
//!
//! Then the identifier (NUL-terminated), the special slots in reverse order (slot n first),
//! and the code slots.

use crate::hash::{HashType, sha1, sha256};

/// Superblob magic of an embedded signature.
pub const MAGIC_EMBEDDED_SIGNATURE: u32 = 0xfade_0cc0;
/// CodeDirectory magic.
pub const MAGIC_CODEDIRECTORY: u32 = 0xfade_0c02;
/// Requirement set magic.
pub const MAGIC_REQUIREMENTS: u32 = 0xfade_0c01;
/// XML entitlements magic.
pub const MAGIC_ENTITLEMENTS: u32 = 0xfade_7171;
/// DER entitlements magic.
pub const MAGIC_ENTITLEMENTS_DER: u32 = 0xfade_7172;
/// CMS signature wrapper magic (empty for ad-hoc).
pub const MAGIC_BLOBWRAPPER: u32 = 0xfade_0b01;

/// Superblob slot of the primary CodeDirectory.
pub const SLOT_CODEDIRECTORY: u32 = 0;
/// Superblob slot of the requirements.
pub const SLOT_REQUIREMENTS: u32 = 2;
/// Superblob slot of the XML entitlements.
pub const SLOT_ENTITLEMENTS: u32 = 5;
/// Superblob slot of the DER entitlements.
pub const SLOT_ENTITLEMENTS_DER: u32 = 7;
/// Superblob slot of the first alternate CodeDirectory.
pub const SLOT_ALTERNATE_CODEDIRECTORIES: u32 = 0x1000;
/// Superblob slot of the CMS signature.
pub const SLOT_SIGNATURE: u32 = 0x10000;

/// Special slot numbers inside a CodeDirectory.
pub mod special {
    /// Hash of Info.plist.
    pub const INFO: usize = 1;
    /// Hash of the requirements blob.
    pub const REQUIREMENTS: usize = 2;
    /// Hash of `_CodeSignature/CodeResources`.
    pub const RESOURCE_DIR: usize = 3;
    /// Hash of the XML entitlements blob.
    pub const ENTITLEMENTS: usize = 5;
    /// Hash of the DER entitlements blob.
    pub const ENTITLEMENTS_DER: usize = 7;
}

/// CodeDirectory flag `CS_ADHOC`.
pub const CS_ADHOC: u32 = 0x2;
/// `execSegFlags` bit `CS_EXECSEG_MAIN_BINARY`.
pub const CS_EXECSEG_MAIN_BINARY: u64 = 0x1;
/// CodeDirectory flag `CS_RUNTIME` (hardened runtime).
pub const CS_RUNTIME: u32 = 0x10000;
/// Version written.
pub const CD_VERSION: u32 = 0x20400;
/// Version written with a hardened runtime (adds `runtime` and `preEncryptOffset`).
pub const CD_VERSION_RUNTIME: u32 = 0x20500;
const CD_HEADER: usize = 88;
const CD_HEADER_RUNTIME: usize = 96;

/// A blob: magic, length, payload.
pub fn blob(magic: u32, payload: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(8 + payload.len());
    b.extend_from_slice(&magic.to_be_bytes());
    b.extend_from_slice(&((8 + payload.len()) as u32).to_be_bytes());
    b.extend_from_slice(payload);
    b
}

/// The empty requirement set (`count` 0, 12 bytes) that ad-hoc signatures carry.
pub fn empty_requirements() -> Vec<u8> {
    blob(MAGIC_REQUIREMENTS, &0u32.to_be_bytes())
}

/// Everything a CodeDirectory is made of except the code pages.
#[derive(Clone, Debug)]
pub struct CodeDirectorySpec {
    /// Digest algorithm.
    pub hash_type: HashType,
    /// Signing identifier.
    pub identifier: String,
    /// CodeDirectory flags.
    pub flags: u32,
    /// Bytes covered by code slots (the signature offset).
    pub code_limit: u32,
    /// log2 of the page size.
    pub page_shift: u8,
    /// `execSegBase`.
    pub exec_seg_base: u64,
    /// `execSegLimit`.
    pub exec_seg_limit: u64,
    /// `execSegFlags`.
    pub exec_seg_flags: u64,
    /// Hardened runtime version (the SDK version).`Some` writes version `0x20500`.
    pub runtime: Option<u32>,
    /// Special slot contents, index 0 = slot 1.`None` = an empty (zero) slot.
    /// Hashed with `hash_type` when the CodeDirectory is built.
    pub special: Vec<Option<Vec<u8>>>,
}

impl CodeDirectorySpec {
    /// Number of code slots.
    pub fn code_slots(&self) -> usize {
        (self.code_limit as usize).div_ceil(1 << self.page_shift)
    }

    fn header_len(&self) -> usize {
        if self.runtime.is_some() {
            CD_HEADER_RUNTIME
        } else {
            CD_HEADER
        }
    }

    /// Length of the CodeDirectory blob.
    pub fn len(&self) -> usize {
        self.header_len()
            + self.identifier.len()
            + 1
            + (self.special.len() + self.code_slots()) * self.hash_type.digest_len()
    }

    /// Whether the directory would be empty (never: the header alone has 88 bytes).
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Builds the CodeDirectory over `code` (which must have at least `code_limit` bytes).
    pub fn build(&self, code: &[u8]) -> Vec<u8> {
        let hs = self.hash_type.digest_len();
        let n_special = self.special.len();
        let n_code = self.code_slots();
        let header = self.header_len();
        let hash_offset = header + self.identifier.len() + 1 + n_special * hs;
        let len = self.len();
        let mut b = Vec::with_capacity(len);
        let p32 = |b: &mut Vec<u8>, v: u32| b.extend_from_slice(&v.to_be_bytes());
        p32(&mut b, MAGIC_CODEDIRECTORY);
        p32(&mut b, len as u32);
        p32(
            &mut b,
            if self.runtime.is_some() {
                CD_VERSION_RUNTIME
            } else {
                CD_VERSION
            },
        );
        p32(&mut b, self.flags);
        p32(&mut b, hash_offset as u32);
        p32(&mut b, header as u32);
        p32(&mut b, n_special as u32);
        p32(&mut b, n_code as u32);
        p32(&mut b, self.code_limit);
        b.extend_from_slice(&[hs as u8, self.hash_type.code(), 0, self.page_shift]);
        p32(&mut b, 0); // spare2
        p32(&mut b, 0); // scatterOffset
        p32(&mut b, 0); // teamOffset
        p32(&mut b, 0); // spare3
        b.extend_from_slice(&0u64.to_be_bytes()); // codeLimit64
        b.extend_from_slice(&self.exec_seg_base.to_be_bytes());
        b.extend_from_slice(&self.exec_seg_limit.to_be_bytes());
        b.extend_from_slice(&self.exec_seg_flags.to_be_bytes());
        if let Some(rt) = self.runtime {
            p32(&mut b, rt);
            p32(&mut b, 0); // preEncryptOffset
        }
        debug_assert_eq!(b.len(), header);
        b.extend_from_slice(self.identifier.as_bytes());
        b.push(0);
        for slot in self.special.iter().rev() {
            match slot {
                Some(data) => b.extend_from_slice(&self.hash_type.digest(data)),
                None => b.extend(std::iter::repeat_n(0u8, hs)),
            }
        }
        let page = 1usize << self.page_shift;
        let code = &code[..self.code_limit as usize];
        for chunk in code.chunks(page) {
            match self.hash_type {
                HashType::Sha1 => b.extend_from_slice(&sha1(chunk)),
                HashType::Sha256 => b.extend_from_slice(&sha256(chunk)),
            }
        }
        debug_assert_eq!(b.len(), len);
        b
    }
}

/// The cdhash of a CodeDirectory: its digest with its own hash type, cut to 20 bytes.
pub fn cdhash(cd: &[u8], hash_type: HashType) -> [u8; 20] {
    let d = hash_type.digest(cd);
    let mut o = [0u8; 20];
    o.copy_from_slice(&d[..20.min(d.len())]);
    o
}

/// Assembles a superblob from `(slot, blob)` pairs, in the order given.
pub fn superblob(blobs: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let header = 12 + 8 * blobs.len();
    let total = header + blobs.iter().map(|(_, b)| b.len()).sum::<usize>();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&MAGIC_EMBEDDED_SIGNATURE.to_be_bytes());
    out.extend_from_slice(&(total as u32).to_be_bytes());
    out.extend_from_slice(&(blobs.len() as u32).to_be_bytes());
    let mut off = header;
    for (slot, b) in blobs {
        out.extend_from_slice(&slot.to_be_bytes());
        out.extend_from_slice(&(off as u32).to_be_bytes());
        off += b.len();
    }
    for (_, b) in blobs {
        out.extend_from_slice(b);
    }
    out
}

/// One blob of a parsed superblob.
#[derive(Clone, Debug)]
pub struct ParsedBlob<'a> {
    /// Superblob slot.
    pub slot: u32,
    /// Blob magic.
    pub magic: u32,
    /// The whole blob (magic and length included).
    pub data: &'a [u8],
}

/// Splits an embedded signature into its blobs.
pub fn parse_superblob(sig: &[u8]) -> Option<Vec<ParsedBlob<'_>>> {
    let be = |o: usize| {
        sig.get(o..o + 4)
            .map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    };
    if be(0)? != MAGIC_EMBEDDED_SIGNATURE {
        return None;
    }
    let count = be(8)? as usize;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let slot = be(12 + 8 * i)?;
        let off = be(16 + 8 * i)? as usize;
        let magic = be(off)?;
        let len = be(off + 4)? as usize;
        out.push(ParsedBlob {
            slot,
            magic,
            data: sig.get(off..off + len)?,
        });
    }
    Some(out)
}

/// The hash type of a CodeDirectory blob.
pub fn cd_hash_type(cd: &[u8]) -> Option<HashType> {
    match cd.get(37)? {
        1 => Some(HashType::Sha1),
        2 => Some(HashType::Sha256),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::hex;

    #[test]
    fn empty_requirements_hash_matches_codesign() {
        let r = empty_requirements();
        assert_eq!(hex(&r), "fade0c010000000c00000000");
        // Special slot 2 as seen in every ad-hoc signature written by codesign.
        assert_eq!(
            hex(&sha256(&r)),
            "987920904eab650e75788c054aa0b0524e6a80bfc71aa32df8d237a61743f986"
        );
        assert_eq!(hex(&sha1(&r)), "3a75f6db058529148e14dd7ea1b4729cc09ec973");
    }

    #[test]
    fn code_directory_shape() {
        let spec = CodeDirectorySpec {
            hash_type: HashType::Sha256,
            identifier: "a".into(),
            flags: CS_ADHOC,
            code_limit: 16384 + 1,
            page_shift: 14,
            exec_seg_base: 0,
            exec_seg_limit: 16384,
            exec_seg_flags: 1,
            runtime: None,
            special: vec![None, Some(empty_requirements())],
        };
        let code = vec![0u8; 20000];
        let cd = spec.build(&code);
        assert_eq!(cd.len(), spec.len());
        assert_eq!(cd.len(), 88 + 2 + 4 * 32);
        assert_eq!(
            u32::from_be_bytes(cd[16..20].try_into().unwrap()),
            88 + 2 + 64
        );
        assert_eq!(&cd[122..154], &[0u8; 32][..]); // slot 1 (empty)
        assert_eq!(&cd[90..122], &sha256(&empty_requirements())[..]);
        assert_eq!(&cd[154..186], &sha256(&code[..16384])[..]);
        assert_eq!(&cd[186..218], &sha256(&code[16384..16385])[..]);
    }

    #[test]
    fn superblob_round_trip() {
        let sb = superblob(&[
            (0, blob(MAGIC_CODEDIRECTORY, b"x")),
            (2, empty_requirements()),
        ]);
        let p = parse_superblob(&sb).unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[1].slot, 2);
        assert_eq!(p[1].data, &empty_requirements()[..]);
        assert_eq!(
            u32::from_be_bytes(sb[4..8].try_into().unwrap()) as usize,
            sb.len()
        );
    }
}
