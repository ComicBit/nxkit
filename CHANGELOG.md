# Changelog

All notable changes are in this file. The format follows Keep a Changelog. Versions follow Semantic Versioning.

## 0.1.0 (first public release)

### Added

- Foundation and runtime crates: ids, hashes, errors, system calls, events, process runner, store, scheduler, parallel I/O.
- `shaderbuild`: the shader step in Rust (`build`, `recompile`, `guards`). On the reference title, 74,421 of 74,422 output files
  are byte-identical to the reference build.
- `nxb-extract` and `nxb-split`: NSP extract and shader split. Compared on three titles.
- `nxb-pack`, `nxb-macapp`, `nxb-ipa`: Mac app stage, ad-hoc signer, IPA writer. The signer matches `codesign` on the tested bundles.
- `nxb-pso`, `nxb-archive`: plan and merge of the pipeline archive.
- `nxb`: the orchestrator that runs the ported stages in one process graph.
- Own front end in layer F: `msl-lex`, `msl-parse`, `msl-sema`, `air-emit`, `air-bc`, `air-lib`, `air-front`, `airc`, `mslcheck`.
  It is on only for shaders on an allow list. A switch turns it off.
- Robustness tests for the NSP and NCA parsers and the zstd decoder.
- Release workflow that builds, signs (ad-hoc) and publishes the binaries for macOS on Apple silicon.

### Limits

- Mac only.
- Not ported: code rewrite, host app build, app binary build, app settings file and icon, XCI and NSZ input, geometry and mesh shaders.
- Verified on one reference title. The shader step for iOS was compared on a sample only.
- The pipeline archive step is slower than the reference scripts on the reference machine (273 s against 257 s).
- The reference scripts remain the fallback for every stage that is not ported.
