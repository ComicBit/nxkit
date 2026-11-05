//! The app binary settings (`launcher.json`): `dm/games/settings.py`. The rows of `dm/games/settings.tsv` whose condition
//! holds for the game, with the game's defaults, as `json.dumps(indent=1, ensure_ascii=False)` + newline. Same checks,
//! same output bytes.

use super::header::{header_one, macros};
use super::profile::{Profiles, dict_set};
use super::pyfmt::splitlines;
use nxb_pyjson::Value;
use std::path::Path;

const SECTIONS: [&str; 3] = ["Game", "Controls", "Performance"];
const SCHEMA: i64 = 1;
const UNKNOWN_LANGUAGES: &str = "This game's files do not list its languages, so some of these may not be in the game. It may then show another one.";

/// The NACP's languages in its own order: (code, name).
pub const LANGUAGES: [(&str, &str); 16] = [
    ("en-US", "American English"),
    ("en-GB", "British English"),
    ("ja", "Japanese"),
    ("fr", "French"),
    ("de", "German"),
    ("es-419", "Latin American Spanish"),
    ("es", "Spanish"),
    ("it", "Italian"),
    ("nl", "Dutch"),
    ("fr-CA", "Canadian French"),
    ("pt", "Portuguese"),
    ("ru", "Russian"),
    ("ko", "Korean"),
    ("zh-Hant", "Traditional Chinese"),
    ("zh-Hans", "Simplified Chinese"),
    ("pt-BR", "Brazilian Portuguese"),
];

/// `nacp.languages`: the codes of the `SupportedLanguageFlag` bits (u32 at 0x302C), none for a missing or short file.
pub fn languages(control: &Path) -> Vec<&'static str> {
    let Ok(b) = std::fs::read(control) else {
        return Vec::new();
    };
    let Some(f) = b.get(0x302C..0x3030) else {
        return Vec::new();
    };
    let f = u32::from_le_bytes([f[0], f[1], f[2], f[3]]);
    LANGUAGES
        .iter()
        .enumerate()
        .filter(|(i, _)| f >> i & 1 == 1)
        .map(|(_, (c, _))| *c)
        .collect()
}

/// The NSP script's `language_code`: a code or a name as typed (any case, `_` for `-`) -> its code.
pub fn language_code(s: &str) -> Option<&'static str> {
    let k = super::pyfmt::strip(s).to_lowercase().replace('_', "-");
    if let Some((c, _)) = LANGUAGES
        .iter()
        .find(|(c, n)| k == c.to_lowercase() || k == n.to_lowercase())
    {
        return Some(c);
    }
    match k.as_str() {
        "en" | "english" => Some("en-US"),
        "zh-cn" => Some("zh-Hans"),
        "zh-tw" => Some("zh-Hant"),
        _ => None,
    }
}

/// The NSP script's `game_language`: `want` when the game has it (or lists none), else the same language of another
/// region, a language the game lacks is an error when it was asked for, else the game's first.
pub fn game_language(want: &str, asked: bool, have: &[&str]) -> Result<String, String> {
    if have.is_empty() || have.contains(&want) {
        return Ok(want.to_string());
    }
    let near = match want {
        "en-US" => "en-GB",
        "en-GB" => "en-US",
        "fr" => "fr-CA",
        "fr-CA" => "fr",
        "es" => "es-419",
        "es-419" => "es",
        "pt" => "pt-BR",
        "pt-BR" => "pt",
        _ => "",
    };
    if have.contains(&near) {
        return Ok(near.to_string());
    }
    if asked {
        return Err(format!(
            "the game has no {want}; its languages: {} (--language)",
            have.join(" ")
        ));
    }
    Ok(have[0].to_string())
}

/// The NSP script's `languages_control`: the first folder's `control.nacp` that lists languages, else the first's.
pub fn languages_control(dirs: &[&Path]) -> Option<std::path::PathBuf> {
    let paths: Vec<_> = dirs.iter().map(|d| d.join("control.nacp")).collect();
    paths
        .iter()
        .find(|p| !languages(p).is_empty())
        .or(paths.first())
        .cloned()
}

/// What `settings.py` reads besides its arguments: the environment (`SETTINGS_PLATFORM`, `GFXAPI`, `build:<VAR>`
/// defaults, `DM_GAMES_DIR`, `DM_GAMES_FIRST`).
pub type Env<'a> = &'a dyn Fn(&str) -> Option<String>;

