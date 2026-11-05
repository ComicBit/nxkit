# Documentation index

`nxbuild` is a Rust workspace that changes Switch game packages into Apple builds. It extracts the game package, splits
the shaders, rewrites the code, translates and compiles the shaders into Metal libraries, makes the pipeline archive, and packs and signs the app.
The output files are byte-identical to the output of the reference build scripts. The own shader front end is the one
exception. Its raw path is on by default since 0.4.0, and GPU tests cover only a part of the shaders.

## Reading order

| # | document | content |
|---|---|---|
| 1 | [glossary.md](glossary.md) | the terms and the number labels |
| 2 | [architecture.md](architecture.md) | layers, crates, data flow of the stages |
| 3 | [shader-step.md](shader-step.md) | the shader step: versions, execution, store, errors |
| 4 | [pipeline-stages.md](pipeline-stages.md) | extract, split, rewrite, NRO collection, pack, sign, iOS stages, events, cache export, pipeline archive |
| 5 | [frontend.md](frontend.md) | the own MSL to AIR front end and its results |
| 6 | [air-conventions.md](air-conventions.md) | the AIR conventions that a front end must obey |
| 7 | [gpu-differential-testing.md](gpu-differential-testing.md) | the `gpudiff` harness |
| 8 | [translator.md](translator.md) | the shader translator, the plan to replace it and the state of the Rust port |
| 9 | [early-cutoff.md](early-cutoff.md) | the key scheme of each stage and the cost of a chain change |
| 10 | [benchmarks.md](benchmarks.md) | measured and estimated timings |
| 11 | [decisions.md](decisions.md) | architecture decisions W1 to W22 and design decisions |
| 12 | [contributing-architecture.md](contributing-architecture.md) | rules for contributors |
| 13 | [research/](research/README.md) | short notes on the studies behind the design |

## Data

The documents give results for one reference title on the reference machine. The game files, keys, the shader corpus and
the run outputs are not in this repository. They are not necessary to compile the workspace. To do the parity tests on
your own files, refer to [contributing-architecture.md](contributing-architecture.md).
