//! `decode_ue_shaders` (placeholder while the port lands).

use super::Ctx;

/// The Unreal decode of the split's versions.
pub(crate) fn decode_ue_shaders(_c: &Ctx<'_>, _versions: &[String]) -> Result<(), String> {
    Err("the Unreal shader decode is not ported yet".into())
}