fn meta(text: &str, n: usize) -> Result<Vec<(String, String)>, String> {
    let mut out: Vec<(String, String)> = Vec::new();
    for pair in text.split(';').map(str::trim).filter(|p| !p.is_empty()) {
        let (key, value) = match pair.split_once('=') {
            Some((k, v)) if k == "level" || k == "needs" => (k, v),
            _ => {
                return Err(format!(
                    "settings.tsv:{n}: unknown meta key {pair:?} (keys: level, needs)"
                ));
            }
        };
        if out.iter().any(|(k, _)| k == key) {
            return Err(format!("settings.tsv:{n}: meta key {key} given twice"));
        }
        let ok = match key {
            "level" => value == "basic",
            _ => value.split_once('=').is_some_and(|(var, val)| {
                var.len() > 3
                    && var.starts_with("DM_")
                    && var[3..]
                        .bytes()
                        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
                    && !val.is_empty()
                    && !val.contains([';', '='])
            }),
        };
        if !ok {
            return Err(format!(
                "settings.tsv:{n}: meta {key}={value:?} does not match its pattern"
            ));
        }
        out.push((key.into(), value.into()));
    }
    Ok(out)
}

/// The table's rows: (line number, 9 columns).
fn rows(text: &str) -> Result<Vec<(usize, Vec<String>)>, String> {
    let mut out = Vec::new();
    for (i, line) in splitlines(text).into_iter().enumerate() {
        let n = i + 1;
        if super::pyfmt::strip(line).is_empty() || line.starts_with('#') {
            continue;
        }
        let mut c: Vec<String> = line.split('\t').map(str::to_string).collect();
        if !(7..=9).contains(&c.len()) {
            return Err(format!(
                "settings.tsv:{n}: 7 to 9 columns expected, {} found",
                c.len()
            ));
        }
        if !SECTIONS.contains(&c[5].as_str()) {
            return Err(format!("settings.tsv:{n}: unknown section: {}", c[5]));
        }
        c.resize(9, String::new());
        out.push((n, c));
    }
    Ok(out)
}

/// `'# KEY<TAB>value'` header rows of a rewriter table.
fn fixups_keys(path: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.split('\n')
        .filter_map(|l| {
            let rest = l.strip_prefix("# ")?;
            let n = rest.find(|c: char| !(c.is_alphanumeric() || c == '_'))?;
            (n > 0 && rest[n..].starts_with('\t')).then(|| rest[..n].to_string())
        })
        .collect()
}

fn render_base(v: &str, id: &str) -> Result<Option<Value>, String> {
    if v.is_empty() {
        return Ok(None);
    }
    let bad = || format!("{id}: render_base is \"<w>x<h> <w>x<h>\" (docked, then handheld): {v}");
    let w: Vec<&str> = super::pyfmt::words(v);
    let size = |s: &str| -> Option<Value> {
        let (a, b) = s.split_once('x')?;
        let num = |t: &str| {
            (!t.is_empty() && t.bytes().all(|c| c.is_ascii_digit()))
                .then(|| t.parse::<i64>().ok())?
        };
        Some(Value::List(vec![Value::Int(num(a)?), Value::Int(num(b)?)]))
    };
    match w.as_slice() {
        [d, h] => Ok(Some(Value::Object(vec![
            ("docked".into(), size(d).ok_or_else(bad)?),
            ("handheld".into(), size(h).ok_or_else(bad)?),
        ]))),
        _ => Err(bad()),
    }
}

/// Python `float(s)` for the range columns.
fn float(s: &str, n: usize) -> Result<f64, String> {
    let t = super::pyfmt::strip(s).replace('_', "");
    let lower = t.to_ascii_lowercase();
    let v = match lower.trim_start_matches(['+', '-']) {
        "inf" | "infinity" | "nan" => lower.parse::<f64>().ok(),
        _ => t.parse::<f64>().ok(),
    };
    v.ok_or_else(|| format!("settings.tsv:{n}: could not convert string to float: {s:?}"))
}

