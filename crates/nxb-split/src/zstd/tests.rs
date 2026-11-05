//! Round trips against the `zstd` command.

use super::*;

/// Round trips through the `zstd` command when it is installed (levels 1, 3, 19, long data).
#[test]
fn against_zstd_command() {
    let mut data = Vec::new();
    let mut x = 12345u32;
    for i in 0..400_000u32 {
        x = x.wrapping_mul(1103515245).wrapping_add(12345);
        let w = ["shader", "vertex", "pixel", "BNSH", "\0\0\0", "grsc"][(x >> 16) as usize % 6];
        data.extend_from_slice(w.as_bytes());
        if i % 7 == 0 {
            data.push((x >> 8) as u8);
        }
    }
    for level in ["-1", "-3", "-19"] {
        let Ok(mut child) = std::process::Command::new("zstd")
            .args([level, "-c", "-q"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
        else {
            return;
        };
        if let Some(mut i) = child.stdin.take() {
            use std::io::Write;
            let _ = i.write_all(&data);
        }
        let Ok(o) = child.wait_with_output() else {
            return;
        };
        let got = decompress(&o.stdout);
        assert!(
            got.as_ref().is_ok_and(|g| *g == data),
            "level {level}: {:?}",
            got.err()
        );
    }
}
