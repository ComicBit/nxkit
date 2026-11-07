//! What the core knows about a game before a family or title is chosen.

use nxb_model::TitleId;

/// The facts the core read from a game's files. The families decide from these.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TitleFacts {
    /// The title id.
    pub title_id: TitleId,
    /// The version (`v131072` is 131072), when known.
    pub version: Option<u32>,
    /// The SDK version string, when known (`22.2.8`).
    pub sdk: Option<String>,
    /// The module names (`main`, `subsdk0`, `sdk`).
    pub modules: Vec<String>,
    /// Names of libraries found in the modules (`sead`, `agl`). Empty when nobody looked.
    pub libraries: Vec<String>,
}

impl TitleFacts {
    /// Facts with the title id only.
    pub fn new(title_id: TitleId) -> TitleFacts {
        TitleFacts {
            title_id,
            version: None,
            sdk: None,
            modules: Vec::new(),
            libraries: Vec::new(),
        }
    }

    /// Whether the modules hold a library of this name.
    pub fn has_library(&self, name: &str) -> bool {
        self.libraries.iter().any(|l| l == name)
    }
}

/// The answer of a detection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Match {
    /// The game belongs to the family.
    Yes,
    /// It does not.
    No,
    /// The facts are not enough to say.
    Unknown,
}
