//! The run-time shader captures: shaders no game file holds. A game that compiles some with its own GLSLC (Fire
//! Emblem: Three Houses, `DM_GLSLC_CAPTURE`, `dm/host/hle_glslc.c`), a Vulkan game without SPIR-V files (Super Mario
//! Galaxy, `DM_VK_CAPTURE`, `dm/gfx/vk/hle_vk.m`) and an OpenGL game whose GLSL is put together at run time
//! (Balatro, `DM_GL_CAPTURE`, `dm/gfx/gl/hle_gl.m`). The game boots in the Mac host (headless and silent) and the
//! host writes each shader under a temporary name, then renames it. The outputs are the same on every run, so a
//! folder that already holds every file the profile names is reused.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use super::{Ctx, boot_mac, hms, host_stopped, read_log, rmtree, set_env, swap_in, tail_lines};
use crate::events::Update;

/// `GLSLC_SECS`: the GLSLC capture's time limit.
pub(crate) const GLSLC_SECS: f64 = 30.0;
/// `GLSLC_KEYS`: A every 4 s from 5 s on during the GLSLC capture (`DM_KEYS`). Crash Team Racing Nitro-Fueled
/// compiles its two run-time programs only after a button press on its title screen.
pub(crate) const GLSLC_KEYS: &str = "5:A*7/4";
/// `CAPTURE_SECS`: the Vulkan and GL capture's time limit.
pub(crate) const CAPTURE_SECS: f64 = 120.0;
/// Without a capture table, a Vulkan or GL capture ends once no shader was added for this long.
pub(crate) const CAPTURE_QUIET_SECS: f64 = 20.0;

/// `glslc_wanted`: the capture files the profile's shader map names (`/glslc_capture/<name>#GLSLC@...`), empty when
/// it names none.
pub(crate) fn glslc_wanted(shader_map: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = shader_map;
    while let Some(i) = rest.find("/glslc_capture/") {
        rest = &rest[i + "/glslc_capture/".len()..];
        let b = rest.as_bytes();
        if b.len() > 22
            && b[..16]
                .iter()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(c))
            && &b[16..22] == b".glslc"
            && b[22] == b'#'
        {
            out.insert(rest[..22].to_string());
        }
    }
    out
}

/// A whole GLSLC output: magic `0x19866891` and its own length at +0x44.
pub(crate) fn glslc_whole(b: &[u8]) -> bool {
    b.len() >= 0x48
        && b[..4] == [0x91, 0x68, 0x86, 0x19]
        && b.get(0x44..0x48)
            .and_then(|x| x.try_into().ok())
            .map(u32::from_le_bytes)
            .is_some_and(|n| n as usize == b.len())
}

fn names(d: &Path) -> Vec<String> {
    std::fs::read_dir(d)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .collect()
}

/// `glslc_done`: the capture files written whole in `d`.
pub(crate) fn glslc_done(d: &Path) -> BTreeSet<String> {
    names(d)
        .into_iter()
        .filter(|n| n.ends_with(".glslc"))
        .filter(|n| std::fs::read(d.join(n)).is_ok_and(|b| glslc_whole(&b)))
        .collect()
}

/// `[RED] shader <key> is not in the shader map` lines of a GLSLC observation run: the keys the game submitted.
pub(crate) fn observed_keys(log: &str) -> BTreeSet<String> {
    const P: &str = "[RED] shader ";
    let mut out = BTreeSet::new();
    let mut rest = log;
    while let Some(i) = rest.find(P) {
        rest = &rest[i + P.len()..];
        if rest.len() >= 16
            && rest.as_bytes()[..16]
                .iter()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(c))
            && rest[16..].starts_with(" is not in the shader map")
        {
            out.insert(rest[..16].to_string());
        }
    }
    out
}

/// `shader_keys`: the base shaders of the shader map (column 4 of the rows whose first column ends with `.bin`).
pub(crate) fn shader_keys(shader_map: &str) -> BTreeSet<String> {
    shader_map
        .lines()
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            (c.len() > 4 && c[0].ends_with(".bin")).then(|| c[3].to_string())
        })
        .collect()
}

