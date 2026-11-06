# Shader translator

This document describes the translator of the shader step. One Rust translator turns the machine code of a shader into
Metal Shading Language (MSL). It runs in the process of the shader step. It needs no .NET runtime and no external translator program.
The decision is D20 in [decisions.md](decisions.md).

## Function

The chain for each shader is:

1. Switch GPU machine code (Maxwell, or Ampere for NVN2).
2. `nxb_xlate::msl::translate`: decode, control flow, SSA, passes and the structured program, then MSL.
3. `metal` and `metallib`, or the own front end for the shaders that it accepts.

The games contain only machine code. The shader step writes one job for each shader with the code, the `c1` jump-table segment,
the answer table (`shader_facts.tsv`) and the options. The job writer is `nxb_shader::job`. The translator takes the job and the code.

## Crates

| crate | content |
|---|---|
| `nxb-maxwell` | the decoders for Maxwell and Ampere |
| `nxb-xlate` | the translation and the MSL back end `nxb_xlate::msl` |
| `nxb-shader` | the job writer, the chain version, the guards and the side library cut |
| `nxb-stages` | the shader step: it calls the translator, compiles the libraries and writes the files |

## Contract

The call is `nxb_xlate::msl::translate(&job, &code, &ctx)`. It returns `Translated` or `Failure`.

- `Context` has the chain version V, the source (`<package>#<index>`), the paired shader of the game's program (`pc_paired`), the answer-table flag `no_facts` and the `partners`.
- `partners` is used by vertex shaders. It holds the `StageInputs` (the user varyings) of the paired pixel shaders. The translator pads the vertex outputs from them.
- `Translated` has the libraries, the `StageInputs` of a pixel shader and the optional `Side`.
- A `Library` has a suffix (`_b`, `_bf`, `_bp`, `_bv`, `_bg*`, `_bl*`, `_bgv`, `_bx`), the MSL text and an optional `Reflection`. The first library is `_b`.
- The `Reflection` is the typed content of `<key>_b.json`. `to_value()` gives the JSON value that the step writes with the same writer as before.
- `Side` names the alpha test entries and the logic op entries of a pixel shader. The step cuts them from the `_b` library into `_bx` at the metallib level.
- `Failure.reason` is one of the reason strings of [shader-step.md](shader-step.md). The step counts it as the refusal of the translator.

The output keeps the contract of the engine host: the argument buffer at `[[buffer(0)]]` with `[[id(n)]]` members, the entry `main0` and its extra entries,
the library suffixes and the fields of `_b.json`. The facts in `_b.json` come from the emitter. Nothing parses MSL text.

## Identity and the cache

The step stores each translation in the store. The key has the translator identity, the job, the code, the chain version V, the source, the paired shader,
the answer-table flag and the varyings of the partners. The identity is the chain version V, which covers the sources of `nxb-xlate` (refer to [shader-step.md](shader-step.md)).
`NXB_TEST_XLATE_SALT` changes it. A failure is not stored.

## 64 bit floats

Metal has no `double`. The retry loop of `nxb_xlate::driver::retry` translates a job again with `softfloat64` set when the structured program uses 64 bit floats.
The passes `opt/double_to_float.rs` and `xform/softfloat.rs` then lower them to 32 bit. The back end never sees a 64 bit float after the retry.

## Output

The MSL is not byte-identical to the output of the former C# translator and `spirv-cross`. GPU tests validate it.
The own front end and the Apple tools compile the MSL as before.

## Open points

- The MSL back end is the part that fills `translate`. A shader that it does not translate gets `Failure` with the reason `not ported` until then.
