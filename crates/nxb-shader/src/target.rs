//! Compile conventions per target (design-v2 9.2): the flags of Apple's compiler and the record string that names
//! them in the reflection JSON and the Metal cache key. In V: a change here changes output identity.

use nxb_model::Target;

/// How shaders of one target are compiled.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TargetConv {
    /// The target.
    pub target: Target,
    /// `metal` flags before the source, `-c` included (as Python's `MAC_METAL` after `xcrun -sdk <sdk> metal`).
    pub compile_flags: &'static [&'static str],
}

/// The Mac convention: `metal -c -O1 -fpreserve-invariance`, then `metallib` (RM 3.3).
pub const MAC: TargetConv = TargetConv {
    target: Target::Mac,
    compile_flags: &["-c", "-O1", "-fpreserve-invariance"],
};

/// The iOS convention (`dm/ios/tools/ios_metal.py`): iOS 18, relaxed math (NaN and Inf kept), invariance, `-O1`.
pub const IOS: TargetConv = TargetConv {
    target: Target::Ios,
    compile_flags: &[
        "-c",
        "-O1",
        "-mios-version-min=18.0",
        "-fmetal-math-mode=relaxed",
        "-fpreserve-invariance",
    ],
};

impl TargetConv {
    /// The record string, byte-equal to Python's `MAC_RECORD` (`dmsh_build.py`): it is part of the reflection
    /// JSON (`chain_metal`) and of the Metal cache identity.
    pub fn record(&self) -> String {
        let sdk = self.target.sdk();
        format!(
            "xcrun -sdk {sdk} metal {}; xcrun -sdk {sdk} metallib",
            self.compile_flags.join(" ")
        )
    }

    /// The flags for building the precompiled header: the compile flags without `-c` (as `dmsh_pch.make`).
    pub fn pch_flags(&self) -> Vec<&'static str> {
        self.compile_flags
            .iter()
            .copied()
            .filter(|f| *f != "-c")
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_record_matches_python() {
        assert_eq!(
            MAC.record(),
            "xcrun -sdk macosx metal -c -O1 -fpreserve-invariance; xcrun -sdk macosx metallib"
        );
        assert_eq!(MAC.pch_flags(), ["-O1", "-fpreserve-invariance"]);
        assert_eq!(
            IOS.record(),
            "xcrun -sdk iphoneos metal -c -O1 -mios-version-min=18.0 -fmetal-math-mode=relaxed -fpreserve-invariance; xcrun -sdk iphoneos metallib"
        );
    }
}