/// `check_glslc_coverage`: an empty GLSLC capture passes only when the offline split holds every shader the boot
/// submitted. This proves the boot's shaders only, not later gameplay.
pub(crate) fn check_glslc_coverage(
    observed: &BTreeSet<String>,
    shader_map: &str,
) -> Result<String, String> {
    if observed.is_empty() {
        return Err("no submitted shader keys to validate after the empty GLSLC capture".into());
    }
    let have = shader_keys(shader_map);
    let missing: Vec<&str> = observed
        .iter()
        .filter(|k| !have.contains(*k))
        .map(String::as_str)
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "no run-time GLSLC outputs and {} observed shaders absent from the offline split: {}",
            missing.len(),
            missing.join(", ")
        ));
    }
    Ok(format!(
        "the offline split covers all {} shaders observed during the GLSLC capture boot (later gameplay unverified)",
        observed.len()
    ))
}

/// `capture_glslc` for the packed version `d` (`<work>/extract/<package>`) among the split's versions `dirs`.
/// Returns the keys a boot submitted when it captured no output and the profile names none (they are checked against
/// the split with [`check_glslc_coverage`]).
pub(crate) fn capture_glslc(
    c: &Ctx<'_>,
    dirs: &[(String, PathBuf)],
) -> Result<Option<BTreeSet<String>>, String> {
    let map = c.profile.join("shader_map.tsv");
    let want = glslc_wanted(&std::fs::read_to_string(&map).unwrap_or_default());
    let todo: Vec<&str> = dirs
        .iter()
        .filter(|(_, d)| {
            let got = glslc_done(&d.join("glslc_capture"));
            if want.is_empty() {
                got.is_empty()
            } else {
                !got.is_superset(&want)
            }
        })
        .map(|(v, _)| v.as_str())
        .collect();
    if todo.is_empty() {
        c.p.say(&format!(
            "reusing the captured run-time shaders: {}",
            dirs.iter()
                .map(|(_, d)| d.join("glslc_capture").display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        return Ok(None);
    }
    if todo != [c.package.as_str()] {
        return Err(format!(
            "the run-time shader capture runs the packed version {}'s code; {} needs a capture of its own",
            c.package,
            todo.iter()
                .filter(|v| **v != c.package)
                .copied()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    c.p.set(Update {
        step: Some("building the Mac host"),
        ..Update::default()
    });
    let host = c.host()?;
    let d = c.game_dir();
    let cap = d.join("glslc_capture.partial");
    rmtree(&cap)?;
    let (rd, mut env) = c.run_dir("glslc_run", false)?;
    let log = c.logs().join("glslc_capture.log");
    // Only a build with no required capture can prove offline coverage: an empty map shows every submitted key.
    let shmap = if want.is_empty() {
        let m = rd.join("shader_map.tsv");
        std::fs::write(&m, "# GLSLC capture: observe submitted shaders\n")
            .map_err(|e| format!("{}: {e}", m.display()))?;
        m
    } else {
        map
    };
    set_env(&mut env, "DM_NVN_SHMAP", shmap);
    set_env(&mut env, "DM_GLSLC_CAPTURE", &cap);
    set_env(&mut env, "DM_KEYS", GLSLC_KEYS);
    let total = (!want.is_empty()).then_some(want.len() as u64);
    c.p.set(Update {
        step: Some("booting the game"),
        done: Some(0),
        total,
        unit: Some("shaders"),
        ..Update::default()
    });
    c.p.say(&format!(
        "booting the game in the Mac host to capture the shaders it compiles at run time → {}",
        d.join("glslc_capture").display()
    ));
    let t0 = Instant::now();
    let ran = boot_mac(
        &host,
        &c.patched(),
        &env,
        &rd,
        &log,
        GLSLC_SECS,
        &mut || {
            let got = glslc_done(&cap);
            let n = if want.is_empty() {
                got.len()
            } else {
                got.intersection(&want).count()
            };
            c.p.set(Update {
                done: Some(n as u64),
                ..Update::default()
            });
            !want.is_empty() && got.is_superset(&want)
        },
    )?;
    let got = glslc_done(&cap);
    let full = !want.is_empty() && got.is_superset(&want);
    if let Some(rc) = ran.rc
        && !full
    {
        c.p.say(&host_stopped(&log, rc, ran.secs));
    }
    let any_file = names(&cap).iter().any(|n| n.ends_with(".glslc"));
    if want.is_empty() && got.is_empty() && ran.rc.is_none() && !any_file {
        let text = read_log(&log);
        let observed = observed_keys(&text);
        if !observed.is_empty() && !text.contains("[RED] glslc:") {
            rmtree(&cap)?;
            rmtree(&rd)?;
            c.p.say(&format!(
                "GLSLC capture: no run-time outputs captured, {} submitted shader keys to check against the offline split",
                observed.len()
            ));
            c.p.say("the GLSL/import heuristic can also select a compiler used only on cache misses. The build continues only if the offline split covers every shader observed in this boot. Later gameplay is unverified");
            return Ok(Some(observed));
        }
    }
    if got.is_empty() || (!want.is_empty() && !full) {
        c.p.say(&format!(
            "---- last lines of {} ----\n{}",
            log.display(),
            tail_lines(&log, 40)
        ));
        return Err(format!(
            "captured only {} run-time shaders{} (host output: {})",
            got.len(),
            if want.is_empty() {
                String::new()
            } else {
                format!(", the profile needs {}", want.len())
            },
            log.display()
        ));
    }
    let extra: Vec<&str> = got.difference(&want).map(String::as_str).collect();
    if !want.is_empty() && !extra.is_empty() {
        c.p.say(&format!(
            "[RED] captured files the profile's shader map does not name: {}",
            extra.join(", ")
        ));
    }
    swap_in(&cap, &d.join("glslc_capture"))?;
    rmtree(&rd)?;
    c.p.say(&format!(
        "GLSLC capture: {} shaders in {}",
        got.len(),
        hms(t0.elapsed().as_secs_f64())
    ));
    Ok(None)
}

/// A run-time shader capture's graphics API (`CAPTURES`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Api {
    /// Vulkan: `<16 hex>.spv`.
    Vk,
    /// OpenGL: `<16 hex>.vert` and `.frag`.
    Gl,
}

impl Api {
    /// `vk` or `gl`, the folder's prefix.
    pub(crate) fn id(self) -> &'static str {
        match self {
            Api::Vk => "vk",
            Api::Gl => "gl",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Api::Vk => "Vulkan",
            Api::Gl => "GL",
        }
    }

    fn env(self) -> &'static str {
        match self {
            Api::Vk => "DM_VK_CAPTURE",
            Api::Gl => "DM_GL_CAPTURE",
        }
    }

    /// The profile's table of the keys a capture must hold.
    pub(crate) fn table(self) -> &'static str {
        match self {
            Api::Vk => "vk_capture.tsv",
            Api::Gl => "gl_capture.tsv",
        }
    }

    fn what(self) -> &'static str {
        match self {
            Api::Vk => "SPIR-V shaders it generates",
            Api::Gl => "GLSL shaders it puts together",
        }
    }

    /// A capture file's name to its key, `None` when it is not a whole file (a temporary name).
    pub(crate) fn key(self, n: &str) -> Option<String> {
        let hex16 = |s: &str| {
            s.len() == 16
                && s.bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        };
        match self {
            Api::Vk => (n.len() == 20 && n.ends_with(".spv")).then(|| n[..16].to_string()),
            Api::Gl => n
                .split_once('.')
                .filter(|(h, x)| hex16(h) && (*x == "vert" || *x == "frag"))
                .map(|_| n.to_string()),
        }
    }
}

/// `capture_wanted`: the keys of a capture table, empty without one.
pub(crate) fn capture_wanted(table: &str) -> BTreeSet<String> {
    table
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| l.trim().to_string())
        .collect()
}

