# Early cutoff

This document gives the key scheme of the shader step (decision W21). Each stage has its own content key. No key contains
the chain version V. Thus a change of the chain does work only for the outputs that it changes.

## Problem

The host loads the shader libraries from the folder `b-<V>/`. V is a digest of all chain sources. The reference build
scripts put the code of the full chain into each cache key. Thus a change of one line in the chain makes all cache
entries of all games invalid, and the next build compiles all shaders again.

## Decision

Each stage output has a key from two parts: the version of that stage and the content hash of its inputs. No key contains V.

- A stage runs again only when its version or the content of its inputs changes.
- If the new output has the same bytes as the previous output, the keys of all subsequent stages stay the same. These stages do not run.

This is early cutoff, as in the build systems Shake, Bazel and Salsa.

| stage | store domain | version part | content part |
|---|---|---|---|
| translator | `nxb.xlate.v1` | translator build id: SHA-256 of the translator assemblies | job JSON without the scratch path, SHA-256 of the code bytes |
| `spirv-cross` | `nxb.msl.v1` | SHA-256 of the `spirv-cross` binary | arguments, SPIR-V bytes |
| MSL passes, variants, reflection | not stored | none | none |
| Metal compile and link | `nxb.metallib.v2` | target record (flags), compiler identity (`metal` and `metallib` versions, SDK), own front-end tag | the last MSL text |
| pipeline archive batch | `nxb-archive batch 1` | architecture, platform, minimum OS, `metal-tt` version | library bytes and the entries of the bases of the batch |

The MSL passes, the variants and the reflection run again in each build. They are pure Rust and take seconds for all
shaders. A cache for them costs more time than it saves.

The effect of a chain change:

- A change of one MSL pass compiles again only the shaders with a changed last MSL, and their archive batches.
- A new translator binary with the same output translates again, but does not compile a shader.
- A new Xcode compiles all shaders again, because the compiler identity is part of each compile key.

## Rules

1. The output folder stays `b-<V>/`, because the host loads only that folder. But no store key contains V.
   A new V makes a new folder from cache hits.
2. A chain change does not delete data. `shaderbuild` does not remove store entries. Old entries stay in the store until a prune by last use.
3. The version of a stage must include all data that changes its output, and no other data.
   Tool binaries go into the version by their content. The Rust passes run in each build, thus they have no version.
4. The reuse key of the build contains only the id of the `shaderbuild` binary. `shaderbuild` owns the keys of each output.
5. If a pass becomes slow, it gets a key from its pass version and the hash of its input MSL, in the same scheme.

## Measurements

Measured on the reference title, Mac target, on the reference machine. The machine load was 21 to 47 during the runs.
Each run started from the store of a full build. The full build took 687 s.

| change | shaders with a change | wall time |
|---|---|---|
| V changes, no other change | 0 compiles, 0 translator starts | 47 s |
| a pass changes 1 % of the pixel and compute shaders | 131 | 66 s |
| a pass changes 10 % of the pixel and compute shaders | 1,119 | 119 s |
| a pass changes 100 % of the pixel and compute shaders | 11,411 | 528 s |
| new translator build with the same output | 0 compiles | 107 s |
| new compiler identity | all | approximately the time of a full build |
| full build from an empty store | all | 687 s |

## Test hooks

These environment variables are only for the benchmark. They have no effect when they are not set.

- `NXB_TEST_PASS_SALT=<percent>` adds a comment to the MSL of that share of pixel and compute shaders. This simulates a pass that changes the output.
- `NXB_TEST_XLATE_SALT=<s>` changes the translator id. The translator output stays the same.
- `NXB_TEST_COMPILER_SALT=<s>` changes the compiler identity.
