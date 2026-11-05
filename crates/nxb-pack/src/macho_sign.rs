//! Signing one thin Mach-O image in memory, as `codesign --force --sign -` does.

use crate::blob::{self, CodeDirectorySpec, special};
use crate::error::{Error, Result, format_err};
use crate::fat;
use crate::hash::{HashType, hex};
use crate::macho::MachO;

/// How the signing identifier is chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Identifier {
    /// Used verbatim (`codesign -i`).
    Explicit(String),
    /// Derived from a bundle identifier or a file name. When it contains no `.`, `codesign`
    /// appends `-` and the hex of the image's identification (`"UUID"` + `LC_UUID`).
    Derived(String),
    /// Derived from the file name: the base name up to its last `.`, then as [`Identifier::Derived`].
    FileName(String),
}

/// Inputs for [`sign_macho`] besides the image.
#[derive(Clone, Debug)]
pub struct SignParams {
    /// The signing identifier.
    pub identifier: Identifier,
    /// Info.plist bytes bound in special slot 1 (the main executable of a bundle).
    pub info_plist: Option<Vec<u8>>,
    /// `CodeResources` bytes bound in special slot 3 (the main executable of a bundle).
    pub code_resources: Option<Vec<u8>>,
    /// Entitlements XML, embedded verbatim (slot 5) and as DER (slot 7).
    pub entitlements: Option<Vec<u8>>,
    /// Hardened runtime (`--options runtime`): flag `0x10000`, CodeDirectory `0x20500` with
    /// the SDK version in its `runtime` field.
    pub runtime: bool,
}

impl SignParams {
    /// Parameters for a standalone file (`codesign -f -s - path`): identifier from the file name.
    pub fn standalone(file_name: &str) -> Self {
        SignParams {
            identifier: Identifier::FileName(file_name.to_string()),
            info_plist: None,
            code_resources: None,
            entitlements: None,
            runtime: false,
        }
    }
}

/// `(hash type, cdhash)` per CodeDirectory, in superblob order (primary first).
pub type CdHashes = Vec<(HashType, [u8; 20])>;

/// A signed image and its CodeDirectory hashes.
#[derive(Clone, Debug)]
pub struct SignedMachO {
    /// The complete signed file.
    pub data: Vec<u8>,
    /// `(hash type, cdhash)` per CodeDirectory, in superblob order (primary first).
    pub cdhashes: CdHashes,
    /// The identifier written.
    pub identifier: String,
}

fn resolve_identifier(id: &Identifier, m: &MachO) -> Result<String> {
    let derived = match id {
        Identifier::Explicit(s) => return Ok(s.clone()),
        Identifier::Derived(s) => s.clone(),
        Identifier::FileName(name) => {
            let base = name.rsplit('/').next().unwrap_or(name);
            match base.rfind('.') {
                Some(i) => base[..i].to_string(),
                None => base.to_string(),
            }
        }
    };
    if derived.contains('.') {
        return Ok(derived);
    }
    Ok(format!("{derived}-{}", hex(&m.identification())))
}

/// Size `codesign` reserves for a superblob of `len` bytes, `short_code_directories` of which
/// have the 88-byte (version 0x20400) header: codesign sizes every CodeDirectory with the
/// 96-byte header of version 0x20500, so each short one leaves 8 bytes of slack, the total is
/// rounded up to 16. Fitted to 72 signatures written by codesign (with and without
/// entitlements, hardened runtime, one and two CodeDirectories).
pub fn reserved_size(len: usize, short_code_directories: usize) -> usize {
    (len + 8 * short_code_directories).next_multiple_of(16)
}

/// The slice `codesign` treats as the main architecture of a universal file on an Apple
/// silicon Mac: arm64 when present, else the first slice. Identification (the ad-hoc identifier
/// suffix) and the cdhash used for nested code come from it.
pub fn main_slice(slices: &[fat::Slice]) -> usize {
    slices
        .iter()
        .position(|s| s.cputype == CPU_TYPE_ARM64)
        .unwrap_or(0)
}

const CPU_TYPE_ARM64: u32 = 0x0100_000c;

