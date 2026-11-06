# Research notes

Each note gives one study: date, question, method, result and decision. The notes contain facts only, no code and no
tool listings. All studies used the reference machine. All but one used the reference title. They used only local files.

| note | question |
|---|---|
| [build-time-analysis.md](build-time-analysis.md) | Where does the time of a build go, and where are the lower limits? |
| [reference-corpus.md](reference-corpus.md) | Which compile paths give libraries that are byte-identical to `metal -c`? |
| [frontend-internals.md](frontend-internals.md) | What does Apple's Metal toolchain do between MSL and the library? |
| [backend-survey.md](backend-survey.md) | Can the pipeline archive stage be faster or replaced? |
| [second-title-shader-step.md](second-title-shader-step.md) | Why is the shader step of a second title much slower? |

The measurements of chain changes are in [../early-cutoff.md](../early-cutoff.md).
