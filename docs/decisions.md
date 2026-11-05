# Decisions

This document records the architecture decisions and their causes. Part 1 has the workspace decisions W1 to W21.
They apply where they are different from the two design proposals. Part 2 has the design decisions of the shader step and of the other stages.

## Principles

All stages are in Rust, and the reference build scripts are only the test reference. A port gives byte-identical output.
The own front end is the one exception. The archive back end stays Apple's tool. The workspace layout sets names, layers
and shared APIs. The designs set the behavior.

## Part 1: workspace decisions

| # | decision | cause |
|---|---|---|
| W1 | One prefix `nxb-` for all crates. The binaries `nxb` and `shaderbuild` come from one crate. | One name space for one workspace. |
| W2 | `nxb-model` (identifiers, digests) and `nxb-error` (classes, reason strings) are two crates in L0 and in V. | The other stages use identifiers and errors without the shader ABI constants. |
| W3 | Own SHA-256 in `nxb-model`. Tests compare it with FIPS vectors and Python `hashlib`. | No network, no vendored code, one implementation for all systems. |
| W4 | The `Translator` trait is in `nxb-tool` (L2). | The trait returns `ToolError`. Process and server bridges are its implementations. |
| W5 | Events are in their own crate `nxb-obs`. | The scheduler, the tool layer and all stages write events. |
| W6 | Store format 1 is the own content-addressed store. A compatible tree reader is an adapter. Format 2 has segment files. | This prevents two meanings of "v1". |
| W7 | Four layers L0 to L3 with an explicit list of sideways edges in L1. | One rule set and one checker. |
| W8 | `nxb-stages` holds all stage modules. `nxb-cli` holds the two binaries. | Fewer crates. |
| W9 | The NSO parser is part of `nxb-rewrite`. The format modules start in `nxb-split`. | Fewer crates. The names stay reserved. |
| W10 | The ad-hoc signer started as a standalone crate. It is part of `nxb-pack` (L3) at this time. | Parallel work with its own file set. |
| W11 | Single-flight uses continuations (`claim` with `on_ready`), plus `get_or_run`, which waits. | A worker that waits is an idle core. |
| W12 | Scheduler queues are `Mutex<VecDeque>` with a priority injector. | Standard library only. Tasks take milliseconds, thus the lock cost is small. |
| W13 | `rust-version` in the workspace, no `rust-toolchain.toml` at this time. | The build machine has no rustup. |
| W14 | Withdrawn. A checkout compiles into `./target`. `CARGO_TARGET_DIR` and `CARGO_BUILD_JOBS` change this. | |
| W15 | `Budget` calculates tokens from the P and E core counts and the factor e. | The budget follows the metric (PCE). e is 0.4 until a measurement replaces it. |
| W16 | V contains `nxb-model`, `nxb-error`, `nxb-pyjson`, `nxb-shader`, `nxb-air` and their data. The other stages have their own format versions. | `nxb-error` holds reason strings that go into `_b_build.json`. |
| W17 | `nxb-model` uses `nxb-sys` for the ARMv8 SHA-256 block function, with a scalar fallback. | Measured: 1,841 MB/s against 225 MB/s on 1 GiB. The intrinsics are `unsafe`, and only `nxb-sys` has `unsafe` code. The digests are the same, thus V does not change. |
| W18 | Shader front ends are a `FrontEnd` trait in `nxb-tool`: `AppleFrontEnd`, `OwnFrontEnd`, `WithFallback`. The verdict data is data of the shader domain. The default uses the allow list (refer to D18). | The front-end id keeps the cache entries apart. |
| W19 | The stage crates with I/O (`nxb-extract`, `nxb-pack`, `nxb-archive`) and the orchestrator `nxb-build` are L3 crates with listed sideways edges. | Each stage crate has one owner. |
| W20 | The front-end crates are a separate layer F with their own names. | They are standalone and use only the standard library. Only `nxb-tool` can use `air-front`. |
| W21 | Early cutoff: each stage output has a key from the stage version and the content hash of its inputs. No key contains V. A V change does not delete data. The MSL passes run again in each build. | A change of one line in the chain must not compile all shaders of all games again. Measured: a V change alone took 47 s with 0 compiles (refer to [early-cutoff.md](early-cutoff.md)). |
| W22 | The shader step translates each key with the Rust translator first, then with the translator host, then with the process bridge. Each fallback is counted by reason. | The Rust translator does not cover geometry, tessellation, transform feedback, vertex shaders as compute and some rare cases. The output is byte-identical for the keys that were compared, thus the cache identity does not change. |

