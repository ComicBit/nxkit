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

- `Context` has the chain version V, the source (`<package>#<index>`), the paired shader of the game's program (`pc_paired`), the answer-table flag `no_facts`, the flags `dual_source` and `fetch_outputs_only` and the `partners`.
- `partners` is used by vertex shaders. It holds the `StageInputs` (the user varyings) of the paired pixel shaders. The translator pads the vertex outputs from them.
- `Translated` has the libraries, the `StageInputs` of a pixel shader and the optional `Side`.
- A `Library` has a suffix (`_b`, `_bf`, `_bp`, `_bv`, `_bg*`, `_bl*`, `_bgv`, `_bx`), the MSL text and an optional `Reflection`. The first library is `_b`.
- The `Reflection` is the typed content of `<key>_b.json`. `to_value()` gives the JSON value that the step writes with the same writer as before.
- `Side` names the alpha test entries and the logic op entries of a pixel shader. The step cuts them from the `_b` library into `_bx` at the metallib level.
- `Failure.reason` is one of the reason strings of [shader-step.md](shader-step.md). The step counts it as the refusal of the translator.

The output keeps the contract of the engine host: the argument buffer at `[[buffer(0)]]` with `[[id(n)]]` members, the entry `main0` and its extra entries,
the library suffixes and the fields of `_b.json`. The facts in `_b.json` come from the emitter. Nothing parses MSL text.

## Entry functions

`msl/entry.rs` writes one function around a translated body. `EntryName` names it: the function name, its `Form`, the return type, the stage input (`StageIn`), extra `Param`s and prologue statements. The form is an entry function or a `static` body function `dm_body_<name>` with the parameters but no attributes.
The shared memory of a body function is a parameter, because its caller owns it. `msl/library.rs` builds the libraries: `_b` with its reflection, `_bv` and `_bp` for a vertex shader, and `_bf` and the side units for a pixel shader.
A variant writes its entries with `EntryName { field: .., ..EntryName::new("main0") }`.
Pixel entries and the wrapper of a task or mesh shader call a body function. The end of a vertex entry comes from `Plan::vertex_epilogue`: the depth conversion. A viewport mask that a vertex shader writes becomes `gl_ViewportIndex`, the lowest set bit capped at 15. `main0_z` and `main0_lz` keep the depth range of the source.

## Vertex variants

The emitter produces everything a vertex shader needs from one trace. The code is in `crates/nxb-xlate/src/msl/variant/`.

- The vertex outputs are padded to the inputs of every paired pixel shader. A varying the pixel shader reads and the vertex shader does not write gets a default. The locations are in `vs_pad`.
- An attribute that the shader only uses as an integer is declared `uint`. The emitter probes this and builds the shader again.
- The entries `main0_z`, `main0_discard`, `main0_l`, `main0_lz` and `main0_gs` are further entries of the `_b` library. `entries` and `msl_entry_kind` in `_b.json` name them.
- `_bv` is the library with shadow inputs for integer attributes. The shadows sit at attribute indices 16 to 30. The field `vin` in `_b.json` lists them.
- `_bp` is the library that pulls the vertices from a buffer with `dm_fetch`. The field `pull` in `_b.json` names the entries and the buffer slot 16.
- `raster` has `pos_attr`, `pos_w`, `uv_from_attr`, `uv_affine` and `screen`. They come from the stores of the main function. A shader with a branch, a loop or a call has no raster facts.

## Compute kernels

A kernel with more than 32 KB of shared memory takes its arrays from a device buffer at `buffer(30)` (`spill`). The reflection field `tg_spill` has the bytes of one workgroup slice. The stores that only the first invocation of a dispatch makes leave the kernel, and the reflection field `first_store` lists them as `{reg, word, value}` for the host. A task or mesh shader keeps its shared memory in the threadgroup and is refused past the limit.

## Pixel variants and side units

A pixel shader gets extra entries around one static body function `dm_body_main0`. Each entry is a thin function that calls the body and then changes the result.

| entry | purpose | `_b.json` field |
|---|---|---|
| `main0_a` | alpha test, state at `[[buffer(30)]]` | `alpha` `{entries}` |
| `main0_l` | logic op, state at `[[buffer(29)]]`, function constants 200 and up | `logic` `{entries}` |
| `main0_u` | integer colour outputs | `entry_u` |
| `main0_m` | mixed outputs, with a mask for the integer targets | `entry_m`, `uint_mask_m` |
| `main0_d` | dual source blending | `entry_d` |