/// `capture_done`: the keys of the whole files in a capture folder.
pub(crate) fn capture_done(d: &Path, api: Api) -> BTreeSet<String> {
    names(d).iter().filter_map(|n| api.key(n)).collect()
}

/// `capture_shaders` for the packed version: `<work>/extract/<package>/<api>_capture`, one more input folder of the
/// shader translator. The run stops once every key of the profile's capture table is there. Without the table, it
/// stops once none was added for [`CAPTURE_QUIET_SECS`].
pub(crate) fn capture_shaders(c: &Ctx<'_>, api: Api) -> Result<PathBuf, String> {
    let want =
        capture_wanted(&std::fs::read_to_string(c.profile.join(api.table())).unwrap_or_default());
    let d = c.game_dir();
    let out = d.join(format!("{}_capture", api.id()));
    if !want.is_empty() && capture_done(&out, api).is_superset(&want) {
        c.p.say(&format!(
            "reusing the captured run-time shaders: {}",
            out.display()
        ));
        return Ok(out);
    }
    c.p.set(Update {
        step: Some("building the Mac host"),
        ..Update::default()
    });
    let host = c.host()?;
    let cap = d.join(format!("{}_capture.partial", api.id()));
    rmtree(&cap)?;
    std::fs::create_dir_all(&cap).map_err(|e| format!("{}: {e}", cap.display()))?;
    let (rd, mut env) = c.run_dir(&format!("{}_run", api.id()), true)?;
    let log = c.logs().join(format!("{}_capture.log", api.id()));
    set_env(&mut env, api.env(), &cap);
    c.p.set(Update {
        step: Some("booting the game"),
        done: Some(0),
        total: (!want.is_empty()).then_some(want.len() as u64),
        unit: Some("shaders"),
        ..Update::default()
    });
    c.p.say(&format!(
        "booting the game in the Mac host to capture the {} at run time → {}",
        api.what(),
        out.display()
    ));
    let t0 = Instant::now();
    let (mut last_n, mut last_t) = (0usize, 0.0f64);
    let ran = boot_mac(
        &host,
        &c.patched(),
        &env,
        &rd,
        &log,
        CAPTURE_SECS,
        &mut || {
            let got = capture_done(&cap, api);
            let n = if want.is_empty() {
                got.len()
            } else {
                got.intersection(&want).count()
            };
            c.p.set(Update {
                done: Some(n as u64),
                ..Update::default()
            });
            let now = t0.elapsed().as_secs_f64();
            if n != last_n {
                (last_n, last_t) = (n, now);
            }
            quiet_done(&want, &got, n, now - last_t)
        },
    )?;
    let got = capture_done(&cap, api);
    let full = !want.is_empty() && got.is_superset(&want);
    if let Some(rc) = ran.rc
        && !full
    {
        c.p.say(&host_stopped(&log, rc, ran.secs));
    }
    if got.is_empty() || (!want.is_empty() && !full) {
        c.p.say(&format!(
            "---- last lines of {} ----\n{}",
            log.display(),
            tail_lines(&log, 40)
        ));
        return Err(format!(
            "captured only {} run-time shaders{} (host output: {})",
            got.len(),
            if want.is_empty() {
                String::new()
            } else {
                format!(", the profile needs {}", want.len())
            },
            log.display()
        ));
    }
    let extra = got.difference(&want).count();
    if !want.is_empty() && extra > 0 {
        c.p.say(&format!(
            "[RED] captured shaders the profile's {} does not list: {extra}",
            api.table()
        ));
    }
    swap_in(&cap, &out)?;
    rmtree(&rd)?;
    c.p.say(&format!(
        "{} shader capture: {} shaders in {}",
        api.name(),
        got.len(),
        hms(t0.elapsed().as_secs_f64())
    ));
    Ok(out)
}

