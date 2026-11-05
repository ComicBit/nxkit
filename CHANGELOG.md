# Changelog

All notable changes are in this file. The format follows Keep a Changelog. Versions follow Semantic Versioning.

## 0.4.0

### Changed (read this first)

- **The raw container tier is now on for each Mac shader that the own front end accepts.** In 0.3.0 it was on only for shaders on an allow list.
  The tier makes the library itself and does not start an Apple tool. If the front end rejects a shader, the shader goes to Apple's compiler.
  The shader also goes to Apple's compiler when the library does not pass a check in the process.
  **The raw libraries are not byte-identical to Apple's libraries.** A GPU test was done for a part of the corpus only.
  It covered 3,625 of 23,752 candidates (about 15 %). In that test, 3,375 shaders gave the same output as Apple's library and 220 gave a different output.
  Nobody ran a GPU test for the other shaders, thus the rate of different output for them is not known.
  A smoke test of the GPU harness on 8 shaders gave 7 equal and 1 different (the largest error was 166 ulp). This is not a sample and gives no rate.
  To turn the tier off, set `NXB_OWN_RAW=0`. To turn off all own tiers, set `NXB_OWN_FRONT=0` or use `--own-front off`.
  `NXB_OWN_RAW=list` gives the old behavior (only the shaders of a raw allow list, which is not in this repository, thus the tier stays off unless you give a list).
  The cache key of a raw library has the field `own-raw@<front-end id>` for each shader. Entries from the 0.3.0 list mode are not reused.
- The exit code for an unsupported input is now 4 for `nxb extract`, `nxb-extract`, `nxb-split`, `nxb rewrite`, `nxb nro`, `nxb-ios` and `nxb build`. Exit code 3 means damaged data.
  The 0.3.0 text said that `nxb-ios` returned 3. The signer binary `nxb-pack` still returns 3 for an unsupported input.
- The translator host is the default when its binary is next to the translator (`--xlate-host off` turns it off).
  The Rust translator is on by default (`--xlate-rust off` or `NXB_XLATE_RUST=0` turns it off).
- Events have two new fields, `code` and `severity`. The old fields do not change, and `t` stays last.

### Added

- The code rewrite step in Rust (`nxb rewrite`, crates `nxb-a64` and `nxb-rewrite`). It has the command line of `patch_nso.py`. The extract stage of `nxb build` uses it.
  It reads NSO and NRO modules, including modules with ZBIC compression. The decoder `nxb-a64` follows Capstone 5 for the fields that the rewriter uses.
- The rewrite rules are data in `data/rewrite-rules` (117 rule files, `snapshot.tsv` and the licence notice of the engine that they come from).
  The release ZIP file has the folder `rewrite-rules` next to the binaries. `nxb rewrite` finds it there. `nxb rewrite-rules` makes the files from a snapshot.
- NRO collection in Rust (`nxb nro`, crate `nxb-nro`). It has the command line of `nro_collect.py`.
- Translator host as the default (`xlatehost` in `nxb-stages`). One host process does the work of all batches. The counters for starts, restarts and fallbacks are in the run report.
- Rust translator (crates `nxb-xlate`, `nxb-maxwell`, `nxb-spvgen`). It translates in the process, for Maxwell and for Ampere machine code (Switch 2).
  A key that it cannot do goes to the C# translator, and the reason is counted. The translator also writes `<key>.iface.json` (the `ShaderInterface`) next to `<key>.res.json`.
- `nxb-extract` and `nxb-split`: zstd dictionaries, many more readers (Unity bundles and serialized files, archive families with ranges, Needle, Sumo, Unreal Engine and more),
  and the options `--allow-damaged`, `--manifest` and `--aoc`. A format that is not ported stops with exit code 4 and a named message.
- iOS: games with more than one program and framework builds.
- Also for iOS: the button-icon font, the header of multi-program profiles, the own signer for the iOS executable and the reuse of the previous IPA.
  `nxb build --platform ios` runs the iOS flow with the usual events.
- `nxb cache export`, `import`, `prune` and `list`: the cache of a title as one export file with a checksum.
- Manifest-first order for `nxb build`: `--shader-manifest <file>` builds the listed shaders first. `--partial` builds only these shaders and writes `_b_partial.json`.
- The crate `nxb-profile` (the profile of a game as typed data with a TOML reader and writer).

### Checked

