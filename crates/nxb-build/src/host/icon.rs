//! The app icon: `pack_app.py`'s `make_icns`. The game's icon (the control data's 256x256 `icon.jpg`) scaled by `sips`
//! to every size an `.icns` holds, packed by `iconutil`. Both tools write the same bytes on every run, so the result is
//! byte-identical to the reference script's `Contents/Resources/AppIcon.icns`.

use super::{Runner, TempDir};
use std::path::Path;

/// The icon sizes (points), each is made at 1x and 2x.
pub const ICON_SIZES: [u32; 5] = [16, 32, 128, 256, 512];

/// Make `out` (an `.icns`) from `jpg`. The work folder is a temporary folder beside `out`.
pub fn make_icns(r: &Runner, jpg: &Path, out: &Path) -> Result<(), String> {
    let parent = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let t = TempDir::new_in(parent)?;
    let set = t.path().join("AppIcon.iconset");
    std::fs::create_dir(&set).map_err(|e| format!("{}: {e}", set.display()))?;
    for n in ICON_SIZES {
        for (k, suffix) in [(1, ""), (2, "@2x")] {
            let px = (n * k).to_string();
            let png = set.join(format!("icon_{n}x{n}{suffix}.png"));
            r.run(
                "sips",
                [
                    "-s".as_ref(),
                    "format".as_ref(),
                    "png".as_ref(),
                    "-z".as_ref(),
                    px.as_ref(),
                    px.as_ref(),
                    jpg.as_os_str(),
                    "--out".as_ref(),
                    png.as_os_str(),
                ],
                None,
            )?;
        }
    }
    r.run(
        "iconutil",
        [
            "-c".as_ref(),
            "icns".as_ref(),
            set.as_os_str(),
            "-o".as_ref(),
            out.as_os_str(),
        ],
        None,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    /// A 24-bit BMP of `w` x `h` pixels (a colour gradient), which `sips` reads like a JPEG.
    fn bmp(w: u32, h: u32) -> Vec<u8> {
        let row = (w * 3).div_ceil(4) * 4;
        let size = 54 + row * h;
        let mut b = Vec::new();
        b.extend_from_slice(b"BM");
        for v in [size, 0, 54, 40, w, h] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&24u16.to_le_bytes());
        for v in [0u32, row * h, 2835, 2835, 0, 0] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for y in 0..h {
            for x in 0..w {
                b.extend_from_slice(&[(x * 255 / w) as u8, (y * 255 / h) as u8, 128]);
            }
            b.resize(b.len() + (row - w * 3) as usize, 0);
        }
        b
    }

    fn runner() -> Option<Runner> {
        let r = Runner::from_parent().ok()?;
        (r.which("sips").is_ok() && r.which("iconutil").is_ok()).then_some(r)
    }

    #[test]
    fn icns_from_a_synthetic_icon() {
        let Some(r) = runner() else { return };
        let d = std::env::temp_dir().join(format!("nxb-icon-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let src = d.join("icon.bmp");
        std::fs::write(&src, bmp(64, 64)).unwrap();
        let (a, b) = (d.join("a.icns"), d.join("b.icns"));
        make_icns(&r, &src, &a).unwrap();
        make_icns(&r, &src, &b).unwrap();
        let bytes = std::fs::read(&a).unwrap();
        assert_eq!(&bytes[..4], b"icns");
        assert_eq!(
            u32::from_be_bytes(bytes[4..8].try_into().unwrap()) as usize,
            bytes.len()
        );
        assert_eq!(bytes, std::fs::read(&b).unwrap(), "two runs, same bytes");
        // only the icon is left beside it
        let names: Vec<_> = std::fs::read_dir(&d)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 3, "{names:?}");
        assert!(make_icns(&r, &d.join("missing.jpg"), &d.join("c.icns")).is_err());
        assert!(!names.iter().any(|n| n == OsStr::new("c.icns")));
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// Oracle: `NXB_ORACLE_ICON_JPG` (a game's `icon.jpg`) and `NXB_ORACLE_ICNS` (the `AppIcon.icns` the reference
    /// script made from it) give the same bytes.
    #[test]
    fn icns_equals_the_reference_script() {
        let (Some(jpg), Some(want)) = (
            std::env::var_os("NXB_ORACLE_ICON_JPG"),
            std::env::var_os("NXB_ORACLE_ICNS"),
        ) else {
            return;
        };
        let Some(r) = runner() else { return };
        let d = std::env::temp_dir().join(format!("nxb-icon-oracle-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let out = d.join("AppIcon.icns");
        make_icns(&r, Path::new(&jpg), &out).unwrap();
        assert!(std::fs::read(&out).unwrap() == std::fs::read(&want).unwrap());
        std::fs::remove_dir_all(&d).unwrap();
    }
}