`Translated.side` names the alpha and logic entries. The step cuts them from `_b` into `_bx` with `nxb_shader::side::cut`.

The library `_bf` is a second build of the pixel shader for framebuffer fetch. A sample of a 2D colour texture is replaced by the attachment `fbK` when its function constant `FX_i` selects it.
The coordinate must be a varying (optionally divided by `w`) or a pixel position times uniforms.
`_b.json` gets the field `fetch` with `calls` (`fc`, `tex_reg`, `varying_locn`, `proj` or `frag_pos`), `mixed_regs` and `outputs`.
Only samples in the main function are replaced. A shader without such a sample gets `_bf` only when `Context.fetch_outputs_only` is set. The step sets it for the keys in the profile list `bf_want.tsv`.
`Context.dual_source` makes the translator build `main0_d`. The step sets it for the keys in the profile list `dual_want.tsv`.

A discard in a helper function sets the flag `dm_kill`. Every caller returns at once after the call, as the main function does after its own discard.
A fragment shader interlock (`FSIBegin`, `FSIEnd`) has no statements. The images and storage buffers that the shader writes carry `raster_order_group(0)` in the argument buffer.
`raster.sample_mask` is false because the IR has no sample mask output. `first_store` belongs to compute kernels and has no pixel form.

Every fragment entry gets `[[early_fragment_tests]]` when `early_tests` is true. That is the case when the shader forces the early depth test, or when its texture pool marks IDs and the shader has no discard, depth write or memory write (`pool::early`). The rule lives in `entry::early_tests`, and the base entries, the side entries and the fetch entry use it.

## Identity and the cache

The step stores each translation in the store. The key has the translator identity, the job, the code, the chain version V, the source, the paired shader,
the answer-table flag, the dual-source and fetch-outputs-only flags and the varyings of the partners. The identity is the chain version V, which covers the sources of `nxb-xlate` (refer to [shader-step.md](shader-step.md)).
`NXB_TEST_XLATE_SALT` changes it. A failure is not stored.

## 64 bit floats

Metal has no `double`. The retry loop of `nxb_xlate::driver::retry` translates a job again with `softfloat64` set when the structured program uses 64 bit floats.
The passes `opt/double_to_float.rs` and `xform/softfloat.rs` then lower them to 32 bit. The back end never sees a 64 bit float after the retry.

## Output

The MSL is not byte-identical to the output of the former C# translator and `spirv-cross`. GPU tests validate it.
The own front end and the Apple tools compile the MSL as before.

## Geometry, task and mesh shaders

A geometry shader runs as a compute kernel. One job of stage `gs` gives the libraries `_b` (a copy of `_bg0`), `_bg0` to `_bg4` and `_bgv`. Each has its own `.json`.
The kernels `_bg0` to `_bg4` are one per input topology. They write the output vertices and the index buffer. The library `_bgv` draws that output.
The `.json` of a kernel has the member `geometry` (`vertices`, `invocations`, `output`, `index_stride`, `stride` in words). The `.json` of `_bgv` has `geometry_reader`, `geometry_output` and `entry_z`.

A task or mesh shader is a job of stage `cs` with the member `mesh_stage`. The code has no SPH. The job carries `stage_memory`, `shared` and `local` as the engine lays them out,
and the translator refuses a job whose layout differs. The body of the shader is one function. The wrapper is a Metal `[[object]]` function for `ts` and a `[[mesh]]` function for `ms`.
The `.json` has `stage` `ts` or `ms`, `msl_entry_kind` null and `mesh_stage` (`kind`, `threads`, `payload`, `task`). A mesh function also has `entry_z`.
The translator fails with `mesh stage failed` when the shader uses shared memory of its own or the sizes do not fit Metal.

Tessellation, transform feedback and vertex as compute are not ported. Each gives `Failure` with the reason `not ported` and a detail that names the stage.
The lean geometry kernels `_bl0`, `_bl1` and `_bl3` are not emitted.

## Open points

- The MSL back end is the part that fills `translate`. A shader that it does not translate gets `Failure` with the reason `not ported` until then.
