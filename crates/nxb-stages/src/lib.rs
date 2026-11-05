//! Stage wiring: one module per stage, each building its part of the task graph from the domain crates and the runtime.
//!
//! A stage module stays under 400 lines.
//!
//! Status: `recompile` (the P1a milestone) is implemented, the other modules are placeholders that fix the crate's shape, see
//! `docs/workspace-layout.md` for the planned public surface.

/// The shader step (design-v2).
pub mod shaders {}

pub mod build;
mod build_finish;
mod build_hooks;
mod build_partial;
mod build_shared;
mod build_xlate;
pub mod front;
pub mod guard_tables;
pub mod inputs;
pub mod own_front;
pub mod own_raw;
pub mod recompile;
mod recompile_build;
pub mod recompile_summary;
mod recompile_variants;
pub mod vertex;
pub mod xlate_cache;
pub mod xlatehost;

/// The extract stage.
pub mod extract {}

/// The module rewrite stage.
pub mod rewrite {}

/// The shader split stage.
pub mod split {}

/// The Mac app stage.
pub mod app {}

/// The IPA stage.
pub mod ipa {}

/// The pipeline archive stage.
pub mod pso {}

/// The whole-build graph: `build(&BuildConfig)`.
pub mod pipeline {}
