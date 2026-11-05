//! The child environment: empty plus an exact allow-list (design-v2 9.1).

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::Path;

/// Names that are never passed to a child: they change compiler output or inject code.
const DENIED_EXACT: &[&str] = &[
    "DEVELOPER_DIR",
    "TOOLCHAINS",
    "CPATH",
    "C_INCLUDE_PATH",
    "CPLUS_INCLUDE_PATH",
    "OBJC_INCLUDE_PATH",
    "LIBRARY_PATH",
    "SDKROOT_OVERRIDE",
    "MACOSX_DEPLOYMENT_TARGET",
    "IPHONEOS_DEPLOYMENT_TARGET",
    "TVOS_DEPLOYMENT_TARGET",
    "XROS_DEPLOYMENT_TARGET",
];

/// Name prefixes that are never passed to a child.
const DENIED_PREFIX: &[&str] = &["MTL_", "METAL_", "CLANG_", "CCC_", "LLVM_", "DYLD_"];

/// A refused environment variable.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EnvPolicyError {
    /// The refused name.
    pub name: String,
}

impl fmt::Display for EnvPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "environment variable {} may not be passed to a tool",
            self.name
        )
    }
}

impl std::error::Error for EnvPolicyError {}

/// True when `name` may never reach a child.
pub fn is_denied(name: &str) -> bool {
    DENIED_EXACT.contains(&name) || DENIED_PREFIX.iter().any(|p| name.starts_with(p))
}

/// The exact environment of a child: it starts empty and holds only what was set. Sorted, so invocations are stable.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct EnvPolicy {
    vars: BTreeMap<OsString, OsString>,
}

impl fmt::Debug for EnvPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Names only: values may hold user paths.
        f.debug_set().entries(self.vars.keys()).finish()
    }
}

impl EnvPolicy {
    /// No variables at all.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Add a variable, denied names are refused.
    pub fn set(mut self, name: &str, value: impl Into<OsString>) -> Result<Self, EnvPolicyError> {
        if name.is_empty() || name.contains('=') || is_denied(name) {
            return Err(EnvPolicyError {
                name: name.to_string(),
            });
        }
        self.vars.insert(OsString::from(name), value.into());
        Ok(self)
    }

    /// Bypass of the deny list for toolchain resolution only (`DEVELOPER_DIR` is honoured once at resolution).
    pub(crate) fn set_for_resolution(mut self, name: &str, value: &OsStr) -> Self {
        self.vars.insert(OsString::from(name), value.to_os_string());
        self
    }

    /// Apple tools (`metal`, its back half, `metallib`): `PATH`, `TMPDIR`, `HOME` and the target's `SDKROOT`.
    /// Without `SDKROOT` a directly called `metal` writes different bytes (design-v2 9.1, measured by the reviewer).
    pub fn apple(sdkroot: &Path, tmpdir: &Path, home: &Path) -> Result<Self, EnvPolicyError> {
        EnvPolicy::empty()
            .set("PATH", "/usr/bin:/bin")?
            .set("TMPDIR", tmpdir.as_os_str())?
            .set("HOME", home.as_os_str())?
            .set("SDKROOT", sdkroot.as_os_str())
    }

    /// `spirv-cross`: `PATH` and `TMPDIR`.
    pub fn spirv_cross(tmpdir: &Path) -> Result<Self, EnvPolicyError> {
        EnvPolicy::empty()
            .set("PATH", "/usr/bin:/bin")?
            .set("TMPDIR", tmpdir.as_os_str())
    }

    /// The translator process bridge: `HOME`, `TMPDIR`, `PATH`, telemetry off, and the optional `DOTNET_ROOT`,
    /// `XLATE_DEBUG`, `XLATE_REDUCED` with today's values.
    pub fn translator(
        home: &Path,
        tmpdir: &Path,
        dotnet_root: Option<&OsStr>,
        xlate_debug: Option<&OsStr>,
        xlate_reduced: Option<&OsStr>,
    ) -> Result<Self, EnvPolicyError> {
        let mut p = EnvPolicy::empty()
            .set("PATH", "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin")?
            .set("HOME", home.as_os_str())?
            .set("TMPDIR", tmpdir.as_os_str())?
            .set("DOTNET_CLI_TELEMETRY_OPTOUT", "1")?;
        for (name, v) in [
            ("DOTNET_ROOT", dotnet_root),
            ("XLATE_DEBUG", xlate_debug),
            ("XLATE_REDUCED", xlate_reduced),
        ] {
            if let Some(v) = v {
                p = p.set(name, v)?;
            }
        }
        Ok(p)
    }

    /// The variables, sorted by name.
    pub fn vars(&self) -> impl Iterator<Item = (&OsStr, &OsStr)> {
        self.vars
            .iter()
            .map(|(k, v)| (k.as_os_str(), v.as_os_str()))
    }

    /// One variable's value.
    pub fn get(&self, name: &str) -> Option<&OsStr> {
        self.vars.get(OsStr::new(name)).map(OsString::as_os_str)
    }

    /// Number of variables.
    pub fn len(&self) -> usize {
        self.vars.len()
    }

    /// True when no variable is set.
    pub fn is_empty(&self) -> bool {
        self.vars.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deny_list() {
        for n in [
            "DEVELOPER_DIR",
            "TOOLCHAINS",
            "MTL_DEBUG",
            "METAL_DEVICE_WRAPPER_TYPE",
            "CLANG_X",
            "CCC_OVERRIDE_OPTIONS",
            "LLVM_PROFILE_FILE",
            "DYLD_INSERT_LIBRARIES",
            "CPATH",
            "MACOSX_DEPLOYMENT_TARGET",
        ] {
            assert!(EnvPolicy::empty().set(n, "x").is_err(), "{n}");
        }
        assert!(EnvPolicy::empty().set("A=B", "x").is_err());
        assert!(EnvPolicy::empty().set("", "x").is_err());
    }

    #[test]
    fn apple_has_exactly_four() {
        let p = EnvPolicy::apple(Path::new("/sdk"), Path::new("/t"), Path::new("/h")).unwrap();
        let names: Vec<_> = p
            .vars()
            .map(|(k, _)| k.to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["HOME", "PATH", "SDKROOT", "TMPDIR"]);
        assert_eq!(p.get("SDKROOT"), Some(OsStr::new("/sdk")));
    }

    #[test]
    fn translator_optional_vars() {
        let p = EnvPolicy::translator(
            Path::new("/h"),
            Path::new("/t"),
            None,
            Some(OsStr::new("1")),
            None,
        )
        .unwrap();
        assert_eq!(p.get("XLATE_DEBUG"), Some(OsStr::new("1")));
        assert_eq!(p.get("DOTNET_ROOT"), None);
        assert_eq!(p.get("DOTNET_CLI_TELEMETRY_OPTOUT"), Some(OsStr::new("1")));
    }

    #[test]
    fn debug_hides_values() {
        let p = EnvPolicy::empty().set("HOME", "/home/secret").unwrap();
        assert!(!format!("{p:?}").contains("secret"));
    }
}
