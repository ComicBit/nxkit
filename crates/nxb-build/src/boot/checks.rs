//! The heap check and the address region check (`heap_check`, `region_check` of the NSP script): the game boots in
//! the Mac host for a few seconds, and what its log and region table show goes into the profile, which is stamped so
//! a later build of the same rewrite skips the boot.

use std::path::Path;

use super::{Ctx, boot_mac, hms, read_log, rmtree, set_env, tail_lines};
use crate::host::survey_profile as sp;

/// `HEAP_CHECK_SECS`: the heap check's time limit.
pub(crate) const HEAP_CHECK_SECS: f64 = 30.0;
/// `HEAP_CHECK_NNMAIN_SECS`: how long the game may run in nnMain without a heap request.
pub(crate) const HEAP_CHECK_NNMAIN_SECS: f64 = 5.0;
/// `REGION_CHECK_NNMAIN_SECS`: how long the region check lets the game run in nnMain.
pub(crate) const REGION_CHECK_NNMAIN_SECS: u32 = 15;
/// `SWITCH_ALIAS`: the region check's alias region, Switch-sized so whatever the game reserves fits.
pub(crate) const SWITCH_ALIAS: u64 = 0x10_0000_0000;

/// The local date as `time.strftime("%Y-%m-%d")` writes it.
pub(crate) fn today() -> String {
    let local = std::process::Command::new("/bin/date")
        .arg("+%Y-%m-%d")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| s.len() == 10);
    local.unwrap_or_else(|| {
        let days = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() / 86400)
            .unwrap_or(0) as i64;
        civil(days)
    })
}

/// Days since 1970-01-01 as `YYYY-MM-DD` (UTC).
fn civil(z: i64) -> String {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// `rewrite_id(patched)`: the short key of the rewritten modules, the first 12 hex digits of the SHA-256 over each
/// `*.patched.bin` file's name and bytes in name order. The script takes the first line of `patched_<package>.key`,
/// which the Rust rewrite does not write. Either way a new rewrite gets a new key and is checked again. Empty before
/// a rewrite.
pub(crate) fn rewrite_id(patched: &Path) -> String {
    let Ok(rd) = std::fs::read_dir(patched) else {
        return String::new();
    };
    let mut names: Vec<String> = rd
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".patched.bin"))
        .collect();
    if names.is_empty() {
        return String::new();
    }
    names.sort();
    let mut h = nxb_model::Sha256::new();
    for n in &names {
        let Ok(b) = std::fs::read(patched.join(n)) else {
            return String::new();
        };
        h.update(n.as_bytes());
        h.update(&[0]);
        h.update(&(b.len() as u64).to_le_bytes());
        h.update(&b);
    }
    h.finalize().to_hex()[..12].to_string()
}

fn write_atomic(p: &Path, text: &str) -> Result<(), String> {
    let part = p.with_extension("tsv.partial");
    std::fs::write(&part, text).map_err(|e| format!("{}: {e}", part.display()))?;
    std::fs::rename(&part, p).map_err(|e| format!("{}: {e}", p.display()))
}

/// `done()` of the heap check: the first heap request answered or refused, or nnMain running for
/// [`HEAP_CHECK_NNMAIN_SECS`] without one. `nnmain` keeps when nnMain was first seen.
pub(crate) fn heap_done(text: &str, nnmain: &mut Option<f64>, now: f64) -> bool {
    if text.contains("SetHeapSize OOM") || text.contains("[mem] heap ") {
        return true;
    }
    if text.contains(">>> nnMain") && nnmain.is_none() {
        *nnmain = Some(now);
    }
    now - nnmain.unwrap_or(now) > HEAP_CHECK_NNMAIN_SECS
}

/// `done()` of the region check: nnMain running for [`REGION_CHECK_NNMAIN_SECS`].
pub(crate) fn region_done(text: &str, nnmain: &mut Option<f64>, now: f64) -> bool {
    if text.contains(">>> nnMain") && nnmain.is_none() {
        *nnmain = Some(now);
    }
    now - nnmain.unwrap_or(now) > f64::from(REGION_CHECK_NNMAIN_SECS)
}

