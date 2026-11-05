# Third-party notes

This repository has no external Rust dependencies. `Cargo.lock` lists only the crates of the workspace.
On macOS, `nxb-sys` links the system zlib (`/usr/lib/libz`) dynamically. No third-party source code is in the tree.

## Formats and algorithms written from public documentation

The code was written from public specifications, public documentation, or observation of the output of tools. No code was
taken from other implementations.

| area | where | basis |
|---|---|---|
| Mach-O code signature (SuperBlob, CodeDirectory, requirements, entitlements, `CodeResources`) | `nxb-pack` | output of Apple `codesign` on test bundles, and public headers |
| metallib container and AIR conventions | `air-bc`, `air-emit`, `air-lib` | observation of the output of Apple tools. No Apple code is copied |
| LLVM 14 bitcode | `air-bc` | the public LLVM bitcode documentation |
| NCA, NSP (PFS0), IVFC, BKTR, RomFS, NSO, NACP, SARC, BNSH, BAHS, Yaz0 | `nxb-extract`, `nxb-split`, `nxb-romfs`, `nxb-hos`, `nxb-rewrite` | public format documentation of the homebrew community. The `prod.keys` text format is the common `name = hex` layout |
| zstd decoder | `nxb-split`, `nxb-codec` | RFC 8878 |
| LZ4 block decoder | `nxb-codec`, `nxb-split` | the public LZ4 block format description |
| AES-128 (ECB, CTR, XTS with the console tweak) | `nxb-extract`, `nxb-sys` | FIPS 197, NIST SP 800-38A, IEEE 1619. The test vectors are the published ones |
| SHA-1, SHA-256 | `nxb-model`, `nxb-sys` | FIPS 180-4. The test vectors are the published ones |
| AArch64 decoding and patching | `nxb-a64`, `nxb-rewrite` | the Arm Architecture Reference Manual |
| Property lists (XML and binary) | `nxb-plist`, `nxb-pack` | public Apple documentation and observed output |

## Tools that nxbuild starts and does not contain

| tool | licence | use |
|---|---|---|
| Shader translator (a build of the Ryujinx translator) | MIT | started as an external process. No code is in this tree |
| spirv-cross | Apache-2.0 | started as an external program. No code is in this tree |
| Apple tools (`metal`, `metallib`, `air-opt`, `air-as`, `codesign`, `metal-tt`, `metal-pack`, `xcrun`) | Apple licence | started from your own Xcode. Not redistributed |

## If code is ported later

If code from the translator or from spirv-cross is translated into this repository, the change must add the original copyright
notice and the licence text to this file. The change must also list which files come from which source.
