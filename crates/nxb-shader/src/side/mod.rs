//! The side library of a pixel shader: its alpha test and logic operation entry points are cut out of the compiled
//! library into `<key>_bx`, the rest stays in `<key>_b`.

mod cut;

pub use cut::{CutError, cut};