- Rewrite: byte-identical to `patch_nso.py` on 25 of 25 real modules (all five tables, `.patched.bin` and the fixups). The modules include one Switch 2 title,
  modules with ZBIC compression, modules with an NRO table and modules with frame step stubs. The 98 NRO modules of one title are also identical (98 of 98).
  With the rules from `data/rewrite-rules`, 120 of 130 earlier cases ran again and all were identical. 14 cases cannot run again, because their input files are deleted (their rules are unchanged).
- NRO collection: `nro.tsv` and 98 of 98 modules are byte-identical to `nro_collect.py` on one real image.
- Rust translator: SPIR-V and `res.json` are byte-identical to the C# translator for all keys of three titles, first-attempt jobs only
  (16,268 of 16,268, 19,592 of 19,592 and 28,669 of 28,669), and for 42,299 of 42,331 keys of one Switch 2 title (Ampere).
  Of the 32 other keys, 17 fail in the C# translator also and 15 go to the C# path (`SharedStoreSmallIntCas`). The corpus has no job for task and mesh shaders.
- Translator host: 802 of 802 results equal to the process bridge.
- Extract and split: one real update image with a 17.6 GB archive gave all package files and tables byte-identical to the reference split. Real zstd dictionaries: byte-identical to `zstd -D`.
  The readers for the other formats were compared with the reference functions on synthetic files.
- iOS: 142 of 142 profiles byte-identical in the profile port. The framework flow, the multi-program flow and the IPA reuse were compared with the reference script on synthetic fixtures
  (the framework IPA also with a real `xcodebuild` run: 47 of 47 entries equal, the executable, the framework binary and the font file equal in part).

### Only synthetic or only partly checked

- Unity readers, archive families without a local sample, layouts of very large text segments (100, 125.3 and 185.6 MB), three frame step builds, one build with registered cuts,
  framework builds and multi-program builds: only synthetic files or fixtures. The rules for real code of those builds are not tested.
- Switch 2 packages with an update and DLC. Ampere translation was checked on one title.
- `nxb build --platform ios` was not run end to end. The wiring is checked up to the first check of the inputs.
- `nxb-ios shell --build` was not run.
- `nxb cache` makes and reads export files. The shader step does not yet write cache entries for it.
- A full `nxb build` in one run with the new defaults was not timed.

### Measured and estimated

- Rewrite (release build, machine not quiet): 3.4 s for a 29 MB main module against 63 s for the script, and 8.9 s against 121 s for a larger one. Synthetic large modules: 1 to 3.5 s against 183 to 672 s.
- NRO collection: 3.5 s against 4.5 s. The archive table dominates both times, so there is no gain in speed.
- Translator host: 1.0 to 1.7 s against 6.3 to 8.4 s for 802 jobs (one start instead of 19). In a subset run of a shader step, the wall time was 20.2 s against 20.7 s, because the Metal compile dominates.
  The change of the .NET starts from 667 to about 1 in a full build is an **estimate**. Nobody measured it on a full build.
- Rust translator: in a subset run (160 keys, load average about 10) the wall time was 17 s against 16 s with the C# translator. The Metal compile dominates.
- Raw tier on all shaders: in a subset run (160 keys, 261 shaders), 363 library compiles gave 363 raw libraries with no fallback, and all 699 output libraries load in Metal.
  The wall time was 6.5 s against 14.4 s with the tier off. This is not a benchmark. The gain on a full cold build is **not measured**.

### Limits

- Not ported in the Rust translator (the key goes to the C# translator): geometry, tessellation, transform feedback, vertex shaders as compute, the interlock function match,
  `SharedStoreSmallIntCas` and the task and mesh jobs. No corpus has the first four, thus they are not ported blind.
- Not ported: the CRA archive family, and each format that needs Oodle (these stop with exit code 4). Also not ported: zip members in bzip2 or lzma, and encrypted Unreal indexes.
- The rewrite port stops with exit code 4 when the engine files differ from the files that the rules come from. It needs the same engine version.
- `nxb split`, `nxb app`, `nxb ipa`, `nxb pso` and `nxb sign` print "not implemented yet". Use `nxb-split`, `nxb-macapp`, `nxb-ipa`, `nxb-pso` and `nxb-pack`.
- The own-identical allow list is not in this repository. The raw path does not use a list by default.
- Mac only for the build tools. The raw tier is not available for iOS.

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
