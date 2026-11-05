//! Variant sources made from a shader's final MSL (ports of `dmsh_pull.py`, `dmsh_vin.py`, `dmsh_fetch.py`):
//! `<key>_bp` (pulling vertex inputs), `<key>_bv` (shadow integer inputs), `<key>_bf` (framebuffer-fetch reads).
//! Each returns the variant's source and its reflection metadata, or the reason there is none, as the Python does.
//! Text matching reproduces the Python regular expressions for the shapes they meet, without a regex engine.

mod common;
pub mod fetch;
pub mod pull;
pub mod vin;

pub use common::StageIn;
