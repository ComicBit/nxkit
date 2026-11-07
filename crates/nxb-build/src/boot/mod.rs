//! The build steps that boot the game in the Mac host (`<work>/tools/dmhost`, headless and silent), as the NSP script
//! runs them: the NRO capture, the heap and address region checks of a provisional profile, the shader captures
//! (GLSLC, Vulkan, GL) and the Unreal shader decode. Each boot has its own run folder under `<work>/tmp`, its log
//! under `<work>/logs`, a time limit and a test that ends it early. Every step keeps its output where the next build
//! finds it and skips the boot while that output is complete.

pub(crate) mod captures;
pub(crate) mod checks;
pub(crate) mod inputs;
pub(crate) mod nro;
pub(crate) mod run;
pub(crate) mod uedec;

use std::ffi::OsString;
use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::events::Progress;
use crate::graph::Build;

/// What the boot steps share: the work folder (resolved), the profile and the package.
pub(crate) struct Ctx<'a> {
    /// The engine folder.
    pub(crate) engine: &'a Path,
    /// The work folder, resolved.
    pub(crate) work: PathBuf,
    /// The profile folder.
    pub(crate) profile: PathBuf,
    /// The game's title ID (`DM_GAME`, the profile the host is built for).
    pub(crate) game: String,
    /// The packed version (`v327680`).
    pub(crate) package: String,
    /// The progress events.
    pub(crate) p: &'a Progress,
}

impl<'a> Ctx<'a> {
    /// The context of build `b`.
    pub(crate) fn of(b: &'a Build, p: &'a Progress) -> Result<Ctx<'a>, String> {
        std::fs::create_dir_all(&b.work).map_err(|e| format!("{}: {e}", b.work.display()))?;
        let work =
            std::fs::canonicalize(&b.work).map_err(|e| format!("{}: {e}", b.work.display()))?;
        Ok(Ctx {
            engine: &b.engine,
            work,
            profile: b.profile.0.clone(),
            game: b.title.clone(),
            package: b.package.clone(),
            p,
        })
    }

    /// `<work>/logs`.
    pub(crate) fn logs(&self) -> PathBuf {
        self.work.join("logs")
    }

    /// `<work>/extract/<package>`.
    pub(crate) fn game_dir(&self) -> PathBuf {
        self.work.join("extract").join(&self.package)
    }

    /// `<work>/patched_<package>`.
    pub(crate) fn patched(&self) -> PathBuf {
        self.work.join(format!("patched_{}", self.package))
    }

    /// The Mac host for the profile, built into `<work>/tools/dmhost` unless the last build there is current
    /// (`build_mac_host`).
    pub(crate) fn host(&self) -> Result<PathBuf, String> {
        let r = crate::host::Runner::from_parent()?;
        let out = self.work.join("tools/dmhost");
        crate::host::machost::build(&r, self.engine, &out, &self.game, &self.logs())?;
        Ok(out)
    }

    /// A fresh run folder `<work>/tmp/<name>` with `save`, `temp` and (`msl`) `msl` inside, and the environment of a boot:
    /// the tools' environment ([`crate::host::Runner::from_parent`], with `DM_GAMES_DIR`), `DM_GAME`, `DM_ROOT` (the
    /// work folder), `WORK` (`<work>/tmp`), the packed version's RomFS (`DM_ROMFS`) and the run folder's parts
    /// (`DM_SAVE`, `DM_TEMP`, `DM_MSL_DIR`), as the script's `henv`.
    pub(crate) fn run_dir(
        &self,
        name: &str,
        msl: bool,
    ) -> Result<(PathBuf, Vec<(OsString, OsString)>), String> {
        let rd = self.work.join("tmp").join(name);
        rmtree(&rd)?;
        let parts: &[&str] = if msl {
            &["save", "temp", "msl"]
        } else {
            &["save", "temp"]
        };
        for x in parts {
            std::fs::create_dir_all(rd.join(x)).map_err(|e| format!("{}: {e}", rd.display()))?;
        }
        let r = crate::host::Runner::from_parent()?;
        let mut env: Vec<(OsString, OsString)> = r
            .env()
            .vars()
            .map(|(k, v)| (k.to_os_string(), v.to_os_string()))
            .collect();
        env.push(("DM_GAME".into(), self.game.clone().into()));
        env.push(("DM_ROOT".into(), self.work.clone().into()));
        env.push(("WORK".into(), self.work.join("tmp").into()));
        env.push(("DM_ROMFS".into(), self.game_dir().join("RomFS.img").into()));
        env.push(("DM_SAVE".into(), rd.join("save").into()));
        env.push(("DM_TEMP".into(), rd.join("temp").into()));
        env.push(("DM_MSL_DIR".into(), rd.join("msl").into()));
        Ok((rd, env))
    }
}

/// Sets `k` in a boot environment (a later value replaces an earlier one).
pub(crate) fn set_env(env: &mut Vec<(OsString, OsString)>, k: &str, v: impl Into<OsString>) {
    env.retain(|(n, _)| n != k);
    env.push((k.into(), v.into()));
}

/// Removes a folder or file, a missing one is no error.
pub(crate) fn rmtree(p: &Path) -> Result<(), String> {
    let r = if p.is_dir() && !p.is_symlink() {
        std::fs::remove_dir_all(p)
    } else {
        std::fs::remove_file(p)
    };
    match r {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("{}: {e}", p.display())),
    }
}

