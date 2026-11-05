# Changelog

All notable changes are in this file. The format follows Keep a Changelog. Versions follow Semantic Versioning.

## 0.3.0

### Added

- iOS stages in `nxb-build::ios` and a new binary `nxb-ios`. It does the work of the reference iOS scripts with the same command lines
  and environment: `nxb-ios ipa [<out.ipa> | --prepare-xcode]` and `nxb-ios shell --prepare-xcode | --build`.
  `xcodegen`, `xcodebuild`, `codesign` and the other Apple tools stay external. Exit code 3 means unsupported: the caller runs the reference script.
- Stage overlap in `nxb build`. The Mac host starts at the same time as the extract. The companion app, the icon and the settings file
  start when the extract and the host are done, beside the shader step. The stream of the pipeline archive gets more workers
  (one for each slot, if the memory is sufficient) when the shader step stops. The events are the same as in a run in series, except the times.
- Raw container tier `own-raw` in the shader step (Mac only). The own front end writes the bitcode and the library container itself
  for shaders on a second allow list. No Apple tool and no process is necessary for these shaders.
  `NXB_OWN_RAW=0` turns the tier off. `NXB_OWN_RAW=<file>` selects a different list.
- The front end has an optimizer for the raw path in `air-bc` (`mem2reg`, removal of dead code, block-local CSE) and the commands `airc raw`, `rawcmp` and `rawcheck`.
- The release ZIP file holds `nxb-ios` in addition to `nxb`, `nxb-extract`, `nxb-split` and `airc`.

### Checked

- iOS stages, each against the reference script on real or synthetic inputs. The manifest, resource, embed and build-setting generators
  and the Xcode workspace writer: byte-identical. The shell project: all files equal, except the font file (equal in size only,
  because the font tool writes a time stamp). The IPA flow on one title: 75 of 75 entries equal in name, method, sizes, CRC and attributes.
  Two entries are equal only in part. In the executable, only `__LINKEDIT` differs (debug map paths and signature), and the bytes before it are identical.
  The font file is equal in size only. In this run each side used its own `xcodebuild`.
- Overlap: on one title, with extract, split and shaders reused, the event streams of the old and the new binary are identical (except the times).
  58,211 of 58,211 paths are present in both outputs. All files are byte-identical, except the two rebuilt Mach-O files (`LC_UUID` and debug map) and their two lines in `CodeResources`.
- Raw tier: the allow list has 3,365 shaders. Each one gave the same GPU output as Apple's library on 12 test vectors.
  The test covered 3,625 of 23,752 candidates (3,375 the same, 220 different, 29 not deterministic, 1 not built).
  The raw libraries are not byte-identical to Apple's libraries. They are different by design.
  On a sample of 40 shaders, the tier made 44 raw libraries with no fallback. The 61 other libraries were byte-identical to the reference.
  With the tier off, all 105 libraries were byte-identical to the reference.

### Measured and estimated

- Raw tier, measured on a small sample on a loaded machine: median compile CPU of 97.5 ms for each library, against 109 ms without the tier (about 11 % less).
  This is not the full corpus and not a full build. The estimate of 4 to 6 minutes less on a cold build is **not** measured.
- Overlap, measured on one run with reused extract, split and shaders: 133.0 s against 136.2 s. The change is small, because the host and the app stages
  are the critical path there. The gain on a cold build is an **estimate** (the host build of 22 to 30 s and the app parts leave the critical path).
  Nobody timed a cold build with overlap.

### Limits

- `nxb build` still takes only the Mac target. `nxb build` refuses `--platform` for iOS. The iOS stages run through `nxb-ios` only.
- `nxb-ios` returns exit code 3 for games with more than one program and for framework builds.
- The default name of the shell app is `NxbShell`. The reference script has a different default name. Set `DM_SHELL_NAME` to get the same name.
- Not run: `nxb-ios shell --build`, and an IPA with a pipeline archive. The IPA is always packed in full (no reuse of the previous IPA).
  The `codesign` of the host signs the executable. The own signer does not.
- The allow lists for the own front end and for the raw tier are not in this repository. Without a list, the tier is off.
  The raw list covers about 14 % of the corpus. The raw tier is not available for iOS (there is no GPU test for iOS libraries).
- After a change of the front end, make the raw list again and change the front-end id. If you do not, the cache keys stay the same.
- Not ported: code rewrite, button-icon font, header of multi-program profiles, zstd dictionaries.

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
