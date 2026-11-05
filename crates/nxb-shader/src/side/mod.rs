//! The pixel shaders' side entry points (port of `dmsh_side.py`, `dmsh_alpha.py`, `dmsh_logic.py` of the engine).
//!
//! One compile unit holds each fragment entry point's body once as a static function, with thin entry points around it:
//! the plain ones (`main0`, `main0_u`, …), the alpha test ones (`<entry>_a`) and the logic operation ones (`<entry>_l`).
//! The compiled library is then cut in two ([`cut`]): `<key>_b` with the plain entry points, `<key>_bx` with the others.
//! Byte identity with the Python port is the rule (design-v2 2.2): the text matchers below reproduce the Python regular
//! expressions exactly for the shapes they meet (documented per function), without a regex engine.

mod alpha;
mod cut;
mod logic;
mod text;
mod unit;

pub use cut::{CutError, cut};
pub(crate) use text::strip as strip_py;
pub use unit::{Unit, unit};

/// Fragment buffer index of the alpha test state (`dmsh_abi.ALPHA_BUFFER`).
pub const ALPHA_BUFFER: u32 = 30;
/// Fragment buffer index of the logic operation state (`dmsh_abi.LOGIC_BUFFER`).
pub const LOGIC_BUFFER: u32 = 29;
/// First function constant index of the logic operation's attachment reads (`dmsh_abi.LOGIC_FC`).
pub const LOGIC_FC: u32 = 200;
