//! `dm/ios/tools/gen_manifest.py`: the `game.manifest.json` inside a route C game framework (`build_ipa.sh` with
//! `DM_FRAMEWORK=1` or `DM_FRAMEWORK_ONLY`). Keys in the schema's order, `json.dumps(indent=2, ensure_ascii=False)`
//! plus a newline, the same checks and `[RED]` messages.

use crate::host::pyfmt::{repr_str, splitlines, words};
use nxb_pyjson::Value;
use std::path::{Path, PathBuf};

/// The schema version.
pub const SCHEMA: i64 = 1;
/// The marker symbol of the framework image.
pub const IMAGE_MARKER: &str = "dmgame_image_marker";
/// The runtime profile at the framework's root.
pub const PROFILE: &str = "profile.tsv";
/// The shader map inside the framework.
pub const SHADER_MAP: &str = "dm/shader_map.tsv";
/// The app binary settings inside the framework.
pub const LAUNCHER: &str = "dm/launcher.json";

/// The script's options (`--name` and the rest, defaults as its `argparse`).
#[derive(Clone, Debug)]
pub struct Options {
    /// `--game`: the base game title ID.
    pub game: String,
    /// `--name`.
    pub name: String,
    /// `--display-version`.
    pub display_version: String,
    /// `--package-version`.
    pub package_version: String,
    /// `--modules` (whitespace separated).
    pub modules: String,
    /// `--nro-table`.
    pub nro_table: Option<PathBuf>,
    /// `--chain` (`b-<version>`).
    pub chain: String,
    /// `--romfs-marker`.
    pub romfs_marker: String,
    /// `--romfs-identity` (default empty).
    pub romfs_identity: String,
    /// `--min-os` (default `26.0`).
    pub min_os: String,
    /// `--framework-name` (default `DmGame`).
    pub framework_name: String,
    /// `--loader` (default `FR`): `FH` when the framework links its own host (a patched engine's shell framework).
    pub loader: String,
    /// `--built-by`.
    pub built_by: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            game: String::new(),
            name: String::new(),
            display_version: String::new(),
            package_version: String::new(),
            modules: String::new(),
            nro_table: None,
            chain: String::new(),
            romfs_marker: String::new(),
            romfs_identity: String::new(),
            min_os: "26.0".into(),
            framework_name: "DmGame".into(),
            loader: "FR".into(),
            built_by: None,
        }
    }
}

/// `re.match(r'^...$', s)`: Python's `$` also matches before one final newline.
fn full(s: &str, first: fn(char) -> bool, rest: fn(char) -> bool) -> bool {
    let s = s.strip_suffix('\n').unwrap_or(s);
    let mut it = s.chars();
    matches!(it.next(), Some(c) if first(c)) && it.all(rest)
}

fn alnum(c: char) -> bool {
    c.is_ascii_alphanumeric()
}

fn word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `nro_count`: the rows `gen_embed.py` embeds (not comments, blank lines or `refused`).
pub fn nro_count(table: Option<&Path>) -> Result<i64, String> {
    let Some(t) = table else { return Ok(0) };
    let text = std::fs::read_to_string(t).map_err(|e| format!("{}: {e}", t.display()))?;
    Ok(splitlines(&text)
        .into_iter()
        .map(|l| l.split('\t').next().unwrap_or(""))
        .filter(|n| !n.is_empty() && !n.starts_with('#') && *n != "refused")
        .count() as i64)
}

