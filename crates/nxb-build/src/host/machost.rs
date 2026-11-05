//! The Mac host (`<work>/tools/dmhost`): the NSP script's `build_mac_host`. `make` generates what the host's compile
//! line needs (the profile header, libvpx, dav1d, the host's Metal shaders), `make -n -B` gives the compile line, which
//! runs with the output moved into the work folder. Reused while the compile line and its inputs are as at the last
//! build into the same path: the reuse key (`dmhost.key`) and the decoder identity (`dmhost.decoder.json` `inputs`)
//! are the script's [`fingerprint`]s, byte for byte, so a host the script built is reused here and the other way round.
//!
//! What "equal" means for the binary: the compile line is the script's argument for argument, but `clang -g` links
//! through temporary objects whose random names and times reach the debug map, so two builds by the script itself
//! differ in those bytes (and in the UUID derived from them). Equal is: the same key, the same decoder inputs, and the
//! same code (`__TEXT`, `__DATA`) as the script's build.

use super::Runner;
use super::fingerprint::{Item, fingerprint, repr_list, repr_path};
use super::pyfmt::{repr_str, shlex_split, splitlines};
use std::path::{Path, PathBuf};

/// The folders the compile line takes its sources and headers from (`dm/host/Makefile` `SRC`, `GFX`, `GFX_H`).
pub const HOST_DIRS: [&str; 7] = [
    "dm/host",
    "dm/gfx",
    "dm/gfx/nvn",
    "dm/gfx/nvn/include",
    "dm/gfx/tex",
    "dm/gfx/vk",
    "dm/gfx/gl",
];
/// The chain version header the host is compiled with.
pub const CHAIN_H: &str = "dm/gfx/nvn/dmsh_chain.h";

/// `Path.suffix`.
fn suffix(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if i > 0 && i + 1 < name.len() => &name[i..],
        _ => "",
    }
}