/// Signs `input` (thin or universal) ad hoc. The result replaces any existing signature.
/// For a universal file every slice is signed with the same identifier and `cdhashes` are the
/// main slice's ([`main_slice`]).
pub fn sign_macho(input: &[u8], params: &SignParams) -> Result<SignedMachO> {
    if !fat::is_fat(input) {
        let m = MachO::parse(input)?;
        let identifier = resolve_identifier(&params.identifier, &m)?;
        return sign_thin(input, &m, params, identifier);
    }
    let mut slices = fat::split(input)?;
    let main = main_slice(&slices);
    let identifier = resolve_identifier(&params.identifier, &MachO::parse(&slices[main].data)?)?;
    let mut cdhashes = Vec::new();
    for (i, s) in slices.iter_mut().enumerate() {
        let m = MachO::parse(&s.data)?;
        let signed = sign_thin(&s.data, &m, params, identifier.clone())?;
        if i == main {
            cdhashes = signed.cdhashes;
        }
        s.data = signed.data;
    }
    Ok(SignedMachO {
        data: fat::assemble(&slices)?,
        cdhashes,
        identifier,
    })
}

/// The digest types and the Mach-O of the main architecture of a thin or universal file.
pub fn main_image(data: &[u8]) -> Result<MachO> {
    if fat::is_fat(data) {
        let slices = fat::split(data)?;
        MachO::parse(&slices[main_slice(&slices)].data)
    } else {
        MachO::parse(data)
    }
}

fn sign_thin(
    input: &[u8],
    m: &MachO,
    params: &SignParams,
    identifier: String,
) -> Result<SignedMachO> {
    if m.has_embedded_info_plist && params.info_plist.is_none() {
        return Err(Error::Unsupported("embedded __info_plist section".into()));
    }
    let runtime =
        if params.runtime {
            Some(m.sdk.ok_or_else(|| {
                Error::Unsupported("hardened runtime without an SDK version".into())
            })?)
        } else {
            None
        };
    let requirements = blob::empty_requirements();
    let ent_blobs = match &params.entitlements {
        Some(xml) => {
            let v = crate::plist::parse(xml)?;
            let der = crate::der::encode_entitlements(&v)?;
            Some((
                blob::blob(blob::MAGIC_ENTITLEMENTS, xml),
                blob::blob(blob::MAGIC_ENTITLEMENTS_DER, &der),
            ))
        }
        None => None,
    };

    let n_special = if ent_blobs.is_some() {
        special::ENTITLEMENTS_DER
    } else if params.code_resources.is_some() {
        special::RESOURCE_DIR
    } else {
        special::REQUIREMENTS
    };
    let mut slots: Vec<Option<Vec<u8>>> = vec![None; n_special];
    slots[special::INFO - 1] = params.info_plist.clone();
    slots[special::REQUIREMENTS - 1] = Some(requirements.clone());
    if let Some(cr) = &params.code_resources {
        slots[special::RESOURCE_DIR - 1] = Some(cr.clone());
    }
    if let Some((xml, der)) = &ent_blobs {
        slots[special::ENTITLEMENTS - 1] = Some(xml.clone());
        slots[special::ENTITLEMENTS_DER - 1] = Some(der.clone());
    }

    let offset = m.signature_offset();
    let code_limit =
        u32::try_from(offset).map_err(|_| Error::Unsupported("code limit beyond 4 GiB".into()))?;
    let (exec_seg_base, exec_seg_limit) = m.text.map_or((0, 0), |t| (t.fileoff, t.filesize));
    let specs: Vec<CodeDirectorySpec> = m
        .digest_types()
        .into_iter()
        .map(|hash_type| CodeDirectorySpec {
            hash_type,
            identifier: identifier.clone(),
            flags: blob::CS_ADHOC
                | if runtime.is_some() {
                    blob::CS_RUNTIME
                } else {
                    0
                },
            runtime,
            code_limit,
            page_shift: m.page_shift(),
            exec_seg_base,
            exec_seg_limit,
            exec_seg_flags: if m.is_main_binary() {
                blob::CS_EXECSEG_MAIN_BINARY
            } else {
                0
            },
            special: slots.clone(),
        })
        .collect();

    let cms = blob::blob(blob::MAGIC_BLOBWRAPPER, &[]);
    let mut other_len = requirements.len() + cms.len();
    let mut count = 2 + specs.len();
    if let Some((x, d)) = &ent_blobs {
        other_len += x.len() + d.len();
        count += 2;
    }
    let sb_len = 12 + 8 * count + other_len + specs.iter().map(|s| s.len()).sum::<usize>();
    let size = u32::try_from(reserved_size(
        sb_len,
        if runtime.is_some() { 0 } else { specs.len() },
    ))
    .map_err(|_| format_err("signature too large"))?;

    let mut data = m.layout_for_signature(input, size)?;
    let cds: Vec<Vec<u8>> = specs.iter().map(|s| s.build(&data)).collect();
    let cdhashes: Vec<(HashType, [u8; 20])> = specs
        .iter()
        .zip(&cds)
        .map(|(s, cd)| (s.hash_type, blob::cdhash(cd, s.hash_type)))
        .collect();

    let mut blobs = vec![
        (blob::SLOT_CODEDIRECTORY, cds[0].clone()),
        (blob::SLOT_REQUIREMENTS, requirements),
    ];
    if let Some((x, d)) = ent_blobs {
        blobs.push((blob::SLOT_ENTITLEMENTS, x));
        blobs.push((blob::SLOT_ENTITLEMENTS_DER, d));
    }
    for (i, cd) in cds.iter().enumerate().skip(1) {
        blobs.push((
            blob::SLOT_ALTERNATE_CODEDIRECTORIES + i as u32 - 1,
            cd.clone(),
        ));
    }
    blobs.push((blob::SLOT_SIGNATURE, cms));
    let sb = blob::superblob(&blobs);
    debug_assert_eq!(sb.len(), sb_len);
    let at = offset as usize;
    data[at..at + sb.len()].copy_from_slice(&sb);
    Ok(SignedMachO {
        data,
        cdhashes,
        identifier,
    })
}

