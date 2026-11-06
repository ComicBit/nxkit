# Shader step of a second title

**Date:** 2026-11-06.

**Question:** Why is the shader step of a second title much slower than that of the reference title?

The second title comes as a NUP package. Its RomFS is 26 GB. Its first build used a local build of commit `321a0ce`,
before 0.6.5. The progress showed about 42,000 shaders and about 45 minutes for the shader step. These two numbers are
read from the progress and not measured. The build ended early, during the shader step.

**Method:**

- Took one `sample` of 5 s of the running build, and counted its child processes every 0.25 s for 10 s.
- After the build was stopped, ran `airc rawcheck` (10 threads) over the 14,796 MSL sources in its shader cache. The
  cache holds only the shaders the build finished before it was stopped.
- Ran `airc bc` on every rejected source and grouped the first error.
- Timed `metal -c -O1 -fpreserve-invariance` without PCH on 6 rejected sources.
- Built the pipeline archive alone with `nxb pso` from the same cache, empty store, `--gpu g14s`, timed with
  `/usr/bin/time -l`. Three cargo builds ran at the same time, so the wall time is pessimistic.

## Result

### The own front end takes 69 % of the sources

| result of `airc rawcheck` | sources |
|---|---|
| library built | 10,276 (69 %) |
| rejected by the front end | 4,340 |
| front end accepts, raw container fails | 180 |
| total | 14,796 |

The build median is 38 ms and the mean 52 ms for each source (measured). All 14,796 sources took 79 s at 10 threads.
On the reference title 1,079 of 23,601 libraries went to Apple (4.6 %, measured with 0.6.4).

### One helper function causes half of the rejects

| first error | sources |
|---|---|
| `template:dm_pool_mark` | 2,282 |
| `reinterpret_cast` | 752 |
| `field-attr:front_facing` | 700 |
| `call:mulhi` | 298 |
| no error from `airc bc`, raw container fails | 180 |
| `call:simd_shuffle` | 163 |
| `lod:textureNd_array` | 109 |
| `call:simd_any` | 36 |

`dm_pool_mark` is a function template that the engine's translator writes into every shader with a descriptor pool. It
marks a pool entry as used with `reinterpret_cast` to a `device atomic_uint*` and an atomic `fetch_or`. Only the first
error of a source is counted. A source may hit a second gap after the first one is closed.

### Apple compiles a reject in about 1 s without PCH

The 6 timed compiles took 0.8 to 1.3 s each (measured). The build uses a PCH. With PCH a compile takes about 140 ms
(derived in [build-time-analysis.md](build-time-analysis.md)). The 4,520 rejects thus cost about 11 CPU minutes
(derived).

### The pipeline archive uses 2.5 cores on average

| measure | value |
|---|---|
| pairs, bases | 26,894 pairs, 6,215 bases |
| batches | 64, none from the store |
| archive size | 804 MB |
| wall time | 667 s |
| CPU user + system | 1,625 + 67 s |
| average cores | 2.5 |

### The build did not fill the machine

In the sample, 9 of the 12 archive threads waited in `Headroom::acquire`. The `nxb` process itself used about one
core. Its threads mostly waited for child processes. Over 10 s the child processes were `metal` (231 counts),
`applegpu-nt` (114), `air-lld` (15), `air-pack` (6) and `metallib` (4).

The CPU work of the 14,796 sources adds up to about 48 minutes: 9 for the own front end, 11 for the rejects and 28 for
the pipeline archive (derived). On 12 cores this is less than 10 minutes of wall time.

### The cache shows a steady rate

The file times in the shader cache show 14,796 sources written in 10 minutes, from 19:22 to 19:31. Each minute has
between 1,350 and 1,750 sources (measured from the file times). The pipeline archive wrote most of its files from
19:29. The rate does not show where the CPU time went.

### The extract reads and writes the same disk

The NUP and the work folder were on the same external USB SSD. The extract read the NUP and wrote `RomFS.img` at the
same time. It reached 5 to 10 MB/s (observed in the progress).

### The archive keeps three compiler runs while the CPU is busy

The archive always starts 3 compiler runs (`MIN_RUNS` in `nxb-build/src/pipeline.rs`). It starts more only while the
CPU and the memory stay below the load limit, and its runs have nice 10. While the shader step fills the CPU, the
archive thus stays at 3 runs. This agrees with the 9 waiting threads in the sample. In the standalone run, three cargo
builds kept the CPU busy. This agrees with the 2.5 cores. Read in the code on 2026-11-06.

Each batch runs through `metal-tt`. Every caller sets `direct: None`. The direct mode with `applegpu-nt -j N` and
`air-pack` exists in `nxb-archive`, but no caller uses it. Read in the code on 2026-11-06. Decision E6 in
[../decisions.md](../decisions.md) describes this.

## After the fixes (0.6.5)

0.6.5 closes the front end gaps of the reject table and fixes the raw container failures. `airc rawcheck` (10 threads,
with `air-validate`) over the same 14,796 sources gives this result (measured on 2026-11-06):

| result | sources |
|---|---|
| library built and passes `air-validate` | 14,780 |
| `sample-option:gradient2d` on `texture2d_array` | 8 |
| `call:simd_is_helper_thread` | 6 |
| `addrspace-value:thread` (a reference to an array as a parameter) | 2 |

All 10,276 libraries that the front end built before stay byte-identical. The pool libraries are not byte-identical to
Apple's libraries, because Apple's IR wraps arrays and atomics in struct types.

On the reference title Apple makes 35 libraries instead of 1,079 (standalone `nxb shaders build`, measured). The other
outputs are byte-identical to 0.6.4, except the libraries that Apple made before and 2 libraries with a fixed call
attribute.

The 16 rejects thus cost about 2 CPU seconds instead of about 11 CPU minutes (derived).

## Open questions

- How long does the full shader step of the second title take with 0.6.5? The CPU work does not explain the 45 minutes
  in the progress. The answer needs a full build with a timeline and the CPU load of each child process.
- How long does the archive take on a quiet machine, and how long after the shader step ends?
- Is the direct mode of E6 faster than `metal-tt` for this title?

## Decision

- Close the front end gaps in the table above, the raw container failures included. Every new library must pass
  `air-validate`. Libraries that the front end builds today stay
  byte-identical.
- Then build all shaders of the second title again and record a timeline.
- A host that reads the NUP directly removes the 26 GB `RomFS.img` and its write.
