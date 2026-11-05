//! Typed ids shared by every stage.

use crate::digest::{ParseError, from_hex_into, to_hex};
use std::fmt;

/// A shader key: 8 bytes shown as exactly 16 lower-case hex characters (`sha256(control[:0x880] ‖ code)[:16]` today).
///
/// Output file names are made only from validated keys (design-v2 18.3), so parsing is strict.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShaderKey([u8; 8]);

impl ShaderKey {
    /// From raw bytes.
    pub fn from_bytes(b: [u8; 8]) -> ShaderKey {
        ShaderKey(b)
    }

    /// Parse `[0-9a-f]{16}`.
    pub fn parse(s: &str) -> Result<ShaderKey, ParseError> {
        let mut b = [0u8; 8];
        if from_hex_into(s, &mut b) {
            Ok(ShaderKey(b))
        } else {
            Err(ParseError::new("16 lower-case hex characters", s))
        }
    }

    /// The raw bytes.
    pub fn bytes(&self) -> [u8; 8] {
        self.0
    }
}

impl fmt::Display for ShaderKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&to_hex(&self.0))
    }
}

impl fmt::Debug for ShaderKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ShaderKey({self})")
    }
}

/// A Switch title id (shown as 16 upper-case hex characters, as the tools print it).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct TitleId(pub u64);

impl fmt::Display for TitleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016X}", self.0)
    }
}

/// A title version number (`v0`, `v327680`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct VersionTag(pub u32);

impl fmt::Display for VersionTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}", self.0)
    }
}

/// A build target. The full compile convention per target (`TargetConv`) lives in `nxb-shader`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum Target {
    /// macOS, SDK `macosx`.
    Mac,
    /// iOS, SDK `iphoneos`.
    Ios,
    /// tvOS, SDK `appletvos` (reserved).
    TvOs,
}

impl Target {
    /// The `xcrun -sdk` name.
    pub fn sdk(self) -> &'static str {
        match self {
            Target::Mac => "macosx",
            Target::Ios => "iphoneos",
            Target::TvOs => "appletvos",
        }
    }

    /// Short lower-case name used in keys and paths.
    pub fn name(self) -> &'static str {
        match self {
            Target::Mac => "mac",
            Target::Ios => "ios",
            Target::TvOs => "tvos",
        }
    }

    /// Every target, in a fixed order.
    pub const ALL: [Target; 3] = [Target::Mac, Target::Ios, Target::TvOs];
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A stage of the whole build (design-pipeline-rest section 6).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum StageId {
    /// Package to images.
    Extract,
    /// Module rewrite.
    Rewrite,
    /// Shader split.
    Split,
    /// The shader step.
    Shaders,
    /// Pipeline archive.
    Archive,
    /// Mac app pack and sign.
    App,
    /// IPA.
    Ipa,
}

impl StageId {
    /// Stable lower-case name (events, logs, metrics).
    pub fn name(self) -> &'static str {
        match self {
            StageId::Extract => "extract",
            StageId::Rewrite => "rewrite",
            StageId::Split => "split",
            StageId::Shaders => "shaders",
            StageId::Archive => "archive",
            StageId::App => "app",
            StageId::Ipa => "ipa",
        }
    }
}

impl fmt::Display for StageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nxb_testkit::check;

    #[test]
    fn shader_key_round_trip() {
        check(31, 200, |r| {
            let mut b = [0u8; 8];
            b.copy_from_slice(&r.bytes(8));
            let k = ShaderKey::from_bytes(b);
            assert_eq!(ShaderKey::parse(&k.to_string()), Ok(k));
        });
    }

    #[test]
    fn shader_key_strict() {
        for bad in [
            "",
            "0123",
            "0123456789ABCDEF",
            "0123456789abcdeg",
            "../../etc/passwd",
            "0123456789abcdef0",
        ] {
            assert!(ShaderKey::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn names() {
        assert_eq!(
            TitleId(0x0100_0000_0000_1234).to_string(),
            "0100000000001234"
        );
        assert_eq!(VersionTag(327680).to_string(), "v327680");
        assert_eq!(Target::Ios.sdk(), "iphoneos");
        assert_eq!(StageId::Shaders.to_string(), "shaders");
    }
}
