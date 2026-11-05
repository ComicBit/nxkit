//! `nacp.py`: the game name and display version of the control data (`control.nacp`). The supported languages are
//! `host::settings::languages` (the script's third mode).

use std::path::Path;

/// `nacp.name`: the first non-empty title of the (maybe deflated) title area, `""` when none is sensible.
pub fn name(path: &Path) -> String {
    let Ok(b) = std::fs::read(path) else {
        return String::new();
    };
    let mut t: Vec<u8> = b[..b.len().min(0x3000)].to_vec();
    if b.len() >= 2 {
        // newer SDKs deflate the title area: a u16 size, raw deflate, 32 languages of 0x300 bytes
        let n = u16::from_le_bytes([b[0], b[1]]) as usize;
        let z = &b[2..(2 + n).min(b.len())];
        if let Some(u) = nxb_sys::zlib::inflate_bytes(z, -15, 0x6000).filter(|u| u.len() == 0x6000)
        {
            t = u;
        }
    }
    for i in 0..t.len() / 0x300 {
        let s = &t[i * 0x300..i * 0x300 + 0x200];
        let s = s.split(|&c| c == 0).next().unwrap_or(&[]);
        if s.is_empty() {
            continue;
        }
        return match std::str::from_utf8(s) {
            Ok(n) if n.chars().all(printable) => n.to_string(),
            _ => String::new(),
        };
    }
    String::new()
}

/// `str.isprintable` for one character, without the Unicode tables: controls, separators other than the space, the
/// format characters of the common blocks, private use and noncharacters are not printable (unassigned code points are
/// taken as printable: the one known difference).
fn printable(c: char) -> bool {
    if c == ' ' {
        return true;
    }
    let u = c as u32;
    !(c.is_control()
        || c.is_whitespace()
        || matches!(u, 0xAD | 0x600..=0x605 | 0x61C | 0x6DD | 0x70F | 0x180E | 0x200B..=0x200F | 0x202A..=0x202E
            | 0x2060..=0x206F | 0xFEFF | 0xFFF9..=0xFFFB | 0xE000..=0xF8FF | 0xFDD0..=0xFDEF | 0xF0000..=0x10FFFF)
        || u & 0xFFFE == 0xFFFE)
}

/// `nacp.version`: the display version (0x10 bytes at 0x3060) cut to up to three dot-separated integers, `""` when it
/// does not start with a number.
pub fn version(path: &Path) -> String {
    let Ok(b) = std::fs::read(path) else {
        return String::new();
    };
    let s = b.get(0x3060..0x3070.min(b.len())).unwrap_or(&[]);
    let digits = |from: usize| s[from..].iter().take_while(|c| c.is_ascii_digit()).count();
    let mut end = digits(0);
    if end == 0 {
        return String::new();
    }
    for _ in 0..2 {
        if s.get(end) == Some(&b'.') && digits(end + 1) > 0 {
            end += 1 + digits(end + 1);
        } else {
            break;
        }
    }
    String::from_utf8_lossy(&s[..end]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nacp(d: &mut Vec<u8>, name: &str, ver: &str) {
        d.clear();
        d.resize(0x4000, 0);
        d[..name.len()].copy_from_slice(name.as_bytes());
        d[0x3060..0x3060 + ver.len()].copy_from_slice(ver.as_bytes());
    }

    #[test]
    fn plain_and_deflated_titles_and_versions() {
        let d = std::env::temp_dir().join(format!("nxb-ios-nacpname-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let mut b = vec![];
        nacp(&mut b, "Sample", "1.2.1.9");
        std::fs::write(d.join("a"), &b).unwrap();
        assert_eq!(name(&d.join("a")), "Sample");
        assert_eq!(version(&d.join("a")), "1.2.1");
        nacp(&mut b, "", "x1");
        std::fs::write(d.join("b"), &b).unwrap();
        assert_eq!(name(&d.join("b")), "");
        assert_eq!(version(&d.join("b")), "");
        // deflated: 32 titles, the third one set
        let mut t = vec![0u8; 0x6000];
        t[2 * 0x300..2 * 0x300 + 3].copy_from_slice("ゲ".as_bytes());
        let z = nxb_sys::zlib::deflate_bytes(&t, 6, -15).unwrap();
        let mut c = (z.len() as u16).to_le_bytes().to_vec();
        c.extend(z);
        c.resize(0x4000, 0);
        std::fs::write(d.join("c"), &c).unwrap();
        assert_eq!(name(&d.join("c")), "ゲ");
        assert_eq!(version(&d.join("none")), "");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Oracle: `NXB_ORACLE_ENGINE`, `NXB_ORACLE_PYTHON`, `NXB_ORACLE_NACP` (a `control.nacp`).
    #[test]
    fn equals_the_reference_script() {
        let (Some(e), Some(py), Some(f)) = (
            std::env::var_os("NXB_ORACLE_ENGINE"),
            std::env::var_os("NXB_ORACLE_PYTHON"),
            std::env::var_os("NXB_ORACLE_NACP"),
        ) else {
            return;
        };
        let run = |a: &[&std::ffi::OsStr]| {
            let o = std::process::Command::new(&py)
                .arg(Path::new(&e).join("dm/tools/nacp.py"))
                .args(a)
                .output()
                .unwrap();
            String::from_utf8(o.stdout).unwrap().trim_end().to_string()
        };
        assert_eq!(name(Path::new(&f)), run(&[f.as_os_str()]));
        assert_eq!(
            version(Path::new(&f)),
            run(&["--version".as_ref(), f.as_os_str()])
        );
    }
}