/// Reads the CodeDirectory hashes of an already signed image, in superblob order.
pub fn cdhashes_of(data: &[u8]) -> Result<CdHashes> {
    if fat::is_fat(data) {
        let slices = fat::split(data)?;
        return cdhashes_of(&slices[main_slice(&slices)].data);
    }
    let m = MachO::parse(data)?;
    let (_, off, size) = m
        .code_signature
        .ok_or_else(|| Error::Refused("code object is not signed at all".into()))?;
    let sig = data
        .get(off as usize..off as usize + size as usize)
        .ok_or_else(|| format_err("signature past the end of the file"))?;
    let blobs = blob::parse_superblob(sig).ok_or_else(|| format_err("bad signature superblob"))?;
    let mut out = Vec::new();
    for b in blobs
        .iter()
        .filter(|b| b.magic == blob::MAGIC_CODEDIRECTORY)
    {
        let t = blob::cd_hash_type(b.data)
            .ok_or_else(|| Error::Unsupported("CodeDirectory hash type".into()))?;
        out.push((t, blob::cdhash(b.data, t)));
    }
    if out.is_empty() {
        return Err(format_err("signature without CodeDirectory"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signs_synthetic_image_twice_identically() {
        let f = crate::macho::tests_support::synthetic(true);
        let p = SignParams::standalone("tool");
        let a = sign_macho(&f, &p).unwrap();
        assert!(a.identifier.starts_with("tool-55554944"));
        // Re-signing a signed file gives the same bytes when the reserved size is unchanged.
        let b = sign_macho(&a.data, &p).unwrap();
        assert_eq!(a.data, b.data);
        assert_eq!(cdhashes_of(&a.data).unwrap(), a.cdhashes);
        assert_eq!(a.cdhashes.len(), 2); // no minos: SHA-1 + SHA-256
    }

    #[test]
    fn identifiers() {
        let f = crate::macho::tests_support::synthetic(true);
        let m = MachO::parse(&f).unwrap();
        assert_eq!(
            resolve_identifier(&Identifier::FileName("libx.1.dylib".into()), &m).unwrap(),
            "libx.1"
        );
        assert_eq!(
            resolve_identifier(&Identifier::Derived("org.a.b".into()), &m).unwrap(),
            "org.a.b"
        );
        assert_eq!(
            resolve_identifier(&Identifier::Explicit("x".into()), &m).unwrap(),
            "x"
        );
        assert_eq!(
            resolve_identifier(&Identifier::FileName("dir/t".into()), &m).unwrap(),
            format!("t-55554944{}", "07".repeat(16))
        );
    }

    #[test]
    fn reserved() {
        // (superblob length, CodeDirectories, codesign's datasize)
        for (len, cds, want) in [
            (316, 1, 336),
            (1006, 1, 1024),
            (10016, 1, 10032),
            (1180, 1, 1200),
            (951, 1, 960),
            (1633, 1, 1648),
            (1192, 1, 1200),
            (704, 1, 720),
            (536, 2, 560),
            (1232, 2, 1248),
            (1025, 2, 1056),
            // hardened runtime: no short CodeDirectory
            (332, 0, 336),
            (922, 0, 928),
            (1472, 0, 1472),
            (1041, 0, 1056),
        ] {
            assert_eq!(reserved_size(len, cds), want, "{len}");
        }
    }
}