/// The settings of game `gid` (its rewritten modules in `patched`, its languages in `control`), as the JSON value.
pub fn build(
    engine: &Path,
    gid: &str,
    patched: &Path,
    control: &Path,
    env: Env,
) -> Result<Value, String> {
    let platform = env("SETTINGS_PLATFORM")
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| "ios".into());
    if platform != "ios" && platform != "mac" {
        return Err(format!("SETTINGS_PLATFORM must be ios or mac: {platform}"));
    }
    let profiles = Profiles::new(engine, env);
    let g = profiles.load(gid)?;
    let macros = macros(&header_one(&g, &profiles.here)?);
    let keys = fixups_keys(&patched.join("main.fixups.tsv"));
    let have = languages(control);
    let langs: Vec<(&str, &str)> = LANGUAGES
        .iter()
        .filter(|(c, _)| have.is_empty() || have.contains(c))
        .copied()
        .collect();
    let macro_value = |name: &str, n: usize| -> Result<String, String> {
        macros
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.trim_matches('"').to_string())
            .ok_or_else(|| {
                format!("settings.tsv:{n}: no {name} in the game profile header (game.py header)")
            })
    };
    let gfxapi = env("GFXAPI").unwrap_or_default();
    let shown = |cond: &str, n: usize| -> Result<bool, String> {
        if cond == "always" {
            return Ok(true);
        }
        if cond == "languages>1" {
            return Ok(langs.len() > 1);
        }
        if let Some(k) = cond.strip_prefix("fixups:") {
            return Ok(keys.iter().any(|x| x == k));
        }
        if let Some(a) = cond.strip_prefix("gfxapi:") {
            return Ok(gfxapi == a);
        }
        if let Some(p) = cond.strip_prefix("platform:") {
            return Ok(platform == p);
        }
        if let Some(rest) = cond.strip_prefix("DM_GAME_") {
            let n_name = rest
                .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            // the regex's \w+ backtracks: the operator is the first != or == after at least one name character
            let op = rest
                .char_indices()
                .skip(1)
                .find(|(i, _)| rest[*i..].starts_with("!=") || rest[*i..].starts_with("=="))
                .map(|(i, _)| i)
                .filter(|i| *i <= n_name);
            if let Some(i) = op {
                let name = format!("DM_GAME_{}", &rest[..i]);
                let want = &rest[i + 2..];
                return Ok((macro_value(&name, n)? == want) == rest[i..].starts_with("=="));
            }
        }
        Err(format!("settings.tsv:{n}: unknown condition {cond}"))
    };
    let value = |d: &str, n: usize| -> Result<String, String> {
        let mut d = d.to_string();
        if d.contains('|') {
            let parts: Vec<&str> = d.split('|').collect();
            if parts.len() != 2 || parts.iter().any(|p| p.is_empty()) {
                return Err(format!(
                    "settings.tsv:{n}: platform default is <ios>|<mac>: {d}"
                ));
            }
            d = if platform == "ios" {
                parts[0]
            } else {
                parts[1]
            }
            .to_string();
        }
        if d.starts_with("DM_GAME_") {
            return macro_value(&d, n);
        }
        if let Some(var) = d.strip_prefix("build:") {
            let v = env(var).unwrap_or_default();
            return Ok(match v.as_str() {
                "YES" => "1".into(),
                "NO" | "" => "0".into(),
                _ => v,
            });
        }
        Ok(d)
    };
    let takes = |kind: &str, values: &str| -> Option<Vec<String>> {
        match kind {
            "toggle" if values == "-" => Some(vec!["1".into(), "0".into()]),
            "toggle" => Some(values.split('/').map(str::to_string).collect()),
            "choice" if values != "languages" => Some(
                values
                    .split(',')
                    .map(|v| v.split('=').next().unwrap_or("").to_string())
                    .collect(),
            ),
            _ => None,
        }
    };
    let tsv = engine.join("dm/games/settings.tsv");
    let text = std::fs::read_to_string(&tsv).map_err(|e| format!("{}: {e}", tsv.display()))?;
    let all = rows(&text)?;
    let mut table: Vec<(String, Option<Vec<String>>)> = Vec::new();
    for (_, c) in &all {
        dict_set(&mut table, &c[0], takes(&c[1], &c[2]));
    }
    let base = render_base(&g.get_or("render_base", ""), &g.id)?;
    let mut out = Vec::new();
    for (n, c) in &all {
        let n = *n;
        let [
            var,
            kind,
            values,
            default,
            cond,
            section,
            label,
            note,
            extra,
        ] = &c[..]
        else {
            continue;
        };
        let m = meta(extra, n)?;
        if let Some((_, needs)) = m.iter().find(|(k, _)| k == "needs") {
            let (need, want) = needs.split_once('=').unwrap_or((needs, ""));
            let Some((_, can)) = table.iter().find(|(k, _)| k == need) else {
                return Err(format!(
                    "settings.tsv:{n}: needs names a variable no row sets: {needs}"
                ));
            };
            if let Some(can) = can
                && !can.iter().any(|v| v == want)
            {
                let mut sorted = can.clone();
                sorted.sort();
                sorted.dedup();
                return Err(format!(
                    "settings.tsv:{n}: needs a value {need} never takes: {want} (it takes {})",
                    sorted.join(", ")
                ));
            }
        }
        if !shown(cond, n)? {
            continue;
        }
        let mut s: Vec<(String, Value)> = vec![
            ("env".into(), Value::str(var)),
            ("kind".into(), Value::str(kind)),
            ("section".into(), Value::str(section)),
            ("label".into(), Value::str(label)),
            ("note".into(), Value::str(note)),
            ("default".into(), Value::str(value(default, n)?)),
        ];
        s.extend(m.into_iter().map(|(k, v)| (k, Value::Str(v))));
        if var == "DM_RENDER_SCALE"
            && let Some(b) = &base
        {
            s.push(("base".into(), b.clone()));
        }
        let get_default = |s: &[(String, Value)]| {
            s.iter()
                .find(|(k, _)| k == "default")
                .and_then(|(_, v)| v.as_str().map(str::to_string))
                .unwrap_or_default()
        };
        match kind.as_str() {
            "choice" => {
                let opts: Vec<Vec<String>> = if values == "languages" {
                    langs
                        .iter()
                        .map(|(c, n)| vec![c.to_string(), n.to_string()])
                        .collect()
                } else {
                    values
                        .split(',')
                        .map(|v| v.splitn(2, '=').map(str::to_string).collect())
                        .collect()
                };
                if opts.iter().any(|o| o.len() != 2) {
                    return Err(format!(
                        "settings.tsv:{n}: choice values are value=label,..."
                    ));
                }
                let d = get_default(&s);
                if !opts.iter().any(|o| o[0] == d)
                    && let Some(first) = opts.first()
                {
                    dict_set(&mut s, "default", Value::str(first[0].clone()));
                }
                let list = opts
                    .into_iter()
                    .map(|o| Value::List(o.into_iter().map(Value::Str).collect()))
                    .collect();
                s.push(("options".into(), Value::List(list)));
                if values == "languages" && have.is_empty() {
                    dict_set(&mut s, "note", Value::str(UNKNOWN_LANGUAGES));
                }
            }
            "range" => {
                let p: Vec<&str> = values.split(':').collect();
                let [lo, hi, st] = p[..] else {
                    return Err(format!(
                        "settings.tsv:{n}: range values are low:high:step: {values}"
                    ));
                };
                s.push(("min".into(), Value::Float(float(lo, n)?)));
                s.push(("max".into(), Value::Float(float(hi, n)?)));
                s.push(("step".into(), Value::Float(float(st, n)?)));
            }
            "toggle" => {
                let d = get_default(&s);
                if d != "0" && d != "1" {
                    return Err(format!(
                        "settings.tsv:{n}: a toggle defaults to 0 or 1: {d}"
                    ));
                }
                let on_off: Vec<String> = if values == "-" {
                    vec!["1".into(), "0".into()]
                } else {
                    values.split('/').map(str::to_string).collect()
                };
                if on_off.len() != 2
                    || on_off.iter().any(String::is_empty)
                    || on_off[0] == on_off[1]
                {
                    return Err(format!(
                        "settings.tsv:{n}: toggle values are - or <on>/<off>: {values}"
                    ));
                }
                dict_set(&mut s, "on", Value::str(on_off[0].clone()));
                dict_set(&mut s, "off", Value::str(on_off[1].clone()));
                let pick = if d == "1" { &on_off[0] } else { &on_off[1] };
                dict_set(&mut s, "default", Value::str(pick.clone()));
            }
            _ => return Err(format!("settings.tsv:{n}: unknown kind {kind}")),
        }
        out.push(Value::Object(s));
    }
    Ok(Value::Object(vec![
        ("schema".into(), Value::Int(SCHEMA)),
        ("game".into(), Value::str(g.id.clone())),
        ("settings".into(), Value::List(out)),
    ]))
}

/// `settings.py <gid> <patched> <control> <out>`: the file's bytes.
pub fn render(
    engine: &Path,
    gid: &str,
    patched: &Path,
    control: &Path,
    env: Env,
) -> Result<String, String> {
    let v = build(engine, gid, patched, control, env)?;
    Ok(format!("{}\n", nxb_pyjson::dumps_indent(&v, 1)))
}

/// Write `out` (as `settings.py` does: the whole file at once).
pub fn write(
    engine: &Path,
    gid: &str,
    patched: &Path,
    control: &Path,
    out: &Path,
    env: Env,
) -> Result<(), String> {
    let text = render(engine, gid, patched, control, env)?;
    std::fs::write(out, text).map_err(|e| format!("{}: {e}", out.display()))
}
