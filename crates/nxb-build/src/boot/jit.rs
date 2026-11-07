//! The JIT capture (`capture_jit`) of a game that makes code at run time through `nn::jit` (Super Mario 64 of Super
//! Mario 3D All-Stars). The iPhone runs no such code. It serves what the game's JIT plugin made from an image. The game
//! boots twice in the Mac host (headless and silent) with `DM_JIT_CAPTURE`, each run into its own folder under
//! `<work>/extract/<package>/jit_capture` (`boot1`, `boot2`). Two runs in different layouts tell which output words are
//! host addresses. A third run plays the profile's `jit_route` (`route`). Every other folder there is added too, such
//! as `play` from a run on the Mac. A folder made with other rewritten modules or request keys is cleared
//! (`jit_capture/stamp.json`). [`nxb_rewrite::jit`] builds the image into `<work>/extract/<package>/jit`, where the app
//! build takes it.

use std::path::{Path, PathBuf};
use std::time::Instant;

use nxb_pyjson::Value;
use nxb_rewrite::jit::{Keys, Record};

use super::{Ctx, boot_mac, host_stopped, rmtree, set_env, swap_in, tail_lines};
use crate::events::{Progress, Update};
use crate::graph::Build;
use crate::host::profile::Game;
use crate::host::pyfmt::{int0, strip};

/// `JIT_SECS`: the longest boot run. Super Mario 64 asks for its 14.2 MB of boot code 3 s after start.
pub(crate) const JIT_SECS: f64 = 90.0;
/// `JIT_ROUTE_SECS`: how long the run of the profile's `jit_route` takes.
pub(crate) const JIT_ROUTE_SECS: f64 = 130.0;
/// A boot run ends once it holds a request larger than this (the boot code) and no new one came for
/// [`JIT_QUIET_SECS`].
const BOOT_CODE: u64 = 1 << 20;
/// See [`BOOT_CODE`].
const JIT_QUIET_SECS: f64 = 10.0;

/// `uses_jit`: main imports `nn::jit::CreateJitEnvironment`. False without `<exefs>/main`.
pub(crate) fn uses_jit(exefs: &Path) -> Result<bool, String> {
    let main = exefs.join("main");
    if !main.is_file() {
        return Ok(false);
    }
    let (img, ..) = crate::host::guess::read_nso(&main)?;
    Ok(crate::host::nsofacts::dynsyms(&img)?
        .iter()
        .any(|s| !s.1 && s.0.starts_with("_ZN2nn3jit20CreateJitEnvironment")))
}

/// `json.dumps` with `ensure_ascii=True`: every character above ASCII as `\uXXXX` (UTF-16).
fn ensure_ascii(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch.is_ascii() {
            out.push(ch);
        } else {
            let mut b = [0u16; 2];
            for u in ch.encode_utf16(&mut b) {
                out.push_str(&format!("\\u{u:04x}"));
            }
        }
    }
    out
}

/// `jit_stamp`: what the captures depend on. The size of every rewritten module (the layout of the image) and the
/// profile's request keys.
pub(crate) fn stamp(patched: &Path, g: &Game) -> String {
    let mut sizes: Vec<(String, u64)> = std::fs::read_dir(patched)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    let size = std::fs::metadata(e.path()).ok()?.len();
                    name.ends_with(".patched.bin").then_some((name, size))
                })
                .collect()
        })
        .unwrap_or_default();
    sizes.sort();
    let modules = sizes
        .into_iter()
        .map(|(n, s)| {
            Value::List(vec![
                Value::Str(n),
                Value::Int(i64::try_from(s).unwrap_or(i64::MAX)),
            ])
        })
        .collect();
    let v = Value::obj([
        ("modules", Value::List(modules)),
        ("jit_key_data", Value::str(g.get_or("jit_key_data", ""))),
        ("jit_out_code", Value::str(g.get_or("jit_out_code", ""))),
        ("jit_route", Value::str(g.get_or("jit_route", ""))),
    ]);
    ensure_ascii(&nxb_pyjson::dumps(&v))
}

