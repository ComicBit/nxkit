# Shader translator

This document gives the translator part of the shader step and the plan to replace it. The study is from 2026-11-05.
It is a read-only survey. Numbers with the label "estimate" are not measured.

## Function

The chain for each shader is:

1. Switch GPU machine code (Maxwell, or Ampere for NVN2).
2. `dmsh_xlate`: a C# driver around the Ryujinx shader translator. It writes SPIR-V and `res.json`.
3. `spirv-cross` (a pinned C++ binary). It writes MSL.
4. The MSL passes (ported to Rust in `nxb-shader::msl`).
5. `metal` and `metallib`.

The games contain no SPIR-V. They contain only NVN machine code. The shader step writes one job JSON for each shader with
the code, the `c1` jump-table segment, the answer table (`shader_facts.tsv`) and the options. SPIR-V is only an exchange format between two tools.

## Size of the components

The driver has 291 lines of C#. The translator has 46,878 lines of C#, of which 44,010 are in scope. The SPIR-V generator
has approximately 3,200 hand-written lines. The `spirv-cross` MSL path has an estimated 53,000 lines of C++.

The local changes in the translator (17 files) add non-constant texel offsets, bindless handles through phi nodes,
1D texel fetch as wrapped 2D, geometry passthrough to compute, bounded image atomics and the full Ampere front end.
These parts have no upstream reference. Thus they have the highest risk.

## Contract

- The call is `dmsh_xlate <jobs.json> <outdir>`. For each key, it writes `<key>.spv` and `<key>.res.json`. Failures go to stderr as `[RED] <key>: ...`.
- For one task, the translator can try up to four times: with guessed image types, with guessed texture types, with `softfloat64`, and with a geometry reader.
- `spirv-cross` options: `--msl --msl-version 20300 --msl-argument-buffers --msl-texture-buffer-native --msl-pad-fragment-output`. MSL 3.1 when the module has `OpImageTexelPointer`.

## Input of the reference title (measured)

| measure | value |
|---|---|
| modules | 16,268 (11,340 fragment, 4,857 vertex, 71 compute) |
| SPIR-V size | 736 MB, median 38.6 KB, maximum 503 KB |
| instructions | 43.7 M, 117 distinct opcodes |
| top 10 opcodes | 89.9 % of all instructions |
| `OpPhi`, `OpSwitch`, `OpLoopMerge` | 2, 0, 4,645 |

`OpLoad`, `OpStore` and `OpVariable` are 68 % of all instructions. The code contains structured selections, almost no loops and no `OpSwitch`.
This is a narrow, regular input for an MSL writer. Other games will add atomics, subgroup operations and FP64.

## Time (measured, warm store, internal disk, 12 tokens)

The translator batches took 440 s of token time, and 529 vertex shaders translated again took approximately 500 s (estimate).
`spirv-cross` took 8.7 ms for each call (measured on 10 modules). Warm wall time 170 to 183 s. Cold wall time 666 s (the compile time is larger). The run had 667 .NET starts.

## Options

| option | content | effort (estimate) | time decrease (estimate) |
|---|---|---|---|
| (a) servers | a translator host that loads the unchanged `dmsh_xlate.dll`, and a `spirv-cross` server over its C API | 6 to 10 work-hours | warm -55 to -75 s, cold -50 to -80 s |
| (b) Rust translator | a port of approximately 47,500 lines C# to an estimated 30,000 to 36,000 lines Rust, byte-identical SPIR-V | 90 to 140 work-hours | warm -15 to -30 s over (a), no .NET in the product |
| (c) Rust `spirv-cross` | a port of the MSL path for the subset in use, byte-identical MSL | 100 to 160 work-hours | 5 to 10 s |
| (d) direct MSL | the Rust translator writes MSL without SPIR-V and `spirv-cross`. Not byte-identical, validated by GPU tests. | 50 to 80 work-hours | warm -10 to -25 s over (b) |

Option (a) keeps the translator binary and its fingerprint. Thus it does not change the chain version.

The highest risks of (b) are the SPIR-V id order (it follows the emit order and the C# dictionary order), float folding
with the same bits as C#, and the local changes without a reference implementation. A development probe that writes the IR
after each pass as text, in C# and in Rust, finds the first different pass.

Risks of (c): expression forwarding, variable hoisting, name deduplication, argument-buffer layout, helper functions,
and float literal format. `spirv-cross` uses `snprintf`, which the Rust formatter does not copy.

## Phases

S1 is the servers of option (a). S2 is the Rust translator of option (b) in five work phases: skeleton and probe, decoder
and SPIR-V generator, emitters and optimizations, structured IR and code generator, other games and the local changes.
Each phase ends when its probe output is the same as the C# output on the full corpus. S3 is option (c) or (d).

## Recommendation

1. Do S1 first. It gives the largest safe decrease of time and keeps the chain version.
2. Then do S2, the Rust translator, byte-identical.
3. For S3, prefer (d) to (c), but only after S2 is byte-identical. If byte identity stays mandatory, use (c) for the measured subset, with the server as fallback.

Estimated total for S1, S2 and S3 (d): the warm translator share decreases from approximately 1,100 token-s to approximately 200 token-s.
At 12 tokens, that is approximately 90 to 130 s less warm wall time.