/// The capture's end: every listed key is there, or without a table, some are and none was added for
/// [`CAPTURE_QUIET_SECS`].
pub(crate) fn quiet_done(
    want: &BTreeSet<String>,
    got: &BTreeSet<String>,
    n: usize,
    quiet: f64,
) -> bool {
    if want.is_empty() {
        n > 0 && quiet > CAPTURE_QUIET_SECS
    } else {
        got.is_superset(want)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(v: &[&str]) -> BTreeSet<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_map_names_its_glslc_captures() {
        let m = "a.bin\t0\t0\tk\t/glslc_capture/0123456789abcdef.glslc#GLSLC@0\n\
                 b.bin\t0\t0\tk\t/glslc_capture/0123456789ABCDEF.glslc#GLSLC@0\n\
                 c.bin\t0\t0\tk\t/glslc_capture/fedcba9876543210.glslc#GLSLC@10\n";
        assert_eq!(
            glslc_wanted(m),
            set(&["0123456789abcdef.glslc", "fedcba9876543210.glslc"])
        );
        assert!(glslc_wanted("x").is_empty());
    }

    #[test]
    fn a_glslc_output_is_whole_with_its_own_length() {
        let mut b = vec![0u8; 0x50];
        b[..4].copy_from_slice(&[0x91, 0x68, 0x86, 0x19]);
        b[0x44..0x48].copy_from_slice(&0x50u32.to_le_bytes());
        assert!(glslc_whole(&b));
        assert!(!glslc_whole(&b[..0x4c]));
        b[0] = 0;
        assert!(!glslc_whole(&b));
    }

    #[test]
    fn observed_keys_and_coverage() {
        let log = "x [RED] shader 0123456789abcdef is not in the shader map\n[RED] shader 0123 is not in the shader map\n[RED] shader fedcba9876543210 is not in the shader map";
        let o = observed_keys(log);
        assert_eq!(o, set(&["0123456789abcdef", "fedcba9876543210"]));
        let map = "a.bin\t1\t2\t0123456789abcdef\tx\nb.txt\t1\t2\tfedcba9876543210\tx\n";
        let e = check_glslc_coverage(&o, map).err().unwrap_or_default();
        assert!(e.ends_with("1 observed shaders absent from the offline split: fedcba9876543210"));
        let map = format!("{map}b.bin\t1\t2\tfedcba9876543210\tx\n");
        assert!(check_glslc_coverage(&o, &map).is_ok());
        assert!(check_glslc_coverage(&BTreeSet::new(), &map).is_err());
    }

    #[test]
    fn capture_names_are_keys_only_when_whole() {
        assert_eq!(
            Api::Vk.key("0123456789abcdef.spv").as_deref(),
            Some("0123456789abcdef")
        );
        assert_eq!(Api::Vk.key("0123456789abcdef.spv.tmp"), None);
        assert_eq!(
            Api::Gl.key("0123456789abcdef.vert").as_deref(),
            Some("0123456789abcdef.vert")
        );
        assert_eq!(
            Api::Gl.key("0123456789abcdef.frag").as_deref(),
            Some("0123456789abcdef.frag")
        );
        assert_eq!(Api::Gl.key("0123456789ABCDEF.frag"), None);
        assert_eq!(Api::Gl.key("0123456789abcdef.geom"), None);
        assert_eq!(Api::Gl.key("0123456789abcdef.vert.part"), None);
        assert_eq!(capture_wanted("# keys\n a \n\nb\n#c\n"), set(&["a", "b"]));
    }

    #[test]
    fn a_capture_without_table_ends_when_quiet() {
        let w = set(&["a", "b"]);
        assert!(quiet_done(&w, &set(&["a", "b", "c"]), 2, 0.0));
        assert!(!quiet_done(&w, &set(&["a"]), 1, 99.0));
        let none = BTreeSet::new();
        assert!(!quiet_done(&none, &set(&["a"]), 1, 20.0));
        assert!(quiet_done(&none, &set(&["a"]), 1, 20.5));
        assert!(!quiet_done(&none, &none, 0, 99.0));
    }
}