/// `int(s)`: a decimal number.
fn int10(s: &str) -> Result<i128, String> {
    strip(s)
        .parse()
        .map_err(|_| format!("invalid literal for int() with base 10: {s:?}"))
}

/// `key_masks` and `out_codes` of `jit_capture.py`: the profile keys `jit_key_data` (`4:0-7,24-27`) and
/// `jit_out_code` (`4:entries=4,count=0x20004,size=0x20000`).
pub(crate) fn keys(g: &Game) -> Result<Keys, String> {
    let mut k = Keys::default();
    for item in g.words_or("jit_key_data", "") {
        let bad = || format!("jit_key_data: {item} is no <command>:<bytes>");
        let [cmd, spans] = item.split(':').collect::<Vec<_>>()[..] else {
            return Err(bad());
        };
        let mut m = [0u8; 32];
        for sp in spans.split(',') {
            let (first, last) = sp.split_once('-').unwrap_or((sp, ""));
            let a = int0(first)?;
            let z = int0(if last.is_empty() { first } else { last })?;
            for i in a..=z {
                *usize::try_from(i)
                    .ok()
                    .and_then(|i| m.get_mut(i))
                    .ok_or_else(|| {
                        format!("jit_key_data: byte {i} of {item} is outside the 32 request bytes")
                    })? = 0xff;
            }
        }
        k.masks.insert(int10(cmd)?, m);
    }
    for item in g.words_or("jit_out_code", "") {
        let bad = || format!("jit_out_code: {item} is no <command>:entries=,count=,size=");
        let [cmd, kv] = item.split(':').collect::<Vec<_>>()[..] else {
            return Err(bad());
        };
        let mut d: Vec<(&str, &str)> = Vec::new();
        for x in kv.split(',') {
            let [n, v] = x.split('=').collect::<Vec<_>>()[..] else {
                return Err(bad());
            };
            d.retain(|(o, _)| *o != n);
            d.push((n, v));
        }
        let get = |n: &str| {
            d.iter()
                .find(|(o, _)| *o == n)
                .ok_or_else(bad)
                .and_then(|(_, v)| int0(v))
        };
        k.out_codes
            .insert(int10(cmd)?, (get("entries")?, get("count")?, get("size")?));
    }
    Ok(k)
}

