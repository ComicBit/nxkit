//! `nxb regressions <library dir> <chain version>`: `shader_regressions.py`'s check. The shaders that translated in the
//! previous build of the same game (the newest other `b-*` folder with a build report) and fail in `b-<chain version>`.
//! Exit codes: 0 none (or nothing to compare with), 1 regressions (one `[RED]` line each), 2 no report for this chain.
//! The chain version is an argument here (the script computes it from the engine's sources). Libraries kept as
//! images are not read.

use nxb_pyjson::Value;
use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const REPORT: &str = "_b_build.json";

fn rows(path: &Path) -> Result<Vec<Value>, String> {
    let t = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    match nxb_pyjson::parse(&t).map_err(|e| format!("{}: {e:?}", path.display()))? {
        Value::List(l) => Ok(l),
        _ => Err(format!("{}: not a list", path.display())),
    }
}

fn game_of(dir: &Path) -> Option<String> {
    let t = std::fs::read_to_string(dir.join("inputs.tsv")).ok()?;
    t.lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.split_once('\t'))
        .find(|(k, _)| *k == "game")
        .map(|(_, v)| v.to_string())
}

/// The newest `b-*` folder beside `b-<chain>` with a build report, of the same game when both name one.
fn previous(lib: &Path, chain: &str) -> Option<String> {
    let me = format!("b-{chain}");
    let game = game_of(&lib.join(&me));
    let mut found: Vec<(SystemTime, String)> = Vec::new();
    for e in std::fs::read_dir(lib).ok()?.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let path = lib.join(&name).join(REPORT);
        if !name.starts_with("b-") || name.ends_with(".part") || name == me || !path.is_file() {
            continue;
        }
        let other = game_of(&lib.join(&name));
        if matches!((&game, &other), (Some(a), Some(b)) if a != b) {
            continue;
        }
        let t = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        found.push((t, name));
    }
    found.into_iter().max().map(|(_, n)| n)
}

fn text(v: Option<&Value>) -> String {
    match v {
        Some(Value::Str(s)) => s.clone(),
        None | Some(Value::Null) => "None".into(),
        Some(o) => nxb_pyjson::dumps(o),
    }
}

/// `(key, status, reason)` of the shaders ok in `prev` and not ok in `now`.
fn lost(prev: &[Value], now: &[Value]) -> Vec<(String, String, String)> {
    let key = |r: &Value| r.get("key").map(|k| text(Some(k))).unwrap_or_default();
    let ok = |r: &Value| r.get("st").and_then(Value::as_str) == Some("ok");
    let was_ok: BTreeSet<String> = prev.iter().filter(|r| ok(r)).map(key).collect();
    now.iter()
        .filter(|r| !ok(r) && was_ok.contains(&key(r)))
        .map(|r| (key(r), text(r.get("st")), text(r.get("err"))))
        .collect()
}

/// Run the command.
pub(crate) fn main(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let [lib, chain] = args else {
        let _ = writeln!(err, "usage: nxb regressions <library dir> <chain version>");
        return 2;
    };
    let lib = PathBuf::from(lib);
    let mine = lib.join(format!("b-{chain}")).join(REPORT);
    let r = (|| -> Result<i32, String> {
        let w = |e: std::io::Error| e.to_string();
        if !mine.is_file() {
            writeln!(
                out,
                "[RED] no build report {}: dmsh_build.py has not built chain {chain} there",
                mine.display()
            )
            .map_err(w)?;
            return Ok(2);
        }
        let now = rows(&mine)?;
        let Some(prev) = previous(&lib, chain) else {
            writeln!(
                out,
                "shader regressions: no earlier build in {} to compare b-{chain} with",
                lib.display()
            )
            .map_err(w)?;
            return Ok(0);
        };
        let l = lost(&rows(&lib.join(&prev).join(REPORT))?, &now);
        writeln!(out, "shader regressions against {prev}: {}", l.len()).map_err(w)?;
        for (k, st, e) in &l {
            writeln!(
                out,
                "[RED] {k}: translated in {prev}, {st} in b-{chain}: {e}"
            )
            .map_err(w)?;
        }
        Ok(i32::from(!l.is_empty()))
    })();
    r.unwrap_or_else(|e| {
        let _ = writeln!(err, "[RED] regressions: {e}");
        1
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> (i32, String) {
        let a: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let c = main(&a, &mut o, &mut e);
        (c, String::from_utf8(o).unwrap())
    }

    #[test]
    fn finds_a_lost_shader() {
        let d = std::env::temp_dir().join(format!("nxb-regr-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        for (n, body) in [
            (
                "b-old",
                r#"[{"key":"a","st":"ok","err":null},{"key":"b","st":"ok","err":null}]"#,
            ),
            (
                "b-new",
                r#"[{"key":"a","st":"ok","err":null},{"key":"b","st":"fail","err":"boom"},{"key":"c","st":"fail","err":"x"}]"#,
            ),
        ] {
            std::fs::create_dir_all(d.join(n)).unwrap();
            std::fs::write(d.join(n).join(REPORT), body).unwrap();
        }
        let ds = d.to_str().unwrap();
        let (c, o) = run(&[ds, "new"]);
        assert_eq!(c, 1);
        assert_eq!(
            o,
            "shader regressions against b-old: 1\n[RED] b: translated in b-old, fail in b-new: boom\n"
        );
        assert_eq!(run(&[ds, "old"]).0, 0);
        assert_eq!(run(&[ds, "none"]).0, 2);
        let _ = std::fs::remove_dir_all(&d);
    }
}
