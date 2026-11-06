//! This Mac's GPU architecture as Metal names it (`mac_gpu_arch` of the NSP script): the default of `--gpu` for a Mac
//! app, so that a plain `nxb build` makes the pipeline archive the host opens at its first start.

use super::Runner;
use std::path::Path;

/// The helper program's source, as the script writes it.
const SOURCE: &str = "#import <Metal/Metal.h>\nint main(void) { @autoreleasepool { id<MTLDevice> d = MTLCreateSystemDefaultDevice(); if (!d) return 1; printf(\"%s\\n\", d.architecture.name.UTF8String); return 0; } }\n";

/// A name `MTLDevice.architecture` gives: `applegpu_g14s`.
fn valid(s: &str) -> bool {
    s.strip_prefix("applegpu_")
        .is_some_and(|r| !r.is_empty() && r.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
}

/// Asks Metal for the default device's architecture with a small program made in `<work>/tools` (made again when its
/// source changed).
pub fn mac_arch(work: &Path) -> Result<String, String> {
    let (src, exe) = (work.join("tools/mtl_arch.m"), work.join("tools/mtl_arch"));
    let fail = |why: String| {
        format!(
            "cannot tell this Mac's GPU architecture ({why}); --gpu names it, --gpu none builds the app without a pipeline archive"
        )
    };
    if !(exe.is_file() && std::fs::read_to_string(&src).is_ok_and(|t| t == SOURCE)) {
        std::fs::create_dir_all(work.join("tools")).map_err(|e| fail(e.to_string()))?;
        std::fs::write(&src, SOURCE).map_err(|e| fail(e.to_string()))?;
        let r = Runner::from_parent()?;
        r.run(
            "xcrun",
            [
                "clang".into(),
                "-fobjc-arc".into(),
                "-framework".into(),
                "Metal".into(),
                "-framework".into(),
                "Foundation".into(),
                src.clone().into_os_string(),
                "-o".into(),
                exe.clone().into_os_string(),
            ],
            None,
        )
        .map_err(fail)?;
    }
    let o = std::process::Command::new(&exe)
        .output()
        .map_err(|e| fail(e.to_string()))?;
    let arch = String::from_utf8_lossy(&o.stdout).trim().to_string();
    if !o.status.success() || !valid(&arch) {
        let why = String::from_utf8_lossy(&o.stderr).trim().to_string();
        return Err(fail(if why.is_empty() {
            format!("{arch:?}, exit {:?}", o.status.code())
        } else {
            why
        }));
    }
    Ok(arch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn architecture_names() {
        assert!(valid("applegpu_g14s"));
        assert!(!valid("applegpu_"));
        assert!(!valid("g14s"));
        assert!(!valid(""));
    }

    /// On a Mac with a Metal device the helper names its architecture, a machine without one says so (CI runners).
    #[test]
    fn this_mac() {
        let d = std::env::temp_dir().join(format!("nxb-gpu-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        match mac_arch(&d) {
            Ok(a) => assert!(valid(&a), "{a}"),
            Err(e) => assert!(e.contains("cannot tell"), "{e}"),
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}
