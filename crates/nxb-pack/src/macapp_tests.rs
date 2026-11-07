//! The tables of the Mac app: the guard tables of the shader build before those of the profile.

use super::*;

#[test]
fn the_shader_builds_guard_tables_take_the_place_of_the_profiles() {
    let d = std::env::temp_dir().join(format!("nxb-macapp-tables-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let (prof, guard) = (d.join("prof"), d.join("guard"));
    std::fs::create_dir_all(&prof).unwrap();
    std::fs::create_dir_all(&guard).unwrap();
    for n in ["shader_map.tsv", "c1_full.tsv", "cb_extent.tsv"] {
        std::fs::write(prof.join(n), "").unwrap();
    }
    std::fs::write(guard.join("c1_full.tsv"), "").unwrap();
    std::fs::write(guard.join("shader_map.tsv"), "").unwrap();
    assert_eq!(
        table_files(&prof, Some(&guard)),
        [
            prof.join("shader_map.tsv"),
            guard.join("c1_full.tsv"),
            prof.join("cb_extent.tsv")
        ]
    );
    assert_eq!(
        table_files(&prof, None),
        ["shader_map.tsv", "c1_full.tsv", "cb_extent.tsv"].map(|n| prof.join(n))
    );
    let _ = std::fs::remove_dir_all(&d);
}