/// The source files of [`HOST_DIRS`] (`.c`, `.m`, `.h`, `.S`, `.inc`), each folder sorted by name.
pub fn sources(engine: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for d in HOST_DIRS {
        let mut names: Vec<PathBuf> = std::fs::read_dir(engine.join(d))
            .map(|rd| rd.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        names.sort();
        out.extend(names.into_iter().filter(|p| {
            let n = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            p.is_file() && [".c", ".m", ".h", ".S", ".inc"].contains(&suffix(&n))
        }));
    }
    out
}

/// The chain header with its version replaced by `<chain>` (`repr`), `None` without the file.
fn chain_repr(chain: &Path) -> String {
    let Ok(text) = std::fs::read_to_string(chain) else {
        return "None".into();
    };
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<String> = text
        .split('\n')
        .map(|l| {
            let v = l
                .strip_prefix("#define DMSH_CHAIN \"")
                .and_then(|r| r.strip_suffix('"'));
            match v {
                Some(v) if !v.contains('"') => "#define DMSH_CHAIN \"<chain>\"".to_string(),
                _ => l.to_string(),
            }
        })
        .collect();
    repr_str(&lines.join("\n"))
}

/// The compile line, its reuse key and its decoder identity.
#[derive(Clone, Debug)]
pub struct Plan {
    /// The compile line with `-o <out>`.
    pub cmd: Vec<String>,
    /// `<out>.key`'s first line.
    pub key: String,
    /// `<out>.decoder.json`'s `inputs`.
    pub decoder: String,
}

fn log(logs: &Path, name: &str, text: &str) {
    if std::fs::create_dir_all(logs).is_ok() {
        let _ = std::fs::write(logs.join(name), text);
    }
}

/// Generate the compile line's inputs (`make`) and compute the plan for `out` (absolute, as the script's resolved work
/// folder gives it).
pub fn plan(
    r: &Runner,
    engine: &Path,
    out: &Path,
    game: &str,
    logs: &Path,
) -> Result<Plan, String> {
    let host = engine.join("dm/host");
    let g = format!("GAME={game}");
    let made = r.run(
        "make",
        [
            "-s",
            &g,
            &format!("gen/{game}/game_profile.h"),
            "gen/vpx/libvpx.a",
            "gen/dav1d/libdav1d.a",
            "host_msl",
        ],
        Some(&host),
    );
    match &made {
        Ok(c) => log(
            logs,
            "mac_host_make.log",
            &format!(
                "{}{}",
                String::from_utf8_lossy(&c.stdout),
                String::from_utf8_lossy(&c.stderr)
            ),
        ),
        Err(e) => {
            log(logs, "mac_host_make.log", e);
            return Err(format!(
                "{e} (full output: {})",
                logs.join("mac_host_make.log").display()
            ));
        }
    }
    let dry = r
        .stdout("make", ["-n", "-B", &g, "dmhost"], Some(&host))
        .map_err(|e| format!("make -n gave no compile line for the Mac host: {e}"))?;
    let line = splitlines(&dry)
        .into_iter()
        .find(|l| l.contains(" -o dmhost "))
        .ok_or("make -n gave no compile line for the Mac host")?;
    let mut cmd = shlex_split(line)?;
    let o = cmd
        .iter()
        .position(|a| a == "-o")
        .ok_or("the Mac host's compile line has no -o")?;
    let out_s = out.to_string_lossy().into_owned();
    *cmd.get_mut(o + 1)
        .ok_or("the Mac host's compile line ends at -o")? = out_s;
    let mut cmd_repr: Vec<String> = cmd.iter().map(|a| repr_str(a)).collect();
    cmd_repr[o + 1] = repr_path(out);
    let tp = engine.join("dm/third_party");
    let srcs = sources(engine);
    let common = fingerprint(
        &[
            Item::Repr(repr_list(&cmd_repr)),
            Item::Files(vec![
                host.join(format!("gen/{game}/game_profile.h")),
                host.join("gen/msl-macosx/host_msl_gen.h"),
                host.join("gen/vpx/libvpx.a"),
                host.join("gen/dav1d/libdav1d.a"),
                tp.join("stb_vorbis/stb_vorbis.c"),
            ]),
            Item::Tree(tp.join("vulkan-headers/include")),
            Item::Tree(tp.join("libvpx/vpx")),
            Item::Tree(tp.join("dav1d/include")),
            Item::Cmd(vec!["clang".into(), "--version".into()]),
            Item::Cmd(vec!["xcrun".into(), "--show-sdk-build-version".into()]),
        ],
        r,
    );
    let key = fingerprint(
        &[Item::Repr(repr_str(&common)), Item::Files(srcs.clone())],
        r,
    );
    let chain = engine.join(CHAIN_H);
    let decoder = fingerprint(
        &[
            Item::Repr(repr_str("oodle-host-v1")),
            Item::Repr(repr_str(&common)),
            Item::Files(srcs.into_iter().filter(|p| *p != chain).collect()),
            Item::Repr(chain_repr(&chain)),
        ],
        r,
    );
    Ok(Plan { cmd, key, decoder })
}

fn sibling(out: &Path, ext: &str) -> PathBuf {
    PathBuf::from(format!("{}{ext}", out.display()))
}

/// `decoder_host_key`: the recorded decoder identity while the recorded binary digest still matches.
fn decoder_host_key(r: &Runner, out: &Path) -> Option<String> {
    let text = std::fs::read_to_string(sibling(out, ".decoder.json")).ok()?;
    let v = nxb_pyjson::parse(&text).ok()?;
    let inputs = v.get("inputs")?.as_str()?.to_string();
    let ok = v.get("format")?.as_i64() == Some(1)
        && inputs.len() == 64
        && inputs
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && out.is_file()
        && v.get("binary")?.as_str()? == fingerprint(&[Item::Files(vec![out.to_path_buf()])], r);
    ok.then_some(inputs)
}

/// Build the Mac host for `game` into `out` unless the last build there is still current. Returns whether it built.
pub fn build(
    r: &Runner,
    engine: &Path,
    out: &Path,
    game: &str,
    logs: &Path,
) -> Result<bool, String> {
    if r.stdout("sysctl", ["-n", "hw.optional.arm64"], None)
        .map(|s| s.trim().to_string())
        .ok()
        .as_deref()
        != Some("1")
    {
        return Err("this step boots the game in the Mac host, which runs the game's own arm64 code natively: it needs a Mac with Apple silicon".into());
    }
    let engine = std::fs::canonicalize(engine).map_err(|e| format!("{}: {e}", engine.display()))?;
    let p = plan(r, &engine, out, game, logs)?;
    let kf = sibling(out, ".key");
    let key_ok =
        std::fs::read_to_string(&kf).is_ok_and(|t| t.split('\n').next() == Some(p.key.as_str()));
    if out.is_file() && key_ok && decoder_host_key(r, out).as_deref() == Some(p.decoder.as_str()) {
        return Ok(false);
    }
    let _ = std::fs::remove_file(&kf);
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let (prog, args) = p.cmd.split_first().ok_or("empty compile line")?;
    let res = r.run(prog, args, Some(&engine.join("dm/host")));
    match &res {
        Ok(c) => log(
            logs,
            "mac_host_build.log",
            &format!(
                "{}{}",
                String::from_utf8_lossy(&c.stdout),
                String::from_utf8_lossy(&c.stderr)
            ),
        ),
        Err(e) => {
            log(logs, "mac_host_build.log", e);
            return Err(format!(
                "{e} (full output: {})",
                logs.join("mac_host_build.log").display()
            ));
        }
    }
    let record = sibling(out, ".decoder.json");
    let part = sibling(out, ".decoder.json.partial");
    let binary = fingerprint(&[Item::Files(vec![out.to_path_buf()])], r);
    let text = format!(
        "{{\"format\": 1, \"binary\": \"{binary}\", \"inputs\": \"{}\"}}\n",
        p.decoder
    );
    std::fs::write(&part, text).map_err(|e| format!("{}: {e}", part.display()))?;
    std::fs::rename(&part, &record).map_err(|e| format!("{}: {e}", record.display()))?;
    std::fs::write(&kf, format!("{}\n", p.key)).map_err(|e| format!("{}: {e}", kf.display()))?;
    Ok(true)
}

/// The stage "Mac host" of `nxb build`: the host for the profile into `<work>/tools/dmhost` (the work folder
/// resolved, as the script resolves it). The button-icon font (`make_ext_font.py`) is not ported: it stays the work
/// folder's `font/`.
pub fn stage(engine: &Path, work: &Path, game: &str) -> Result<String, String> {
    std::fs::create_dir_all(work).map_err(|e| format!("{}: {e}", work.display()))?;
    let work = std::fs::canonicalize(work).map_err(|e| format!("{}: {e}", work.display()))?;
    let r = Runner::from_parent()?;
    let out = work.join("tools/dmhost");
    let built = build(&r, engine, &out, game, &work.join("logs"))?;
    Ok(format!(
        "{}{}",
        out.display(),
        if built {
            ""
        } else {
            " (reused: inputs unchanged)"
        }
    ))
}

/// The bytes before `__LINKEDIT` with the `LC_UUID` value zeroed: what two builds of the same compile line share (the
/// debug map's temporary object names and times, the UUID and the linker's signature differ). `None` when `b` is not
/// a thin 64-bit Mach-O.
pub fn code_bytes(b: &[u8]) -> Option<Vec<u8>> {
    let u32_at = |o: usize| {
        b.get(o..o + 4)
            .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    };
    let u64_at = |o: usize| {
        b.get(o..o + 8)
            .map(|s| u64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]))
    };
    if u32_at(0)? != 0xfeed_facf {
        return None;
    }
    let ncmds = u32_at(16)? as usize;
    let (mut off, mut uuid, mut linkedit) = (32usize, None, None);
    for _ in 0..ncmds {
        let (cmd, size) = (u32_at(off)?, u32_at(off + 4)? as usize);
        if cmd == 0x1b {
            uuid = Some(off + 8);
        }
        if cmd == 0x19 && b.get(off + 8..off + 24)?.starts_with(b"__LINKEDIT\0") {
            linkedit = Some(u64_at(off + 40)? as usize);
        }
        off = off.checked_add(size.max(8))?;
    }
    let mut v = b.get(..linkedit?)?.to_vec();
    if let Some(u) = uuid {
        v.get_mut(u..u + 16)?.fill(0);
    }
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_path_suffix() {
        assert_eq!(suffix("a.c"), ".c");
        assert_eq!(suffix(".c"), "");
        assert_eq!(suffix("a."), "");
        assert_eq!(suffix("x.tar.inc"), ".inc");
        assert!(code_bytes(b"not a mach-o").is_none());
    }

    /// Oracle: `NXB_ORACLE_ENGINE`, `NXB_ORACLE_GAME` and `NXB_ORACLE_HOST` (a `dmhost` the NSP script's
    /// `build_mac_host` built, with its `.key` and `.decoder.json`). The plan's key and decoder identity equal the
    /// script's, the script's build is reused, and a build into another folder has the same code.
    #[test]
    fn host_equals_the_reference_script() {
        let (Some(engine), Some(game), Some(host)) = (
            std::env::var_os("NXB_ORACLE_ENGINE"),
            std::env::var_os("NXB_ORACLE_GAME"),
            std::env::var_os("NXB_ORACLE_HOST"),
        ) else {
            return;
        };
        let (engine, host) = (Path::new(&engine), PathBuf::from(&host));
        let game = game.to_string_lossy().into_owned();
        let r = Runner::from_parent().unwrap();
        let logs = std::env::temp_dir().join(format!("nxb-host-logs-{}", std::process::id()));
        let engine = std::fs::canonicalize(engine).unwrap();
        let p = plan(&r, &engine, &host, &game, &logs).unwrap();
        let key = std::fs::read_to_string(sibling(&host, ".key")).unwrap();
        assert_eq!(format!("{}\n", p.key), key);
        assert_eq!(
            decoder_host_key(&r, &host).as_deref(),
            Some(p.decoder.as_str())
        );
        assert!(
            !build(&r, &engine, &host, &game, &logs).unwrap(),
            "the script's host is reused"
        );

        // NXB_ORACLE_HOST_OUT keeps this build (for the script to reuse it in turn)
        let keep = std::env::var_os("NXB_ORACLE_HOST_OUT").map(PathBuf::from);
        let d = keep.clone().unwrap_or_else(|| {
            std::env::temp_dir().join(format!("nxb-host-{}", std::process::id()))
        });
        let out = d.join("tools/dmhost");
        assert!(build(&r, &engine, &out, &game, &logs).unwrap());
        assert!(
            !build(&r, &engine, &out, &game, &logs).unwrap(),
            "its own build is reused"
        );
        let (a, b) = (std::fs::read(&host).unwrap(), std::fs::read(&out).unwrap());
        assert!(
            code_bytes(&a).is_some() && code_bytes(&a) == code_bytes(&b),
            "same code"
        );
        if keep.is_none() {
            std::fs::remove_dir_all(&d).unwrap();
        }
        let _ = std::fs::remove_dir_all(&logs);
    }
}