/// `heap_check`: the heap check of a provisional profile whose game has virtual address memory and sets a heap. The
/// game boots without shader libraries, with the profile's shader map and alias region, until its first heap request
/// is answered or fails. The keys the log shows go into the profile, which is stamped.
pub(crate) fn heap_check(c: &Ctx<'_>) -> Result<(), String> {
    let gtsv = c.profile.join("game.tsv");
    let text = std::fs::read_to_string(&gtsv).map_err(|e| format!("{}: {e}", gtsv.display()))?;
    let alias = sp::keys_of(&text)
        .get("mem_alias")
        .cloned()
        .ok_or("the profile has no mem_alias")?;
    c.p.say("heap check: booting the game in the Mac host");
    let host = c.host()?;
    let (rd, mut env) = c.run_dir("heap_run", true)?;
    let log = c.logs().join("heap_check.log");
    set_env(&mut env, "DM_NVN_SHMAP", c.profile.join("shader_map.tsv"));
    set_env(&mut env, "DM_ALIAS_SIZE", alias);
    let t0 = std::time::Instant::now();
    let mut seen = None;
    let ran = boot_mac(
        &host,
        &c.patched(),
        &env,
        &rd,
        &log,
        HEAP_CHECK_SECS,
        &mut || heap_done(&read_log(&log), &mut seen, t0.elapsed().as_secs_f64()),
    )?;
    let ltext = read_log(&log);
    rmtree(&rd)?;
    let text = std::fs::read_to_string(&gtsv).map_err(|e| format!("{}: {e}", gtsv.display()))?;
    let r = sp::heap_check_result(&text, &ltext, ran.rc, &today())?;
    for (k, (v, why)) in &r.set {
        c.p.say(&format!("heap check: {k} {v}: {why}"));
    }
    write_atomic(&gtsv, &r.text)?;
    let heap = ltext.contains("[mem] heap ") && r.how.starts_with("heap ");
    if !heap && !ltext.contains(">>> nnMain") && !ltext.contains("SetHeapSize OOM") {
        c.p.say(&format!(
            "[RED] heap check: {}; ---- last lines of {} ----\n{}",
            r.how,
            log.display(),
            tail_lines(&log, 20)
        ));
    }
    let set = r.set.iter().map(|s| s.0.as_str()).collect::<Vec<_>>();
    c.p.say(&format!(
        "heap check: {} in {}{}",
        r.how,
        hms(t0.elapsed().as_secs_f64()),
        if set.is_empty() {
            String::new()
        } else {
            format!("; set {}", set.join(", "))
        }
    ));
    Ok(())
}

