# Glossary

This glossary gives one term for each concept. All documents in this folder use these terms and no synonyms.
The documents use Simplified Technical English (ASD-STE100). The terms below are technical nouns or technical verbs.

## Technical nouns

| term | definition |
|---|---|
| cache export | One file with the cache entries of a title, made by `nxb cache export`. It has a checksum at the end. |
| allow list | The list of shader digests with the verdict `own-identical`. By default, the own front end compiles only these shaders. All other shaders go to the Apple front end. |
| Apple front end | Apple's `metal -c` compiler, used from MSL source to AIR. |
| AIR | Apple's intermediate representation for Metal shaders. It is LLVM IR with Apple metadata and intrinsics. |
| back half | The part of the Metal compile after the front end: `air-opt` (or `metal -c` from IR) and `metallib`. |
| build | The full procedure that makes an app from a game package. |
| chain version (V) | The version of the shader output. It names the folder `b-<V>/`. A change of shader output bytes changes V. |
| compile input | One MSL source with one compile convention. The shader step compiles each compile input one time. |
| corpus | A set of compile inputs from one game, kept on the local machine for tests. |
| deny list | The list of shader digests that the own front end must not compile. |
| event code | The stable code (`NXB` and four digits) of an event in the NDJSON stream. Each event also has a severity. |
| early cutoff | A stage does not run when the keys of its inputs did not change. The term comes from build systems. Refer to [early-cutoff.md](early-cutoff.md). |
| fallback | The use of the Apple tool for a shader or a file that the own code does not cover. |
| game package | An input file or folder with game content: NSP, XCI, NSZ, XCZ, NUP or a dump folder. |
| host | The runtime program that loads the game files and the shader libraries on the target Mac or iOS device. |
| library | A Metal library file (`.metallib`). |
| manifest | The list of files of one store entry, with size and digest of each file. The store writes it last. |
| partial build | A build with `--partial`. It builds only the shaders of the shader manifest and writes `_b_partial.json`. |
| process bridge | The way to run the C# translator with one process for each batch. |
| provisional profile | The game profile that `nxb build` writes for a game that has no registered profile. The survey of the game gives its values. It is not checked by a run of the game. |
| own front end | The Rust compiler from MSL to AIR in this workspace (layer F). |
| parity test | A test that compares the output of a stage with the output of the reference build scripts, byte for byte. |
| PCE | P-core equivalent. PCE = P + e x E, with P and E the core counts and e the relative speed of an E core. |
| raw allow list | The list of shader digests that matched in the GPU differential test. Before 0.4.0, only these shaders used the raw path. Now the list is optional (`NXB_OWN_RAW=list`). |
| raw path | The path of the own front end that writes the bitcode and the library container itself. Its output is not byte-identical to Apple's output. Since 0.4.0, it is on for each Mac shader that the front end accepts. |
| reference build scripts | The Python and C# build scripts that the workspace replaces. The tests use them as the reference. |
| reference machine | An Apple M2 Max with 8 performance cores, 4 efficiency cores and 32 GB of memory. |
| reference title | The one game that gave all measured numbers in these documents. |
| overlap | Two stages of `nxb build` run at the same time. The events stay the same as in a run in series. |
| Rust translator | The translator in the crates `nxb-xlate`, `nxb-maxwell` and `nxb-spvgen`. It is off by default since 0.5.1. `--xlate-rust on` or `NXB_XLATE_RUST=1` turns it on. |
| rewrite rules | The registered patches for code modules (cuts, observation gates, frame step stubs and others), kept as data in `data/rewrite-rules`. |
| reuse key | A digest of all inputs of a stage. If the key does not change, the stage uses its previous output. |
| ShaderInterface | The typed description of the bindings and the inputs and outputs of one translated shader. The Rust translator writes it as `<key>.iface.json`. |
| shader step | The stage that translates and compiles the shaders of a game into libraries. The command is `nxb shaders`. |
| survey | The examination of the files of a game that has no profile. It writes `survey.md` and the values of the provisional profile. Use `--survey-only` to stop after it. |
| stage | One part of the build: extract, rewrite, split, shader step, pipeline archive, pack, sign. |
| Unsupported | The result of a tool for an input that it does not cover. The exit code is 4, and the caller runs the reference step. |
| store | The content-addressed result store (`nxb-store`). |
| translator host | One process that loads the C# translator and does the work of many batches. |
| verdict | The result of the front-end check for one compile input: `own-identical`, `own-validated` or `apple`. |
| shader manifest | A text file with shader keys, one for each line. `--shader-manifest` builds these shaders first. |
| work folder | The folder with the intermediate files of one build. Its layout is compatible with the nxnative layout. |

## Technical verbs

These verbs are computer-process verbs (rule 1.12). The documents use them only with this definition.

| verb | definition |
|---|---|
| compile | Change source code into machine code or IR with a compiler. |
| decrypt | Change encrypted data into plain data. |
| extract | Copy the files out of a game package. |
| hash | Calculate a digest. |
| link | Join compiled modules into one file. |
| parse | Read a file format into a data structure. |
| run | Start a program and let it operate until it stops. |
| sign | Add a code signature to a file. |
| split | Find the shaders in the game files and write them into package records. |
| rewrite | Change the code of a game module so that it runs on the target. Also the name of the stage. |
| translate | Change machine code of the game GPU into SPIR-V and MSL. |

## Number labels

Each number has a label: **measured** (from a run on the reference machine, frequently with other programs on the CPU),
**derived** (a calculation from measured numbers) or **estimate** (nobody measured it).
