# Changelog

All notable changes are in this file. The format follows Keep a Changelog. Versions follow Semantic Versioning.

## 0.2.0

### Added

- `nxb-extract` reads game cards (`.xci`), compressed packages (`.nsz`, `.xcz`) and dump folders (`--dump`).
  The zstd decoder streams the data and has an output limit. `--app <id>` selects one application in a merged package.
- Mac stages in `nxb build`: the Mac host (`make` and `clang`), the companion app, the app settings file and the app icon.
  The reference build scripts are the oracle.

### Checked

- XCI, merged packages and dump folders: synthetic fixtures only. The output is byte-identical to the reference extractors.
  No real sample was available.
- NSZ: one real file. The decoded NCA is identical to the output of the reference decompressor. The NCZ block mode was not checked on real data.
- App icon and companion app: byte-identical on one title.
- App settings file: 425 of 425 outputs are byte-identical for 142 profiles. One multi-program header is not ported.
- Mac host: the reuse key and the decoder inputs are byte-identical. The binary is not byte-identical.
  The reference script also gives a different binary on each run (`LC_UUID` and the debug map). The code part is identical.

### Limits

- `nxb build` end to end with the new stages was not run on a real title.
- Not ported: the button-icon font, the header of multi-program profiles, zstd dictionaries, code rewrite.

## 0.1.1

### Added

- Reads Switch 2 packages (`.nup`) without keys. Same output layout as the reference script.
  Use `nxb extract --nup <game.nup>` or `nxb-extract` with the same arguments.
- Checked on one real package and on synthetic containers, including damaged ones.
- Not checked on real data: packages with base game, update and DLC, and the control data.

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
