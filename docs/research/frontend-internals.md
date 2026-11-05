# Front-end internals of the Metal toolchain

**Date:** 2026-11-05.

**Question:** What does Apple's Metal toolchain (32023.922) do between MSL and the library? What must an own front end
write, and which parts can stay with Apple?

**Method:**

- Compiled small MSL samples for each construct family: vertex, fragment and kernel entries, function constants, texture
  arrays, an argument buffer, amplification, post-tessellation, mesh, tile, visible, stitchable and intersection functions.
- Recorded the pass sequence with `-mllvm -print-after-all`.
- Read the IR text with `air-opt -S`, and compared 75 reference `.ll` files from clang.
- Parsed the library container and rebuilt libraries from their parts.
- Called the tools only. The study copied no Apple code.

## Result

**Pass pipeline of `metal -c`.** It is a reduced O1 pipeline. It has no vectorizer, no loop unroll and no LICM.
It contains 13 AIR passes. The last block (finalization) also runs at `-O0` and for IR input: function-predicate
resolution, removal of disabled inputs, marks for unused arguments, metadata fixup, emulation of resource arrays,
metadata pruning and a call-graph check. iOS runs the same 13 AIR passes.

**`metallib`.** `metallib` is the same program as `air-lld`. It divides the module into one module for each entry point
and runs its own pass list on each part, with InstCombine. Thus its output is not only a container.

**Container format (`MTLB`).** A 0x58-byte header gives the platform, the versions, the file size and the offsets of four
sections: function list, public metadata, private metadata, bitcode. Each record is a tag stream that ends with `ENDT`.
The `HASH` tag is the SHA-256 of the bitcode blob of that function. A library that the tool rebuilt from its parts was
byte-identical on all samples. `metallib` output is deterministic, thus the tests compare libraries, not `.air` files.

**Metadata.** The required metadata is in [../air-conventions.md](../air-conventions.md). `air-verify` accepted a module
without `air.arg_type_name` on an output, thus the minimum set of metadata is not known.

**Open questions:**

1. Does Apple's reader accept bitcode from an upstream LLVM 14 writer? (Apple's reader accepts the output of the own `air-bc` writer.)
2. Which byte range does the library UUID hash? Only the one-step driver writes a UUID. `metallib` omits it.
3. When does the resource-array emulation pass change IR? It made no change on macOS AIR 2.9.
4. What is the accurate format of generated varying names for structs, arrays and half types?
5. Which IR flags change with the fast-math option?

## Decision

- Keep Apple's `air-opt` and `metallib` as the back half. The own code writes bitcode only.
- Do not write an own `metallib`. It must copy the per-function pass list with InstCombine and the LLVM bitcode writer accurately.
- Learn the metadata of each construct family from clang output, not by a guess.
- Use the container parser to read libraries for verdicts and tags.