/// The names in `d` that end in `.jit` (hidden ones too), sorted, with their sizes.
fn jit_files(d: &Path) -> Vec<(String, u64)> {
    let mut v: Vec<(String, u64)> = std::fs::read_dir(d)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    let size = std::fs::metadata(e.path()).ok()?.len();
                    name.ends_with(".jit").then_some((name, size))
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// A run's folder holds the boot code: a request larger than [`BOOT_CODE`].
fn has_boot_code(d: &Path) -> bool {
    jit_files(d).iter().any(|(_, s)| *s > BOOT_CODE)
}

/// Clears `caps` when its `stamp.json` is missing or differs from `stamp`, then writes the stamp.
pub(crate) fn clear_stale(caps: &Path, stamp: &str, p: &Progress) -> Result<(), String> {
    let sf = caps.join("stamp.json");
    if caps.exists() && std::fs::read_to_string(&sf).ok().as_deref() != Some(stamp) {
        p.say(&format!(
            "[RED] {}: made with other rewritten modules or request keys; cleared (play captures included: play the game on the Mac again for them)",
            caps.display()
        ));
        rmtree(caps)?;
    }
    std::fs::create_dir_all(caps).map_err(|e| format!("{}: {e}", caps.display()))?;
    std::fs::write(&sf, stamp).map_err(|e| format!("{}: {e}", sf.display()))
}

/// The capture folders in `caps`: folders whose name does not end in `.partial`, sorted.
pub(crate) fn capture_dirs(caps: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(caps)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|x| {
                    x.is_dir()
                        && !x
                            .file_name()
                            .is_some_and(|n| n.to_string_lossy().ends_with(".partial"))
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|x| x.to_string_lossy().into_owned());
    v
}

/// The requests in the capture folders (`read`), in folder order and by name in each.
fn read_records(dirs: &[PathBuf]) -> Result<Vec<Record>, String> {
    let mut records = Vec::new();
    for d in dirs {
        for (name, _) in jit_files(d) {
            let f = d.join(&name);
            let b = std::fs::read(&f).map_err(|e| format!("{}: {e}", f.display()))?;
            records.push(Record::parse(&f.display().to_string(), &b).map_err(|e| e.to_string())?);
        }
    }
    Ok(records)
}

/// `jit_capture.py <dirs> <out>`: the image of the capture folders in `<out>` (`jit.img`, `jit.out`, `jit.tsv`, made in
/// `tmp` and swapped in), its notes and summary in `log`. Returns the summary.
pub(crate) fn image(
    dirs: &[PathBuf],
    keys: &Keys,
    tmp: &Path,
    out: &Path,
    log: &Path,
) -> Result<String, String> {
    let mut notes = Vec::new();
    let res = read_records(dirs)
        .and_then(|r| nxb_rewrite::jit::build(r, keys, &mut notes).map_err(|e| e.to_string()));
    match &res {
        Ok(img) => notes.push(img.summary.clone()),
        Err(e) => notes.push(format!("[RED] {e}")),
    }
    let text = notes.join("\n") + "\n";
    if let Some(d) = log.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    std::fs::write(log, text).map_err(|e| format!("{}: {e}", log.display()))?;
    let img = res.map_err(|e| format!("the JIT image: {e} (full output: {})", log.display()))?;
    rmtree(tmp)?;
    std::fs::create_dir_all(tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    for (n, b) in [("jit.img", &img.img), ("jit.out", &img.out)] {
        std::fs::write(tmp.join(n), b).map_err(|e| format!("{}: {e}", tmp.join(n).display()))?;
    }
    std::fs::write(tmp.join("jit.tsv"), &img.tsv).map_err(|e| format!("{}: {e}", tmp.display()))?;
    if !img.disagree.is_empty() {
        return Err(format!(
            "the JIT image: {} requests differ between captures (full output: {})",
            img.disagree.len(),
            log.display()
        ));
    }
    swap_in(tmp, out)?;
    Ok(img.summary)
}

/// `capture_jit`: the boot runs that are missing, then the image. Returns the stage's summary.
pub(crate) fn capture(c: &Ctx<'_>, g: &Game) -> Result<String, String> {
    let d = c.game_dir();
    let (caps, out) = (d.join("jit_capture"), d.join("jit"));
    clear_stale(&caps, &stamp(&c.patched(), g), c.p)?;
    let keys = keys(g)?;
    let route = g.get_or("jit_route", "");
    let mut names = vec!["boot1", "boot2"];
    if !route.is_empty() {
        names.push("route");
    }
    let mut host = None;
    for name in names {
        let cd = caps.join(name);
        if !jit_files(&cd).is_empty() {
            continue;
        }
        if host.is_none() {
            c.p.set(Update {
                step: Some("building the Mac host"),
                ..Update::default()
            });
            host = Some(c.host()?);
        }
        let Some(host) = host.as_ref() else {
            continue;
        };
        let part = caps.join(format!("{name}.partial"));
        let log = c.logs().join(format!("jit_{name}.log"));
        rmtree(&part)?;
        let (rd, mut env) = c.run_dir("jit_run", true)?;
        std::fs::create_dir_all(&part).map_err(|e| format!("{}: {e}", part.display()))?;
        let romfs = if d.join("RomFS.img").is_file() {
            d.join("RomFS.img")
        } else {
            d.join("RomFS")
        };
        set_env(&mut env, "DM_ROMFS", romfs);
        set_env(&mut env, "DM_MSL_DIR", c.work.join("shader"));
        set_env(&mut env, "DM_JIT_CAPTURE", &part);
        if name == "route" {
            set_env(&mut env, "DM_KEYS", route.as_str());
        }
        c.p.set(Update {
            step: Some(&format!("booting the game ({name})")),
            ..Update::default()
        });
        c.p.say(&format!(
            "booting the game in the Mac host to capture the code its JIT plugin makes ({name}) → {}",
            cd.display()
        ));
        let t0 = Instant::now();
        let (mut last_n, mut last_t) = (0usize, 0.0f64);
        let route_run = name == "route";
        let ran = boot_mac(
            host,
            &c.patched(),
            &env,
            &rd,
            &log,
            if route_run { JIT_ROUTE_SECS } else { JIT_SECS },
            &mut || {
                if route_run {
                    return false;
                }
                let fs = jit_files(&part);
                let now = t0.elapsed().as_secs_f64();
                if fs.len() != last_n {
                    (last_n, last_t) = (fs.len(), now);
                }
                fs.iter().any(|(_, s)| *s > BOOT_CODE) && now - last_t > JIT_QUIET_SECS
            },
        )?;
        if !has_boot_code(&part) {
            if let Some(rc) = ran.rc {
                c.p.say(&host_stopped(&log, rc, ran.secs));
            }
            return Err(format!(
                "the JIT capture ({name}) holds no boot code (host output: {})",
                log.display()
            ));
        }
        swap_in(&part, &cd)?;
        rmtree(&rd)?;
    }
    c.p.set(Update {
        step: Some("image"),
        ..Update::default()
    });
    let log = c.logs().join("jit_capture.log");
    let r = image(
        &capture_dirs(&caps),
        &keys,
        &d.join("jit.partial"),
        &out,
        &log,
    );
    if r.is_err() {
        c.p.say(&format!(
            "---- last lines of {} ----\n{}",
            log.display(),
            tail_lines(&log, 40)
        ));
    }
    Ok(r?.chars().take(120).collect())
}

/// The "JIT capture" stage of an iOS build: [`capture`] when main imports `nn::jit`.
pub(crate) fn stage(b: &Build, p: &Progress) -> Result<String, String> {
    let c = Ctx::of(b, p)?;
    if !uses_jit(&c.game_dir().join("ExeFS"))? {
        return Ok("not needed: main does not import nn::jit".into());
    }
    let g = Game::open(&c.profile)?;
    capture(&c, &g)
}

#[cfg(test)]
mod tests;

/// A small uncompressed NSO whose one dynamic symbol is `name` (undefined unless `defined`).
#[cfg(test)]
pub(crate) fn test_main(name: &str, defined: bool) -> Vec<u8> {
    let mut img = vec![0u8; 0x200];
    let put = |img: &mut Vec<u8>, o: usize, v: &[u8]| img[o..o + v.len()].copy_from_slice(v);
    put(&mut img, 4, &0x10u32.to_le_bytes());
    put(&mut img, 0x10, b"MOD0");
    put(&mut img, 0x14, &0x20u32.to_le_bytes());
    for (i, (t, v)) in [(5u64, 0x100u64), (6, 0x80)].iter().enumerate() {
        put(&mut img, 0x30 + i * 16, &t.to_le_bytes());
        put(&mut img, 0x38 + i * 16, &v.to_le_bytes());
    }
    put(&mut img, 0x98, &1u32.to_le_bytes());
    img[0x9c] = 0x12;
    put(&mut img, 0x9e, &u16::from(defined).to_le_bytes());
    put(&mut img, 0x101, name.as_bytes());
    let mut nso = vec![0u8; 0x100];
    nso[..4].copy_from_slice(b"NSO0");
    for (i, (fo, mo, sz)) in [
        (0x100u32, 0u32, 0x200u32),
        (0x300, 0x1000, 0),
        (0x300, 0x1000, 0),
    ]
    .iter()
    .enumerate()
    {
        put(&mut nso, 0x10 + i * 0x10, &fo.to_le_bytes());
        put(&mut nso, 0x14 + i * 0x10, &mo.to_le_bytes());
        put(&mut nso, 0x18 + i * 0x10, &sz.to_le_bytes());
    }
    nso.extend_from_slice(&img);
    nso
}
