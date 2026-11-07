//! The versions a build needs, against the versions its input holds.
//!
//! The NSP script packs the newest version of the folder. When the game profile needs another version (its
//! `package` or `shader_versions`), the script derives a profile from the version it has. `nxb build` takes the
//! profile's `package` and `shader_versions` as they are, and the extract stage writes one image for each version
//! that the input holds. A needed version that the input lacks never gets an image, and the rewrite stage then
//! stopped on a missing `RomFS.img` with an NRO collection error. [`check`] stops the build before the extraction,
//! and says why.

use std::path::Path;

use crate::inputs::Inputs;

/// `v327680` as 327680.
fn number(version: &str) -> Option<u32> {
    version.strip_prefix('v').and_then(|n| n.parse().ok())
}

/// The versions the input holds, oldest first: `v0` for the base game, then one for each update.
pub(crate) fn held(src: &Inputs) -> Vec<String> {
    let mut v: Vec<u32> = src.updates.iter().map(|u| u.0).collect();
    if src.has_base {
        v.push(0);
    }
    v.sort_unstable();
    v.dedup();
    v.into_iter().map(|n| format!("v{n}")).collect()
}

/// The `needed` versions that no package of the input gives and that the work folder has no image of either (an
/// image from an earlier build is reused as it is). In the order of `needed`, without repeats.
pub(crate) fn missing(src: &Inputs, work: &Path, needed: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for &version in needed {
        let Some(n) = number(version) else {
            // not a version number: nothing can give it
            if !out.iter().any(|m| m == version) {
                out.push(version.to_string());
            }
            continue;
        };
        let given = if n == 0 {
            src.has_base
        } else {
            src.updates.iter().any(|u| u.0 == n)
        };
        let kept = work
            .join("extract")
            .join(version)
            .join("RomFS.img")
            .is_file();
        if !given && !kept && !out.iter().any(|m| m == version) {
            out.push(version.to_string());
        }
    }
    out
}

