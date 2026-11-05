# Reference corpus

**Date:** 2026-11-05.

**Question:** Which compile paths from MSL to a library give the same bytes as `metal -c -O1` and `metallib`?
How much of the compile time is the front end?

**Method:**

- Compiled all 23,785 distinct compile inputs of the reference title with `metal -c -O1` and `metallib` (Mac convention). This is the reference.
- Compiled the same inputs through four alternative paths and compared the libraries byte for byte:
  1. p1: clang front end only, then a normalized IR comparison.
  2. p2: clang front end, then `air-opt -O1`, then `metallib`.
  3. p2b: as p2, plus `air-opt --discard-value-names`.
  4. p3: clang IR text with `-S`, then `metal -x ir`, then `metallib`.
- Classified each byte difference by cause.
- Timed the parts of `metal -c` on 2,000 inputs (14 entry kinds, 12 parallel processes), with and without PCH.

## Result

| path | libraries with the same bytes |
|---|---|
| p1, normalized IR | 23,784 of 23,785 (100.00 %) |
| p2, front end and `air-opt -O1` | 2 of 23,785 (0.01 %) |
| p2b, as p2 with `--discard-value-names` | 23,784 of 23,785 (100.00 %) |
| p3, IR text and `metal -x ir` | 23,785 of 23,785 (100.00 %) |

- In p2, 99.9 % of the differences come from the names of basic blocks. `--discard-value-names` removes them.
- The one remaining difference in p2b comes from value names.
- The reference compiled all 23,785 inputs without a failure.

Time of `metal -c` for each input, as one process sees it (CPU contention included):

| part | without PCH, mean | with PCH, mean |
|---|---|---|
| `metal -c` total | 0.937 s | 0.457 s |
| front end only | 0.909 s | 0.394 s |
| `air-opt -O1` | 0.082 s | 0.096 s |
| `metallib` | 0.057 s | 0.051 s |

The full sample took 335 s without PCH and 168 s with PCH (0.167 s and 0.084 s throughput for each input).
Large pixel shaders take the most time (1.5 to 1.6 s mean without PCH). Vertex shaders take 0.1 to 0.3 s.

## Decision

- The clang front end is approximately 86 % to 97 % of the `metal -c` time (derived). Thus the own front end is the largest lever on the compile time.
- The back half for the own front end is `air-opt -O1`, then `--discard-value-names`, then `metallib`. With clang's IR, this path is byte-identical on 23,784 of 23,785 inputs.
- An own front end can give byte-identical libraries only if its IR is equal to clang's IR. Measure identity on the library.
- Keep the stored reference libraries for the verdicts of the own front end (refer to [../frontend.md](../frontend.md)).
