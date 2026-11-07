//! `capture_nro`: an NRO its NRR registers and no file holds (`missing.tsv` of the collection). The game decodes it at
//! run time, so the rewritten modules boot with `DM_NRO_CAPTURE` (`dm/host/hle_ro.c`), which writes the image of the
//! first `LoadModule` the host has no rewritten copy of, named by its SHA-256, and ends the run. The image joins the
//! collected NROs (kept for the next build), and the rewrite runs again with it.

use std::path::Path;

use super::{Ctx, boot_mac, rmtree, set_env, tail_lines};

/// `NRO_CAPTURE_SECS`: the capture's time limit.
pub(crate) const NRO_CAPTURE_SECS: f64 = 120.0;

/// `missing.tsv` as the script's `dict(l.split('\t') for l in ... if l)`: SHA-256 to NRR, in file order.
pub(crate) fn parse_missing(text: &str) -> Result<Vec<(String, String)>, String> {
    let mut v: Vec<(String, String)> = Vec::new();
    for l in text.lines().filter(|l| !l.is_empty()) {
        let (h, n) = l
            .split_once('\t')
            .filter(|(_, n)| !n.contains('\t'))
            .ok_or_else(|| format!("missing.tsv: not two columns: {l}"))?;
        match v.iter_mut().find(|(k, _)| k == h) {
            Some(e) => e.1 = n.to_string(),
            None => v.push((h.to_string(), n.to_string())),
        }
    }
    Ok(v)
}

/// The NRO's name: `nro<n>` with `n` the manifest rows that are neither comments nor refused.
pub(crate) fn next_name(manifest: &str) -> String {
    let n = manifest
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("refused"))
        .count();
    format!("nro{n}")
}

/// The manifest with the captured NRO's row: `'\n'.join(rows + [row]) + '\n'` over the non-empty rows.
pub(crate) fn add_row(manifest: &str, name: &str, sha: &str, size: u64, nrr: &str) -> String {
    let mut rows: Vec<String> = manifest
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    rows.push(format!("{name}\t{sha}\t{size}\t{nrr}\tcapture"));
    rows.join("\n") + "\n"
}

/// `missing.tsv` again without the captured one.
pub(crate) fn missing_text(want: &[(String, String)]) -> String {
    want.iter().map(|(h, n)| format!("{h}\t{n}\n")).collect()
}

fn nros_in(cap: &Path) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = std::fs::read_dir(cap)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "nro"))
        .collect();
    v.sort();
    v
}

/// The capture when `nro_<package>/missing.tsv` names an NRO. Returns whether one was captured (the rewrite then runs
/// again). An error when the game loaded none of them.
pub(crate) fn capture_nro(c: &Ctx<'_>) -> Result<bool, String> {
    let nro_dir = c.work.join(format!("nro_{}", c.package));
    let missing = nro_dir.join("missing.tsv");
    let Ok(text) = std::fs::read_to_string(&missing) else {
        return Ok(false);
    };
    let mut want = parse_missing(&text)?;
    let Some(first) = want.first().map(|w| w.0.clone()) else {
        return Ok(false);
    };
    let host = c.host()?;
    let cap = nro_dir.join("capture.partial");
    rmtree(&cap)?;
    std::fs::create_dir_all(&cap).map_err(|e| format!("{}: {e}", cap.display()))?;
    let (rd, mut env) = c.run_dir("nro_run", false)?;
    let log = c.logs().join("nro_capture.log");
    c.p.say(&format!(
        "{} registered NRO(s) in no file: booting the game in the Mac host to capture what it decodes at run time",
        want.len()
    ));
    set_env(&mut env, "DM_NRO_CAPTURE", &cap);
    boot_mac(
        &host,
        &c.patched(),
        &env,
        &rd,
        &log,
        NRO_CAPTURE_SECS,
        &mut || !nros_in(&cap).is_empty(),
    )?;
    let got = nros_in(&cap);
    let stem = got
        .first()
        .and_then(|p| p.file_stem())
        .map(|s| s.to_string_lossy().into_owned());
    let Some(i) = stem
        .as_ref()
        .and_then(|s| want.iter().position(|w| &w.0 == s))
    else {
        c.p.say(&format!(
            "---- last lines of {} ----\n{}",
            log.display(),
            tail_lines(&log, 40)
        ));
        return Err(format!(
            "the game loaded none of the {} NRO(s) its NRR registers and no file holds, first {first} (host output: {})",
            want.len(),
            log.display()
        ));
    };
    let (sha, nrr) = want.remove(i);
    let manifest = nro_dir.join("nro.tsv");
    let mtext =
        std::fs::read_to_string(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let name = next_name(&mtext);
    let dst = nro_dir.join(format!("{name}.nro"));
    std::fs::rename(&got[0], &dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    let size = std::fs::metadata(&dst)
        .map_err(|e| format!("{}: {e}", dst.display()))?
        .len();
    std::fs::write(&manifest, add_row(&mtext, &name, &sha, size, &nrr))
        .map_err(|e| format!("{}: {e}", manifest.display()))?;
    std::fs::write(&missing, missing_text(&want))
        .map_err(|e| format!("{}: {e}", missing.display()))?;
    rmtree(&cap)?;
    rmtree(&rd)?;
    c.p.say(&format!(
        "NRO capture: {name}, {:.1} MB",
        size as f64 / f64::from(1u32 << 20)
    ));
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_rows_parse_in_order() {
        let w = parse_missing("bb\tb.nrr\naa\ta.nrr\n\n")
            .ok()
            .expect("parse");
        assert_eq!(
            w,
            [("bb".into(), "b.nrr".into()), ("aa".into(), "a.nrr".into())]
        );
        assert!(parse_missing("x\n").is_err());
        assert_eq!(missing_text(&w[1..]), "aa\ta.nrr\n");
        assert_eq!(missing_text(&[]), "");
    }

    #[test]
    fn the_capture_row_is_appended() {
        let m = "# name\tsha256\tsize\tnrr\twhere\nnro0\taa\t4\tx.nrr\tindex a\nrefused\tbb\t4\tx.nrr\tindex b\n";
        assert_eq!(next_name(m), "nro1");
        assert_eq!(
            add_row(m, "nro1", "cc", 9, "y.nrr"),
            "# name\tsha256\tsize\tnrr\twhere\nnro0\taa\t4\tx.nrr\tindex a\nrefused\tbb\t4\tx.nrr\tindex b\nnro1\tcc\t9\ty.nrr\tcapture\n"
        );
        assert_eq!(next_name("# h\n"), "nro0");
    }
}
