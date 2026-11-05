//! Observability for every stage (layer L2): progress events as NDJSON and the sinks that receive them.
//!
//! One event is one line of compact JSON with `"ev"` first and a fixed field order per kind, so streams are stable to diff.
//! Strings are escaped like Python's `json.dumps(..., ensure_ascii=False)`: non-ASCII stays as UTF-8, control characters
//! become `\n`, `\t`, … or `\u00xx`. The front ends' existing stream (`stage`, `step`, `progress`, `stage_end`, `say`) is
//! mapped from these events in `nxb-stages`, its byte format goes through `nxb-pyjson` when `nxb build` replaces the Python
//! entry (workspace-layout 5.6).
//!
//! Later in this crate: logfmt log, Chrome trace, `_b_build_metrics.json`.

mod event;
mod json;
mod sink;

pub use event::{Event, Value};
pub use json::escape_into;
pub use sink::{MemorySink, NdjsonSink, NullSink, ProgressCounter, Sink};
