# Security notes

This document lists what the tools trust, what limits the code applies, and what gaps remain.
The limits come from reading the code of version 0.1.0. Where a gap is open, this document says so.

## Trust model

| input | trusted? | note |
|---|---|---|
| Game packages (NSP, NCA, RomFS, NSO and the archives inside them) | no | Anyone can change these files. The tools must reject damaged files and must not crash |
| Key file (`prod.keys`) | yes | You supply it. The tools never print key values, and they zero the title keys after use |
| Command line arguments and environment | yes | They come from the person who runs the tool |
| Files of a previous build (work folder, store) | partly | The store checks sizes and checksums. A damaged entry moves to quarantine |
| Apple tools and the shader translator | yes | You install them. nxbuild starts them with an empty environment plus an allow-list |

The tools do not check the digital signatures of packages. A package with a valid structure but wrong content is accepted.
Use packages from your own dumps only.

## What the code does against damaged input

### Rules for all readers

- Only `nxb-sys` and one module of `nxb-extract` (hardware AES) can contain `unsafe` code. All other crates use `forbid(unsafe_code)`.
- Clippy denies `unwrap`, `expect` and `panic!` outside tests. A reader returns a typed error.
- Integer reads from a byte slice go through helpers that return zero or an error when the slice is too short.

### NSP (PFS0) and NCA

- The header size is computed in 64 bits and must be inside the file. It must be at most 64 MiB.
- A file name offset must be inside the string table. A file entry must end inside the package (`checked_add`).
- Every storage read checks the range with `checked_add`. A read past the end is an error.
- The IVFC hash levels are at most 256 MiB each. The BKTR tables are at most 256 MiB each. The RomFS tables are at most 512 MiB each.
- The hash tree (IVFC) and the section hashes are checked. A mismatch is a "damaged" error.
- The RomFS walk has a loop guard. It stops when the visit count is more than the number of table entries.

### Compression

- zstd: the decoder rejects unknown magic, truncated blocks, and dictionaries. See the gaps below for the output size.
- LZ4: every copy is checked against the input and against the declared output size. The result must have the exact size.
- Yaz0: the stream must end with the declared size. A short stream is an error.
- Inflate (deflate): the caller gives a limit, and the decoder stops when the output grows past it.
- NSO: the segment layout must fit in 256 times the file size.

### Packing and signing

- The Mach-O reader checks every load command size and every read against the file length.
- Staging paths are checked. A destination path must be relative and must not contain empty, `.` or `..` parts.
- The ZIP code is a writer. There is no ZIP reader in the tree.

### Child processes

- `nxb-tool` starts tools with an argument vector (no shell), an empty environment plus an allow-list, and their own process group.
- Each run has a timeout. At the timeout, the whole group gets `SIGKILL`.

## Known gaps

These gaps are open in version 0.1.0. Until they are closed, treat a package from an unknown source as dangerous.

| # | gap | effect | where |
|---|---|---|---|
| 1 | zstd has no limit on the output size. An RLE block of a few bytes can make 2 MiB of output | memory use without limit | `nxb-split/src/zstd` |
| 2 | Yaz0 reserves memory for the size in the header (up to 4 GiB) before it reads the data | large allocation | `nxb-split/src/yaz0.rs` |
| 3 | The file names of an ExeFS and the RomFS entry paths are joined to the output folder without a check for `..` | a file can be written outside the output folder | `nxb-extract/src/run.rs`, `nxb-extract/src/html.rs` |
| 4 | `Storage::read_vec` reserves memory for the requested length before it checks the range | large allocation | `nxb-extract/src/storage.rs` |
| 5 | The offset of a PFS0 entry was added to the header size without an overflow check. The robustness test found it. The public tree fixes it with `checked_add`, and the live source still needs the same change | a panic in a debug build, a wrong offset in a release build | `nxb-extract/src/nca.rs` |
| 6 | The package signatures are not checked | a changed package is accepted | `nxb-extract` |
| 7 | The deterministic robustness tests cover a small set of seeds. They are not a replacement for a coverage-guided fuzzer | other defects can exist | `crates/*/tests/robustness.rs` |
| 8 | The plist, Mach-O and BNSH/SARC readers have no dedicated robustness tests yet | other defects can exist | `nxb-pack`, `nxb-split` |

## Robustness tests

The tests `crates/nxb-extract/tests/robustness.rs` and `crates/nxb-split/tests/robustness.rs` take small synthetic inputs
and change them in many ways. The changes are single bytes, extreme 32-bit and 64-bit values, cut-off tails and random bytes
from a fixed seed. A reader can return an error. It must not panic. The tests use the standard library only, and `cargo test` runs them.
The tests do not use game data.