/// Stops a build whose input lacks a version it needs: `Err((exit code, text))`.
///
/// - `explicit`: the packed version came from `-p` / `--package`. The script refuses that with an error when the
///   folder lacks it, and so does this (exit code 1).
/// - Any other missing version (the profile's `package`, or one of its `shader_versions`) is a case the script
///   handles with a derived profile and `nxb build` does not do yet: exit code 4, "not supported yet", so that the
///   NX2Apple launcher builds the game with the script.
pub(crate) fn check(
    src: &Inputs,
    work: &Path,
    title: &str,
    package: &str,
    split_versions: &[String],
    explicit: bool,
) -> Result<(), (i32, String)> {
    let needed: Vec<&str> = split_versions
        .iter()
        .map(String::as_str)
        .chain([package])
        .collect();
    let missing = missing(src, work, &needed);
    if missing.is_empty() {
        return Ok(());
    }
    let have = held(src).join(", ");
    if explicit && missing.iter().any(|m| m == package) {
        return Err((
            1,
            format!(
                "the version to pack (-p / --package) is not in the input: {package}. The input holds {have}"
            ),
        ));
    }
    Err((
        4,
        format!(
            "unsupported: game profile {title} needs {} and the input holds {have}. The NSP script packs such an input from a profile derived from the version it has, and nxb does not do that yet",
            missing.join(", ")
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::{check, held, missing};
    use crate::inputs::{Inputs, Source};
    use std::path::PathBuf;

    fn inputs(has_base: bool, updates: &[u32]) -> Inputs {
        Inputs {
            source: Source::Packages,
            app: 0x0100_F2C0_115B_6000,
            base: PathBuf::from("base.nsp"),
            updates: updates
                .iter()
                .map(|v| (*v, PathBuf::from(format!("u{v}.nsp"))))
                .collect(),
            dlc: Vec::new(),
            app_flag: None,
            has_base,
            notes: Vec::new(),
        }
    }

    fn work(name: &str) -> PathBuf {
        let w = std::env::temp_dir().join(format!("nxb-versions-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&w);
        std::fs::create_dir_all(&w).expect("work folder");
        w
    }

    fn keep_image(work: &std::path::Path, version: &str) {
        let d = work.join("extract").join(version);
        std::fs::create_dir_all(&d).expect("version folder");
        std::fs::write(d.join("RomFS.img"), b"x").expect("image");
    }

    #[test]
    fn held_lists_the_base_game_and_each_update_oldest_first() {
        assert_eq!(
            held(&inputs(true, &[720_896, 655_360])),
            ["v0", "v655360", "v720896"]
        );
        assert_eq!(held(&inputs(false, &[65_536])), ["v65536"]);
    }

    #[test]
    fn a_version_the_input_holds_is_not_missing() {
        let w = work("holds");
        let src = inputs(true, &[720_896]);
        assert!(missing(&src, &w, &["v0", "v720896", "v720896"]).is_empty());
        let _ = std::fs::remove_dir_all(&w);
    }

    #[test]
    fn a_version_nothing_gives_is_missing_once() {
        let w = work("lacks");
        let src = inputs(true, &[655_360]);
        assert_eq!(
            missing(&src, &w, &["v720896", "v0", "v720896", "latest"]),
            ["v720896", "latest"]
        );
        let _ = std::fs::remove_dir_all(&w);
    }

    #[test]
    fn an_image_of_an_earlier_build_counts() {
        let w = work("kept");
        let src = inputs(true, &[655_360]);
        keep_image(&w, "v720896");
        assert!(missing(&src, &w, &["v720896"]).is_empty());
        let _ = std::fs::remove_dir_all(&w);
    }

    #[test]
    fn the_profile_package_that_the_folder_lacks_is_not_supported_yet() {
        let w = work("profile");
        let src = inputs(true, &[655_360]);
        let (code, text) = check(
            &src,
            &w,
            "0100F2C0115B6000",
            "v720896",
            &["v720896".to_string()],
            false,
        )
        .expect_err("v720896 is not in the folder");
        assert_eq!(code, 4);
        assert!(text.starts_with("unsupported:"), "{text}");
        assert!(
            text.contains("v720896") && text.contains("v0, v655360"),
            "{text}"
        );
        let _ = std::fs::remove_dir_all(&w);
    }

    #[test]
    fn a_shader_version_that_the_folder_lacks_is_not_supported_yet_even_with_p() {
        let w = work("shaders");
        let src = inputs(true, &[655_360]);
        let (code, text) = check(
            &src,
            &w,
            "0100F2C0115B6000",
            "v655360",
            &["v720896".to_string()],
            true,
        )
        .expect_err("the shader version is not in the folder");
        assert_eq!(code, 4);
        assert!(text.contains("v720896"), "{text}");
        let _ = std::fs::remove_dir_all(&w);
    }

    #[test]
    fn a_p_version_that_the_folder_lacks_is_an_error_as_in_the_script() {
        let w = work("explicit");
        let src = inputs(true, &[655_360]);
        let (code, text) = check(
            &src,
            &w,
            "0100F2C0115B6000",
            "v720896",
            &["v720896".to_string()],
            true,
        )
        .expect_err("-p names a version the folder lacks");
        assert_eq!(code, 1);
        assert!(
            text.contains("-p / --package") && text.contains("v720896"),
            "{text}"
        );
        assert!(!text.starts_with("unsupported"), "{text}");
        let _ = std::fs::remove_dir_all(&w);
    }

    #[test]
    fn an_input_with_every_needed_version_passes() {
        let w = work("passes");
        let src = inputs(true, &[720_896]);
        assert!(
            check(
                &src,
                &w,
                "0100F2C0115B6000",
                "v720896",
                &["v0".to_string(), "v720896".to_string()],
                false,
            )
            .is_ok()
        );
        let _ = std::fs::remove_dir_all(&w);
    }

    /// A dump folder of a game with a base and one update, a profile whose `package` is another version.
    fn dump_and_engine(name: &str, update: u32) -> (PathBuf, PathBuf, PathBuf) {
        let root = work(name);
        let (dump, engine) = (root.join("dump"), root.join("engine"));
        let mut npdm = vec![0u8; 0xA0];
        npdm[..4].copy_from_slice(b"META");
        npdm[0x70..0x74].copy_from_slice(&0x80u32.to_le_bytes());
        npdm[0x74..0x78].copy_from_slice(&0x20u32.to_le_bytes());
        npdm[0x80..0x84].copy_from_slice(b"ACI0");
        npdm[0x90..0x98].copy_from_slice(&0x0100_F2C0_115B_6000u64.to_le_bytes());
        std::fs::create_dir_all(dump.join("base/exefs")).expect("base exefs");
        std::fs::write(dump.join("base/exefs/main.npdm"), npdm).expect("npdm");
        std::fs::create_dir_all(dump.join("base/romfs")).expect("base romfs");
        std::fs::create_dir_all(dump.join(format!("updates/{update}/romfs"))).expect("update");
        let profile = engine.join("dm/games/0100F2C0115B6000");
        std::fs::create_dir_all(&profile).expect("profile folder");
        std::fs::write(
            profile.join("game.tsv"),
            "name\tTest game\npackage\tv720896\nshader_versions\tv720896\n",
        )
        .expect("game.tsv");
        (root, dump, engine)
    }

    fn plan_of(root: &std::path::Path, dump: PathBuf, engine: PathBuf) -> Result<(), String> {
        let a = crate::Args {
            dump: Some(dump),
            engine: Some(engine),
            work_dir: Some(root.join("work")),
            shaders: Some(root.join("shaders")),
            ..crate::Args::default()
        };
        crate::graph::plan(&a).map(|_| ())
    }

    #[test]
    fn plan_stops_when_the_profile_package_is_not_in_the_input() {
        let (root, dump, engine) = dump_and_engine("plan-lacks", 655_360);
        let e = plan_of(&root, dump, engine).expect_err("v720896 is not in the dump");
        assert!(e.starts_with("unsupported:"), "{e}");
        assert!(e.contains("v720896") && e.contains("v0, v655360"), "{e}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn plan_goes_on_when_the_profile_package_is_in_the_input() {
        let (root, dump, engine) = dump_and_engine("plan-has", 720_896);
        plan_of(&root, dump, engine).expect("the dump holds v720896");
        let _ = std::fs::remove_dir_all(&root);
    }
}
