//! `game.py profile <id> <out>`: the runtime game profile the host reads from `DM_PROFILE`, written into a route C
//! framework (`profile.tsv`). The values are the header's, in the file's plain spelling.

use super::header::{header_one, macros};
use super::profile::Game;
use std::path::Path;

/// (key of the file, macro suffix), in the file's order.
const KEYS: [(&str, &str); 37] = [
    ("title_id", "ID"),
    ("program_id", "PROGRAM_ID"),
    ("name", "NAME"),
    ("romfs_marker", "ROMFS_MARKER"),
    ("modules", "MODULES"),
    ("mem_reserve", "MEM_RESERVE"),
    ("mem_alias", "MEM_ALIAS"),
    ("mem_stack", "MEM_STACK"),
    ("app_pool", "APP_POOL"),
    ("mem_used", "MEM_USED"),
    ("sys_resource", "SYS_RESOURCE"),
    ("mem_map_limit", "MEM_MAP_LIMIT"),
    ("present_interval", "PRESENT_DISPLAY"),
    ("core_preempt", "CORE_PREEMPT"),
    ("alloc_big_align", "ALLOC_BIG_ALIGN"),
    ("fps_target", "FPS_TARGET"),
    ("fps_hold", "FPS_HOLD"),
    ("present_snap", "PRESENT_SNAP"),
    ("pso_mtl4", "PSO_MTL4"),
    ("shared_font", "SHARED_FONT_TTF"),
    ("pad_layout", "PAD_LAYOUT"),
    ("op_mode", "OPMODE"),
    ("perf_mode", "PERF_BOOST"),
    ("feedback_snap", "FEEDBACK_SNAP"),
    ("tex_flush_upload", "TEX_FLUSH_UPLOAD"),
    ("render_cond", "RENDER_COND"),
    ("tex_dirty_upload", "TEX_DIRTY_UPLOAD"),
    ("tex_dyn_slots", "TEX_DYN_SLOTS"),
    ("gpu_timestamps", "GPU_TS_SEGMENT"),
    ("vconv_range", "VCONV_RANGE"),
    ("stdalloc_cap", "STDALLOC_CAP"),
    ("tex_bc_cpu", "TEX_BC_CPU"),
    ("ue_texture_pool", "UE_TEXTURE_POOL"),
    ("enc_thread", "ENC_THREAD"),
    ("game_args", "ARGS"),
    ("game_env", "ENV"),
    ("", ""),
];

/// `re.findall(r'\{"([0-9a-f]+)", "([^"]*)"\}', v)`.
fn display_versions(v: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = v;
    while let Some(i) = rest.find("{\"") {
        let a = &rest[i + 2..];
        let id: String = a
            .chars()
            .take_while(|c| matches!(c, '0'..='9' | 'a'..='f'))
            .collect();
        let tail = &a[id.len()..];
        if !id.is_empty()
            && let Some(t) = tail.strip_prefix("\", \"")
            && let Some(e) = t.find('"')
            && t[e + 1..].starts_with('}')
        {
            out.push((id, t[..e].to_string()));
            rest = &t[e + 2..];
        } else {
            rest = &rest[i + 1..];
        }
    }
    out
}

