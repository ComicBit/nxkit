//! Translates every SPIR-V file of a RomFS image and compiles the MSL with `xcrun metal`. Run by hand:
//! `NXB_VK_ROMFS=<RomFS.img> NXB_TEST_OUT=<dir> cargo test -p nxb-build --test vk_probe -- --ignored --nocapture`.
#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use nxb_build::host::rom::Rom;
use std::path::{Path, PathBuf};

fn metal(src: &str, path: &Path) -> Result<(), String> {
    std::fs::write(path, src).unwrap();
    let r = std::process::Command::new("xcrun")
        .args([
            "-sdk",
            "macosx",
            "metal",
            "-c",
            "-fpreserve-invariance",
            "-std=macos-metal2.3",
        ])
        .arg(path)
        .arg("-o")
        .arg(path.with_extension("air"))
        .output()
        .unwrap();
    if r.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&r.stderr)
            .chars()
            .take(800)
            .collect())
    }
}

#[test]
#[ignore]
fn probe() {
    let img = PathBuf::from(std::env::var("NXB_VK_ROMFS").unwrap());
    let out = PathBuf::from(std::env::var("NXB_TEST_OUT").unwrap());
    std::fs::create_dir_all(&out).unwrap();
    let rom = Rom::open(&img).unwrap();
    let (mut n, mut ok, mut comp, mut dual) = (0, 0, 0, 0);
    for f in rom.files() {
        if !nxb_layer::is_spirv(&rom.head(f, 4).unwrap()) {
            continue;
        }
        n += 1;
        let data = rom.read(f, 0, f.size).unwrap();
        let name = f.path.trim_start_matches('/').replace('/', "_");
        match nxb_layer::vk::translate(&data) {
            Err(e) => println!("FAIL {}: {e}", f.path),
            Ok(nxb_layer::vk::Outcome::Skipped(st)) => println!("SKIP {} {st}", f.path),
            Ok(nxb_layer::vk::Outcome::Shader(s)) => {
                ok += 1;
                println!("ROW {}\t{}\t{}", s.key, s.stage, s.fields);
                match metal(&s.msl, &out.join(format!("{name}.metal"))) {
                    Ok(()) => comp += 1,
                    Err(e) => println!("METAL {}: {e}", f.path),
                }
                if let Some(d) = &s.msl_dual {
                    match metal(d, &out.join(format!("{name}_d.metal"))) {
                        Ok(()) => dual += 1,
                        Err(e) => println!("METAL dual {}: {e}", f.path),
                    }
                }
            }
        }
    }
    println!("spirv {n} translated {ok} compiled {comp} dual {dual}");
}