/// `region_check`: the address region check of a profile whose game has virtual address memory. The game boots with
/// a Switch-sized alias region until it has run [`REGION_CHECK_NNMAIN_SECS`] in nnMain. The host's region table
/// (`DM_REGIONS`) says which regions the game took and where they reach against the profile's alias region. A red
/// line when they reach past it. The one fix it makes is `stdalloc_cap`. The profile is stamped with the rewrite's key.
pub(crate) fn region_check(c: &Ctx<'_>, exefs: &Path) -> Result<(), String> {
    let gtsv = c.profile.join("game.tsv");
    c.p.say("address regions: booting the game in the Mac host");
    let host = c.host()?;
    let (rd, mut env) = c.run_dir("region_run", true)?;
    let log = c.logs().join("region_check.log");
    let table = c.logs().join("regions.tsv");
    rmtree(&table)?;
    set_env(&mut env, "DM_NVN_SHMAP", c.profile.join("shader_map.tsv"));
    set_env(&mut env, "DM_ALIAS_SIZE", format!("{SWITCH_ALIAS:#x}"));
    set_env(&mut env, "DM_REGIONS", &table);
    let t0 = std::time::Instant::now();
    let mut seen = None;
    let secs = HEAP_CHECK_SECS + f64::from(REGION_CHECK_NNMAIN_SECS);
    let ran = boot_mac(&host, &c.patched(), &env, &rd, &log, secs, &mut || {
        region_done(&read_log(&log), &mut seen, t0.elapsed().as_secs_f64())
    })?;
    rmtree(&rd)?;
    let ltext = read_log(&log);
    let rows = std::fs::read(&table)
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default();
    let sdk = exefs.join("sdk");
    let syms = if sdk.exists() {
        let (img, ..) = crate::host::guess::read_nso(&sdk)?;
        crate::host::nsofacts::defined(&img)
    } else {
        Vec::new()
    };
    let text = std::fs::read_to_string(&gtsv).map_err(|e| format!("{}: {e}", gtsv.display()))?;
    let before = sp::keys_of(&text);
    let r = sp::region_check_result(
        &text,
        &ltext,
        &rows,
        &syms,
        ran.rc,
        REGION_CHECK_NNMAIN_SECS,
        &rewrite_id(&c.patched()),
        &today(),
    )?;
    let after = sp::keys_of(&r.text);
    for (k, (v, why)) in crate::host::guess_check::depth_signal(&ltext, &before) {
        c.p.say(&format!("address regions: {k} {v}: {why}"));
    }
    match (before.get("stdalloc_cap"), after.get("stdalloc_cap")) {
        (a, Some(b)) if a != Some(b) => {
            let n = crate::host::pyfmt::int0(b)? as f64;
            c.p.say(&format!(
                "address regions: stdalloc_cap {b} ({:.2} GB) set in {}",
                n / f64::from(1u32 << 30),
                gtsv.display()
            ));
        }
        (Some(_), None) => c.p.say(
            "address regions: the stdalloc_cap an earlier check set is no longer needed: removed",
        ),
        _ => {}
    }
    for (red, t) in &r.found {
        c.p.say(&format!(
            "{}address regions: {t}",
            if *red { "[RED] " } else { "" }
        ));
    }
    write_atomic(&gtsv, &r.text)?;
    let how = r
        .text
        .lines()
        .rev()
        .find_map(|l| l.strip_prefix("# address regions: "))
        .unwrap_or("")
        .to_string();
    c.p.say(&format!(
        "address regions: {how} in {}",
        hms(t0.elapsed().as_secs_f64())
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates() {
        assert_eq!(civil(0), "1970-01-01");
        assert_eq!(civil(20_454), "2026-01-01");
        assert_eq!(civil(19_782), "2024-02-29");
        assert_eq!(today().len(), 10);
    }

    #[test]
    fn heap_done_waits_for_a_request_or_nnmain() {
        let mut s = None;
        assert!(!heap_done("boot", &mut s, 1.0));
        assert!(heap_done("x\n[mem] heap 1.44 GB\n", &mut s, 1.0));
        assert!(heap_done("SetHeapSize OOM", &mut s, 1.0));
        assert!(!heap_done(">>> nnMain", &mut s, 2.0));
        assert_eq!(s, Some(2.0));
        assert!(!heap_done(">>> nnMain", &mut s, 7.0));
        assert!(heap_done(">>> nnMain", &mut s, 7.1));
    }

    #[test]
    fn region_done_after_15_s_in_nnmain() {
        let mut s = None;
        assert!(!region_done("boot", &mut s, 40.0));
        assert!(!region_done(">>> nnMain", &mut s, 41.0));
        assert!(!region_done(">>> nnMain", &mut s, 56.0));
        assert!(region_done(">>> nnMain", &mut s, 56.5));
    }

    #[test]
    fn rewrite_id_follows_the_modules() {
        let d = std::env::temp_dir().join(format!("nxb-rid-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).ok().expect("dir");
        assert_eq!(rewrite_id(&d), "");
        std::fs::write(d.join("main.patched.bin"), b"abc")
            .ok()
            .expect("w");
        std::fs::write(d.join("nro.tsv"), b"x").ok().expect("w");
        let a = rewrite_id(&d);
        assert_eq!(a.len(), 12);
        std::fs::write(d.join("nro.tsv"), b"y").ok().expect("w");
        assert_eq!(rewrite_id(&d), a);
        std::fs::write(d.join("main.patched.bin"), b"abd")
            .ok()
            .expect("w");
        assert_ne!(rewrite_id(&d), a);
        let _ = std::fs::remove_dir_all(&d);
    }
}
