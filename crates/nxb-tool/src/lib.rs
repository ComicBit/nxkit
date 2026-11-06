//! External tools behind one interface (layer L2): the `Tool` trait, the process implementation, the child environment
//! policy, toolchain resolution, the circuit breaker and the translator contract.
//!
//! Rules (design-v2 9): argument vectors only, never a shell, every child starts from an **empty environment** plus an
//! exact allow-list, with `SDKROOT` per target for Apple tools, every child runs in its own process group with a timeout.
//! an interrupt or abort kills every live group through the [`ProcessRegistry`]. This crate knows no shaders.

mod apple;
mod breaker;
mod env;
mod frontend;
mod hostwire;
mod invocation;
mod process;
mod tool;
mod toolchain;
mod translator;
mod xlate_host;
mod xlate_process;
mod xlate_rust;

#[cfg(feature = "air-front")]
pub mod airfront;
pub use apple::{Io, MetalTools, StepCost};
pub use breaker::{BreakerConfig, CircuitBreaker};
pub use env::{EnvPolicy, EnvPolicyError, is_denied};
pub use frontend::{
    AppleFrontEnd, DenyList, FrontEnd, FrontEndError, FrontEndOutput, OwnFrontEnd, OwnToolPaths,
    WithFallback,
};
pub use invocation::{Completed, DEFAULT_MAX_STDERR, Invocation};
pub use process::{ProcessRegistry, kill_everything, run_process};
pub use tool::{ProcessTool, Tool, ToolId};
pub use toolchain::{MissingSdk, Resolver, SdkRoots};
pub use translator::{Translator, XlateFailure, XlateJob, XlateOutput, XlateResult};
pub use xlate_host::{HostConfig, HostTranslator};
pub use xlate_process::{ProcessTranslator, XlateConfig};
pub use xlate_rust::{ENV_SWITCH as XLATE_RUST_ENV, RustTranslator};
