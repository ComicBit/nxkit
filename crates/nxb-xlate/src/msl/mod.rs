//! The MSL back end: the structured program to Metal Shading Language, with the typed reflection of each library.
//!
//! This replaces the SPIR-V generator, `spirv-cross` and the MSL text passes. The output keeps the contract of the
//! engine host: the argument buffer at `[[buffer(0)]]` with `[[id(n)]]` members, the entry `main0` and the extra
//! entries, the library files `<key>_b`, `_bf`, `_bp`, `_bv`, `_bx`, `_bg*`, `_bl*`, `_bgv`, and `_b.json` as
//! [`refl::Reflection`] writes it. Refer to `docs/translator.md`.

pub mod abi;
pub mod refl;

use crate::job::Job;
pub use refl::Reflection;

/// What the stage step knows about one shader besides its job and code.
#[derive(Clone, Debug, Default)]
pub struct Context<'a> {
    /// The chain version V.
    pub chain: &'a str,
    /// `<package>#<index>`.
    pub source: String,
    /// The paired shader of the game's program.
    pub pc_paired: &'a str,
    /// Vertex shaders: the inputs of the paired pixel shaders (`vs_pad`).
    pub partners: &'a [StageInputs],
    /// The key is not in the answer table, so the queries use defaults.
    pub no_facts: bool,
}

/// One user varying a pixel shader reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Varying {
    /// The location.
    pub location: u32,
    /// The MSL type (`float4`, `int2`, ...).
    pub msl_type: String,
    /// The interpolation qualifier (`""`, `flat`, `center_no_perspective`, ...).
    pub interpolation: String,
}

/// The user varyings a pixel shader reads, in location order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StageInputs {
    /// The varyings.
    pub varyings: Vec<Varying>,
}

/// The entries of a pixel shader that the side library (`_bx`) takes: the alpha test and the logic op functions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Side {
    /// Alpha test entries.
    pub alpha: Vec<String>,
    /// Logic op entries.
    pub logic: Vec<String>,
}

/// One library of a shader: one `.metal` source, compiled to one `.metallib`.
#[derive(Clone, Debug, PartialEq)]
pub struct Library {
    /// The file suffix: `_b`, `_bf`, `_bp`, `_bv`, `_bg0`, `_bgv`, `_bl0`, ...
    pub suffix: String,
    /// The MSL source.
    pub msl: String,
    /// The library's own JSON file. `None` when the library has none (its facts are a field of `_b.json`).
    pub reflection: Option<Reflection>,
}

/// The translation of one shader.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Translated {
    /// The shader key.
    pub key: String,
    /// `vs`, `ps`, `cs`, `gs`, `ts`, `ms`.
    pub stage: String,
    /// The libraries. The first one is `_b`.
    pub libraries: Vec<Library>,
    /// Pixel shaders: what the shader reads, for its paired vertex shaders.
    pub inputs: StageInputs,
    /// Pixel shaders: the side library units, when the shader has any.
    pub side: Option<Side>,
}

/// Why a shader has no translation. `reason` is one of the fixed reason strings of `docs/shader-step.md`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    /// The reason string (`translation failed`, `not ported`, `MSL failed`, ...).
    pub reason: String,
    /// The detail.
    pub detail: String,
}

impl Failure {
    /// A failure with a reason and a detail.
    pub fn new(reason: &str, detail: impl Into<String>) -> Failure {
        Failure {
            reason: reason.to_string(),
            detail: detail.into(),
        }
    }
}

/// Translate one shader from its job and machine code to its libraries.
pub fn translate(job: &Job, code: &[u8], ctx: &Context<'_>) -> Result<Translated, Failure> {
    let _ = (code, ctx);
    Err(Failure::new(
        "not ported",
        format!("unported: MSL back end ({})", job.stage),
    ))
}
