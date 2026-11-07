//! The engine family of Nintendo EPD games on sead, agl and gsys.
//!
//! What is here was seen in one game, The Legend of Zelda: Ocarina of Time for Nintendo Switch 2
//! (`0400486024DEC000`), and is believed to hold for the whole engine. A second game of the family confirms each
//! fact or moves it into its title module.

use nxtitle::{
    BuildHooks, Evidence, Fact, Family, Match, Need, RuntimeHooks, ShaderRole, ShaderView,
    TitleFacts,
};

/// The family.
pub struct Agl;

/// The one instance the registry lists.
pub static AGL: Agl = Agl;

const OOT_README: Evidence = Evidence::Recorded {
    source: "seedds/nxnative dm/games/0400486024DEC000/README.md at c8bfa161",
    read: "2026-11-07",
};

const NEEDS: &[Fact<Need>] = &[
    Fact::new(
        Need {
            capability: "render_cond",
            setting: "on",
            why: "agl clears a copy of the prepass depth again under a render condition the console skips. Run \
                  unconditionally, the opaque pass's EQUAL depth test fails everywhere.",
        },
        OOT_README,
    ),
    Fact::new(
        Need {
            capability: "tex_flush_upload",
            setting: "on",
            why: "the CPU writes terrain height, blend and hole maps into textures after creating them. Without \
                  the upload they stay zero: blue holes in the ground. Also seen in Tears of the Kingdom.",
        },
        OOT_README,
    ),
    Fact::new(
        Need {
            capability: "tex_dirty_upload",
            setting: "on",
            why: "the terrain maps are rewritten from the CPU as tile slots are reused. Also seen in Tears of the \
                  Kingdom.",
        },
        OOT_README,
    ),
];

impl BuildHooks for Agl {
    fn shader_role(&self, shader: &ShaderView<'_>) -> Option<Fact<ShaderRole>> {
        shader.archive.filter(|a| a.contains("agl_ndsr")).map(|_| {
            Fact::new(
                ShaderRole::Upscaler {
                    name: "agl_ndsr_vr",
                },
                Evidence::Unverified {
                    note: "the upscaler networks are named agl_ndsr_vr in the nxnative OoT README. That their \
                           archive path carries this name is not checked.",
                },
            )
        })
    }
}

impl RuntimeHooks for Agl {
    fn needs(&self) -> &'static [Fact<Need>] {
        NEEDS
    }
}

impl Family for Agl {
    fn id(&self) -> &'static str {
        "agl"
    }

    fn name(&self) -> &'static str {
        "Nintendo EPD engine (sead, agl, gsys)"
    }

    fn detect(&self, facts: &TitleFacts) -> Match {
        if facts.libraries.is_empty() {
            Match::Unknown
        } else if facts.has_library("sead") && facts.has_library("agl") {
            Match::Yes
        } else {
            Match::No
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nxtitle::ShaderStage;

    fn facts(libs: &[&str]) -> TitleFacts {
        let mut f = TitleFacts::new(nxtitle::TitleId(0));
        f.libraries = libs.iter().map(|s| s.to_string()).collect();
        f
    }

    #[test]
    fn detects_from_libraries() {
        assert_eq!(AGL.detect(&facts(&[])), Match::Unknown);
        assert_eq!(AGL.detect(&facts(&["sead", "agl", "gsys"])), Match::Yes);
        assert_eq!(AGL.detect(&facts(&["UnrealEngine"])), Match::No);
    }

    #[test]
    fn ndsr_is_an_upscaler_and_unverified() {
        let s = ShaderView {
            key: "0000000000000000",
            stage: ShaderStage::Compute,
            archive: Some("Shader/agl_ndsr_vr.bnsh"),
        };
        let r = AGL.shader_role(&s);
        assert!(matches!(
            r.map(|f| f.value),
            Some(ShaderRole::Upscaler { .. })
        ));
        assert!(r.is_some_and(|f| !f.evidence.is_checked()));
        assert!(AGL.needs().iter().all(|n| n.evidence.is_checked()));
    }
}