/// `manifest(args)`: the manifest, or the script's `[RED]` message.
pub fn manifest(o: &Options) -> Result<Value, String> {
    // str.upper() of a title ID: only ASCII can pass the check below
    let title_id = o.game.to_uppercase();
    let modules = words(&o.modules);
    let hex = |c: char| c.is_ascii_digit() || ('A'..='F').contains(&c);
    let tid = title_id.strip_suffix('\n').unwrap_or(&title_id);
    if tid.chars().count() != 16 || !tid.chars().all(hex) {
        return Err(format!(
            "[RED] gen_manifest: --game {} is not a 16-digit hex title ID",
            repr_str(&o.game)
        ));
    }
    if modules.is_empty() || modules.iter().any(|m| !full(m, alnum, alnum)) {
        return Err(format!(
            "[RED] gen_manifest: --modules {} names no module or a module with an underscore (the loader's embedded section names cannot hold one)",
            repr_str(&o.modules)
        ));
    }
    if !full(&o.framework_name, |c| c.is_ascii_alphabetic(), word) {
        return Err(format!(
            "[RED] gen_manifest: --framework-name {} is not a bundle executable name",
            repr_str(&o.framework_name)
        ));
    }
    if !o
        .chain
        .strip_prefix("b-")
        .is_some_and(|v| full(v, chain_char, chain_char))
    {
        return Err(format!(
            "[RED] gen_manifest: --chain {} is not written b-<version> (dm/gfx/nvn/dmsh_chain.h)",
            repr_str(&o.chain)
        ));
    }
    if o.loader != "FR" && o.loader != "FH" {
        return Err(format!(
            "[RED] gen_manifest: --loader {} is not FR or FH",
            repr_str(&o.loader)
        ));
    }
    if !o.romfs_identity.is_empty() && !o.romfs_identity.starts_with("DMROMFSID1 ") {
        return Err(
            "[RED] gen_manifest: --romfs-identity is not a romfs_image.py identity line (DMROMFSID1 ...)".into(),
        );
    }
    let s = |v: &str| Value::str(v);
    let mut m = vec![
        ("schema", Value::Int(SCHEMA)),
        ("title_id", Value::Str(title_id.clone())),
        ("name", s(&o.name)),
        ("display_version", s(&o.display_version)),
        ("package_version", s(&o.package_version)),
        (
            "modules",
            Value::List(modules.iter().map(|m| s(m)).collect()),
        ),
        ("nro_count", Value::Int(nro_count(o.nro_table.as_deref())?)),
        ("chain", s(&o.chain)),
        ("loader", s(&o.loader)),
        (
            "framework",
            Value::Str(format!("{}.framework", o.framework_name)),
        ),
        ("framework_executable", s(&o.framework_name)),
        ("image_marker", s(IMAGE_MARKER)),
        ("profile", s(PROFILE)),
        ("shader_map", s(SHADER_MAP)),
        ("launcher", s(LAUNCHER)),
        ("romfs_marker", s(&o.romfs_marker)),
        ("romfs_identity", s(&o.romfs_identity)),
        ("min_os", s(&o.min_os)),
    ];
    if let Some(b) = o.built_by.as_deref().filter(|b| !b.is_empty()) {
        m.push(("built_by", s(b)));
    }
    Ok(Value::Object(
        m.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
    ))
}

fn chain_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')
}

/// The file's bytes: `json.dumps(data, indent=2, ensure_ascii=False) + '\n'`.
pub fn text(v: &Value) -> String {
    nxb_pyjson::dumps_indent(v, 2) + "\n"
}