/// The text of `profile.tsv` of one profile.
pub fn text(g: &Game, here: &Path) -> Result<String, String> {
    let m = macros(&header_one(g, here)?);
    let val = |k: &str| -> Option<String> {
        let v = m
            .iter()
            .find(|(x, _)| x == &format!("DM_GAME_{k}"))?
            .1
            .trim()
            .to_string();
        Some(if v.starts_with('{') && v.ends_with('}') {
            v[1..v.len() - 1]
                .split(',')
                .map(|x| x.trim().trim_matches('"'))
                .filter(|x| !x.trim().is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        } else if v.starts_with('"') {
            v[1..v.len() - 1].to_string()
        } else {
            v.strip_suffix("ull").unwrap_or(&v).to_string()
        })
    };
    let name = g.get_or("name", "None");
    let mut l = vec![
        format!(
            "# Runtime game profile of {} ({name}), written by dm/games/game.py profile from {}/game.tsv",
            g.id,
            g.dir.display()
        ),
        "# Read by the host from DM_PROFILE (hle_profile.c set_kv): KEY<TAB>VALUE, # starts a comment. Every value the compiled".into(),
        "# profile would hold is written, so the file alone describes the game.".into(),
    ];
    for (key, mac) in KEYS.iter().filter(|k| !k.0.is_empty()) {
        if let Some(v) = val(mac) {
            l.push(format!("{key}\t{v}"));
        }
    }
    let (maj, min) = (val("NVN_API_MAJOR"), val("NVN_API_MINOR"));
    l.push(match maj {
        Some(a) => format!(
            "device_api\t{a}.{}",
            min.filter(|x| x != "0" || true)
                .unwrap_or_else(|| "0".into())
        ),
        None => "device_api\t0.0".into(),
    });
    if val("SHARED_FONT_TTF").is_none() {
        l.push("shared_font\tauto".into());
    }
    let s = |k: &str| val(k).unwrap_or_else(|| "None".into());
    l.push(format!(
        "shader_key_skip\t{}+{}",
        s("SHADER_KEY_SKIP_OFF"),
        s("SHADER_KEY_SKIP_LEN")
    ));
    let dv = m
        .iter()
        .find(|(x, _)| x == "DM_GAME_DISPLAY_VERSIONS")
        .map(|(_, v)| v.as_str())
        .unwrap_or("");
    for (b, d) in display_versions(dv) {
        l.push(format!("display_version\t{b}\t{d}"));
    }
    Ok(format!("{}\n", l.join("\n")))
}

/// `game.py profile`: written only when the text changes (an unchanged file keeps its time).
pub fn write(g: &Game, here: &Path, out: &Path) -> Result<(), String> {
    let t = text(g, here)?;
    if let Some(d) = out.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    if std::fs::read_to_string(out).ok().as_deref() != Some(&t) {
        std::fs::write(out, t).map_err(|e| format!("{}: {e}", out.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::profile::Profiles;

    #[test]
    fn versions_list() {
        assert_eq!(
            display_versions(r#"{{"ab12", "1.0"}, {"cd", "x y"}}"#),
            [("ab12".into(), "1.0".into()), ("cd".into(), "x y".into())]
        );
    }

    /// Oracle: `NXB_ORACLE_ENGINE`, `NXB_ORACLE_PYTHON`: `game.py profile` of every profile whose header the script
    /// writes (a profile the script refuses is refused here too).
    #[test]
    fn every_profile_equals_the_reference_script() {
        let (Some(e), Some(py)) = (
            std::env::var_os("NXB_ORACLE_ENGINE"),
            std::env::var_os("NXB_ORACLE_PYTHON"),
        ) else {
            return;
        };
        let e = std::path::PathBuf::from(e);
        let p = Profiles::new(&e, &|_| None);
        let mut ids: Vec<String> = std::fs::read_dir(e.join("dm/games"))
            .unwrap()
            .flatten()
            .filter(|d| d.path().join("game.tsv").is_file())
            .map(|d| d.file_name().to_string_lossy().into_owned())
            .collect();
        ids.sort();
        let step = if std::env::var_os("NXB_ORACLE_ALL").is_some() {
            1
        } else {
            12
        };
        let out = std::env::temp_dir().join(format!("nxb-profile-tsv-{}.tsv", std::process::id()));
        let (mut same, mut refused) = (0, 0);
        for id in ids.iter().step_by(step) {
            let o = std::process::Command::new(&py)
                .arg(e.join("dm/games/game.py"))
                .args(["profile", id])
                .arg(&out)
                .output()
                .unwrap();
            let g = p.load(id).unwrap();
            let got = text(&g, &p.here);
            if o.status.success() {
                assert_eq!(got.unwrap(), std::fs::read_to_string(&out).unwrap(), "{id}");
                same += 1;
            } else {
                assert!(got.is_err(), "{id}: the reference refused it");
                refused += 1;
            }
            let _ = std::fs::remove_file(&out);
        }
        eprintln!("{same} profile.tsv identical, {refused} refused by both");
        assert!(same > 0);
    }
}