/// `swap_in`: the new output in place of the old one, as a whole.
pub(crate) fn swap_in(tmp: &Path, dst: &Path) -> Result<(), String> {
    rmtree(dst)?;
    std::fs::rename(tmp, dst).map_err(|e| format!("{}: {e}", dst.display()))
}

/// `hms`: seconds as `m:ss`, from an hour on `h:mm:ss`.
pub(crate) fn hms(s: f64) -> String {
    let s = s.max(0.0) as u64;
    let (h, m) = (s / 3600, s / 60 % 60);
    if h > 0 {
        format!("{h}:{m:02}:{:02}", s % 60)
    } else {
        format!("{m}:{:02}", s % 60)
    }
}

/// The last `n` lines of a file, joined, without the final newline (`collections.deque(f, maxlen=n)`).
pub(crate) fn tail_lines(p: &Path, n: usize) -> String {
    let Ok(f) = std::fs::File::open(p) else {
        return String::new();
    };
    let mut q: std::collections::VecDeque<String> = std::collections::VecDeque::new();
    let mut r = BufReader::new(f);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match r.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                q.push_back(String::from_utf8_lossy(&buf).into_owned());
                if q.len() > n {
                    q.pop_front();
                }
            }
        }
    }
    q.into_iter()
        .collect::<String>()
        .trim_end_matches('\n')
        .to_string()
}

/// A log read as text (bytes that are not UTF-8 replaced), empty when it is not there.
pub(crate) fn read_log(p: &Path) -> String {
    std::fs::read(p)
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default()
}

/// `host_stopped`: the red line of a capture run whose host stopped by itself before the capture was complete.
pub(crate) fn host_stopped(log: &Path, rc: i32, secs: f64) -> String {
    format!(
        "[RED] the game stopped after {secs:.0} s in the Mac host (exit {rc}): its shaders are incomplete and the IPA will not get further; ---- last lines of {} ----\n{}",
        log.display(),
        tail_lines(log, 12)
    )
}

/// How a boot ended.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Ran {
    /// The host's exit code when it stopped by itself (a signal is `-<signal>`), `None` when it was still running.
    pub(crate) rc: Option<i32>,
    /// Seconds it ran.
    pub(crate) secs: f64,
}

fn code_of(s: std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    s.code().or_else(|| s.signal().map(|n| -n)).unwrap_or(-1)
}

/// `boot_mac`: boots `host <patched>` in `rd` with exactly `env` (headless and silent: no `DM_WINDOW`, no
/// `DM_AUDIO`), its output in `log`, until `done()` says so, the host exits, or `secs` pass. Then `SIGTERM` to its
/// process group (the host's graceful exit) and `SIGKILL` 5 s later. An interrupt of nxb stops it and is an error.
pub(crate) fn boot_mac(
    host: &Path,
    patched: &Path,
    env: &[(OsString, OsString)],
    rd: &Path,
    log: &Path,
    secs: f64,
    done: &mut dyn FnMut() -> bool,
) -> Result<Ran, String> {
    if let Some(d) = log.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let f = std::fs::File::create(log).map_err(|e| format!("{}: {e}", log.display()))?;
    let f2 = f
        .try_clone()
        .map_err(|e| format!("{}: {e}", log.display()))?;
    let t0 = Instant::now();
    let mut child = std::process::Command::new(host)
        .arg(patched)
        .current_dir(rd)
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .stdin(std::process::Stdio::null())
        .stdout(f)
        .stderr(f2)
        .process_group(0)
        .spawn()
        .map_err(|e| format!("{}: {e}", host.display()))?;
    let pid = child.id();
    let mut interrupted = false;
    let mut rc = None;
    loop {
        if let Ok(Some(s)) = child.try_wait() {
            rc = Some(code_of(s));
            break;
        }
        if done() || t0.elapsed().as_secs_f64() >= secs {
            break;
        }
        if nxb_sys::stdio::interrupted().is_some() {
            interrupted = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    if rc.is_none() {
        if let Ok(Some(s)) = child.try_wait() {
            rc = Some(code_of(s));
        }
    }
    if rc.is_none() {
        let _ = nxb_sys::process::kill_group(pid, nxb_sys::process::Signal::Term);
        let t1 = Instant::now();
        let mut gone = false;
        while t1.elapsed() < Duration::from_secs(5) {
            if let Ok(Some(_)) = child.try_wait() {
                gone = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        if !gone {
            let _ = nxb_sys::process::kill_group(pid, nxb_sys::process::Signal::Kill);
            let _ = child.wait();
        }
    }
    // anything the game started in the host's group goes with it
    let _ = nxb_sys::process::kill_group(pid, nxb_sys::process::Signal::Kill);
    if interrupted {
        return Err("interrupted".into());
    }
    Ok(Ran {
        rc,
        secs: t0.elapsed().as_secs_f64(),
    })
}

/// `captured_nros(patched)`: the NRO folder when its `nro.tsv` has a row the NRO capture added.
pub(crate) fn captured_nros(nro_dir: &Path) -> Option<PathBuf> {
    let t = std::fs::read_to_string(nro_dir.join("nro.tsv")).ok()?;
    t.split('\n')
        .any(|l| l.ends_with("\tcapture"))
        .then(|| nro_dir.to_path_buf())
}

#[cfg(test)]
mod tests;