/// `main`: write the manifest to `out` (parent folders made) and return it.
pub fn write(out: &Path, o: &Options) -> Result<Value, String> {
    let v = manifest(o)?;
    if let Some(p) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    std::fs::write(out, text(&v)).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ios::oracle;

    fn opts() -> Options {
        Options {
            game: "0100152000022000".into(),
            name: "Mario Kart \u{2605} \"8\"".into(),
            display_version: "3.0.1".into(),
            package_version: "v196608".into(),
            modules: " main  sdk\tsubsdk0 ".into(),
            chain: "b-0ad559cdbb28".into(),
            romfs_marker: "Data/x.bin".into(),
            ..Options::default()
        }
    }

    /// The reference script's output for [`opts`] with `nro.tsv` = 2 rows, a comment, a blank line and `refused`
    /// (`python3 gen_manifest.py out --game 0100152000022000 ... --built-by ci`).
    const EXPECTED: &str = "{\n  \"schema\": 1,\n  \"title_id\": \"0100152000022000\",\n  \"name\": \"Mario Kart \u{2605} \\\"8\\\"\",\n  \"display_version\": \"3.0.1\",\n  \"package_version\": \"v196608\",\n  \"modules\": [\n    \"main\",\n    \"sdk\",\n    \"subsdk0\"\n  ],\n  \"nro_count\": 2,\n  \"chain\": \"b-0ad559cdbb28\",\n  \"loader\": \"FR\",\n  \"framework\": \"DmGame.framework\",\n  \"framework_executable\": \"DmGame\",\n  \"image_marker\": \"dmgame_image_marker\",\n  \"profile\": \"profile.tsv\",\n  \"shader_map\": \"dm/shader_map.tsv\",\n  \"launcher\": \"dm/launcher.json\",\n  \"romfs_marker\": \"Data/x.bin\",\n  \"romfs_identity\": \"\",\n  \"min_os\": \"26.0\",\n  \"built_by\": \"ci\"\n}\n";

    const NRO: &str = "# name\tx\nfoo\t1\n\nrefused\tnn\nbar\t2\n";

    #[test]
    fn equals_the_reference_output() {
        let d = oracle::scratch("manifest");
        std::fs::write(d.join("nro.tsv"), NRO).unwrap();
        let mut o = opts();
        o.nro_table = Some(d.join("nro.tsv"));
        o.built_by = Some("ci".into());
        write(&d.join("m/game.manifest.json"), &o).unwrap();
        assert_eq!(
            std::fs::read_to_string(d.join("m/game.manifest.json")).unwrap(),
            EXPECTED
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn writes_the_loader_it_is_given() {
        let mut o = opts();
        assert_eq!(
            manifest(&o).unwrap().get("loader").unwrap().as_str(),
            Some("FR")
        );
        o.loader = "FH".into();
        assert_eq!(
            manifest(&o).unwrap().get("loader").unwrap().as_str(),
            Some("FH")
        );
    }

    #[test]
    fn refuses_what_the_schema_cannot_carry() {
        let bad = |f: fn(&mut Options)| {
            let mut o = opts();
            f(&mut o);
            manifest(&o).unwrap_err()
        };
        assert!(bad(|o| o.game = "0100".into()).contains("16-digit"));
        assert!(bad(|o| o.modules = "main sub_sdk".into()).contains("--modules"));
        assert!(bad(|o| o.modules = " ".into()).contains("--modules"));
        assert!(bad(|o| o.framework_name = "9Dm".into()).contains("--framework-name"));
        assert!(bad(|o| o.chain = "c-1".into()).contains("--chain"));
        assert!(bad(|o| o.chain = "b-".into()).contains("--chain"));
        assert!(bad(|o| o.romfs_identity = "X".into()).contains("--romfs-identity"));
        assert!(bad(|o| o.loader = "FX".into()).contains("--loader"));
        // lower case title IDs are written upper case, Python's `$` passes one final newline
        let mut o = opts();
        o.game = "0100152000022abc".into();
        o.chain = "b-1\n".into();
        let v = manifest(&o).unwrap();
        assert_eq!(
            v.get("title_id").unwrap().as_str(),
            Some("0100152000022ABC")
        );
    }

    /// Oracle: the engine's `gen_manifest.py` on the same options, byte for byte (several option sets).
    #[test]
    fn equals_the_reference_script() {
        let Some(e) = oracle::engine() else { return };
        let d = oracle::scratch("manifest-oracle");
        std::fs::write(d.join("nro.tsv"), NRO).unwrap();
        let mut cases = vec![opts()];
        let mut o = opts();
        o.nro_table = Some(d.join("nro.tsv"));
        o.built_by = Some("ci".into());
        cases.push(o.clone());
        o.nro_table = Some(d.join("nro.tsv"));
        o.built_by = Some("ci".into());
        o.framework_name = "DmGame0100152000022000".into();
        o.romfs_identity = "DMROMFSID1 123 abc".into();
        o.min_os = "18.0".into();
        o.game = "0100152000022abc".into();
        cases.push(o);
        for (i, o) in cases.iter().enumerate() {
            let py = d.join(format!("py{i}.json"));
            let mut c = oracle::python();
            c.arg(e.join("dm/ios/tools/gen_manifest.py")).arg(&py);
            for (k, v) in [
                ("--game", &o.game),
                ("--name", &o.name),
                ("--display-version", &o.display_version),
                ("--package-version", &o.package_version),
                ("--modules", &o.modules),
                ("--chain", &o.chain),
                ("--romfs-marker", &o.romfs_marker),
                ("--romfs-identity", &o.romfs_identity),
                ("--min-os", &o.min_os),
                ("--framework-name", &o.framework_name),
            ] {
                c.arg(k).arg(v);
            }
            if let Some(t) = &o.nro_table {
                c.arg("--nro-table").arg(t);
            }
            if let Some(b) = &o.built_by {
                c.arg("--built-by").arg(b);
            }
            if o.loader != "FR" {
                c.arg("--loader").arg(&o.loader);
            }
            assert!(c.status().unwrap().success());
            let rs = d.join(format!("rs{i}.json"));
            write(&rs, o).unwrap();
            assert_eq!(std::fs::read(&rs).unwrap(), std::fs::read(&py).unwrap());
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}