## Part 2: design decisions

### Shader step

| # | decision | cause |
|---|---|---|
| D1 | V from the sources that have an effect on the output, and `OUTPUT_VERSION`. | A runtime fix must not make libraries invalid. A domain change must change V. |
| D2 | Binary provenance in `shaderbuild.lock`. BIN_ID in the reuse key of the build. | This separates the code that made the bytes from the binary that ran. |
| D3 | The own front end is not byte-identical. Image correctness validates it. | Unoptimized IR gave the same library in only 2 of 300 samples (measured). |
| D4 | Three gates: `gpudiff` with class tolerances, coverage, scene comparison. | Each gate finds a different error type. |
| D5 | Fallback to the Apple front end for each shader. `NXB_OWN_FRONT=0` sets the Apple front end for all shaders. | An incorrect shader costs one fallback, not a release. |
| D6 | First back-end path: unoptimized IR into Apple's back half. | This keeps Apple's AIR passes. The back half took 57 ms median (measured). |
| D7 | Front-end id in TOOL_FP and in the Metal cache identity. | Apple and own libraries do not mix. |
| D8 | `SDKROOT` for each target and a fixed environment allow list. | A direct call without `SDKROOT` gives different bytes (measured). |
| D9 | Python regular-expression semantics behind one shim. Hand-written matchers for frequent patterns. | 19 back-references and look-arounds. A verbatim port is the safest path to parity. |
| D10 | Error classes and a circuit breaker. | A full disk must not become thousands of shader failures and an incorrect reuse key. |
| D11 | Manifests with SHA-256, quarantine, dirty marker. | This finds damage and keeps the evidence. |
| D12 | Process groups and division of bad batches. | One bad shader costs log2(batch) runs. |
| D13 | Memory budget from the memory size and the peak memory of the previous build. | Games with 130,000 shaders and Macs with 8 GB. |
| D14 | CPU use in PCE. | E cores are slower. A core count hides a P-core shortage at the end. |
| D15 | Drift samples. | They find nondeterminism, damage and toolchain changes at approximately 0.5 % cost. |
| D16 | Typed task graph in memory. Each external tool is behind a `Tool` adapter. The first adapter is a child process. | A tool changes to a persistent worker only when a benchmark shows a better time and parity stays. |
| D17 | No own optimizer and no own `metallib` writer. | No pass subset in the test gave Apple's machine code (0 of 15). |
| D18 | The own front end uses the allow list by default: only shaders with the verdict `own-identical` in the shipped verdict file. | With a deny list, a shader that is not in the corpus goes through the own path without a check. |
| D19 | Since 0.4.0, the raw path is on by default for each Mac shader that the own front end accepts. It has a check in the process and a fallback to Apple. `NXB_OWN_RAW=0` turns it off. It supersedes D18 for the raw path only. | The path is faster and needs no Apple tool. The cost is that its output is not byte-identical to Apple's output, and a GPU test covered only about 15 % of the corpus. D17 stays for the identical path. |

### Other stages

| # | decision | cause |
|---|---|---|
| E1 | Rust extract, one process, all versions in one pass, shared block cache, parallel `pwrite`. | The C# path reads, decrypts and examines the base two times. |
| E2 | Own AArch64 decoder `nxb-a64`. | The rewriter uses only register and memory operands. A C library is then the only native build dependency. |
| E3 | Own ad-hoc signer. `codesign` is the reference and the fallback. | `codesign` reads 4.7 GB again. The own signer uses the known digests. |
| E4 | The split operates on the data of the extract. | The stages read each file only one time. |
| E5 | One hash for each byte, at the producer. | No byte gets a second hash. |
| E6 | Archive batches as tasks under a memory budget, with the same merge sequence. Each batch runs `applegpu-nt -j 1` and `air-pack`, with one batch for each core. | Measured: 207 s against 257 s cold, and the loader probe found all 18,300 bases. |
| E7 | One `nxb build` with one task graph. Stage commands first, as a migration step. | One graph lets the extract feed the split and the shader step feed the archive. |
| E8 | Own zstd decoder in `nxb-split`, standard library only. | NSZ, XCZ and `.zs` files use zstd. The workspace has no external crates. |
| E9 | No own GPU code generator. | Closed plugin, private format and ISA (refer to [research/backend-survey.md](research/backend-survey.md)). |
