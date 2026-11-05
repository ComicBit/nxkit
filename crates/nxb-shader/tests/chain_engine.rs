//! Chain version V and inputs digest against an engine checkout and the values a reference build computed.
//! The test skips itself unless all four variables are set:
//! `NXB_PARITY_ENGINE` (engine folder), `NXB_PARITY_CHAIN` (expected V),
//! `NXB_PARITY_PROFILE` (game profile folder in the engine) and `NXB_PARITY_INPUTS` (expected inputs digest).

use std::path::PathBuf;

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

#[test]
fn chain_and_inputs_match_reference() {
    let (Some(engine), Some(chain), Some(profile), Some(inputs)) = (
        var("NXB_PARITY_ENGINE"),
        var("NXB_PARITY_CHAIN"),
        var("NXB_PARITY_PROFILE"),
        var("NXB_PARITY_INPUTS"),
    ) else {
        eprintln!(
            "skipped: NXB_PARITY_ENGINE, NXB_PARITY_CHAIN, NXB_PARITY_PROFILE, NXB_PARITY_INPUTS are not set"
        );
        return;
    };
    let engine = PathBuf::from(engine);
    assert_eq!(
        nxb_shader::chain::version(&engine).as_deref(),
        Ok(chain.as_str())
    );
    assert_eq!(
        nxb_shader::chain::inputs(&PathBuf::from(profile)).as_deref(),
        Ok(inputs.as_str())
    );
}
