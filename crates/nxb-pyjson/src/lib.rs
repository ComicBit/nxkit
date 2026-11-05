//! Python `json` compatibility (layer L1, in V): an ordered JSON value, a parser, and a writer byte-equal to
//! `json.dumps(value, ensure_ascii=False[, indent=N])`, including Python's float `repr`.
//!
//! Every JSON file the shader chain writes (`_b.json` with `indent=1`, the reflection fields, metadata) goes through
//! [`dumps`], so the port writes the same bytes as the Python chain (design-v2 12).

mod float;
mod parse;
mod value;
mod write;

pub use float::repr;
pub use parse::{ParseError, parse};
pub use value::Value;
pub use write::{dumps, dumps_indent};
