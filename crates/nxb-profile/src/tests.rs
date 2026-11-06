//! Round trips and the rules of 2.11.

use crate::toml::{self, Value};
use crate::*;

const SAMPLE: &str = r#"
title_id = "0100000000010000"

[computed]
address_space = "64bit"   # NPDM
pool_size = 268_435_456
module_sizes = [1048576, 2097152]
mem_used = 3221225472
shader_groups = ["a", 'b']
pad_layout = "pro-controller"

[declared]
title = "Sample"
display_name = "Sample \"Game\""
op_mode = "docked"
present = "display"
fps_rules = ["menus-30"]

[declared.proof.op_mode]
capability = "op_mode"
test = "t_boot_docked"

[declared.proof.present]
capability = "present"
test = "t_present_pace"

[declared.proof.fps_rules]
capability = "frame pacing"
test = "t_fps"

[rules]
engine = "unreal"
frame_limit = { kind = "cvar", name = "t.MaxFPS" }
cuts = [
  { bind = "game", fingerprint = "ab12", bytes = "1f2003d5", why = "skips the splash" },
  { bind = "game", fingerprint = "cd34", bytes = "00", why = "no sleep" },
]
future_key = 7
"#;

#[test]
fn reads_all_three_sections() {
    let p = Profile::from_toml(SAMPLE).unwrap();
    assert_eq!(p.title_id.as_deref(), Some("0100000000010000"));
    assert_eq!(p.computed.pool_size, Some(268_435_456));
    assert_eq!(p.computed.module_sizes, [1048576, 2097152]);
    assert_eq!(p.declared.display_name.as_deref(), Some("Sample \"Game\""));
    assert_eq!(p.declared.present, Some(Present::Display));
    assert_eq!(
        p.rules.frame_limit,
        Some(FrameLimit {
            kind: "cvar".into(),
            name: "t.MaxFPS".into()
        })
    );
    assert_eq!(p.rules.cuts.len(), 2);
    assert_eq!(p.rules.extra.get("future_key"), Some(&Value::Int(7)));
    assert_eq!(
        p.get_str("rules.frame_limit.name").as_deref(),
        Some("t.MaxFPS")
    );
    assert_eq!(
        p.get_str("declared.proof.op_mode.test").as_deref(),
        Some("t_boot_docked")
    );
    assert_eq!(p.get("declared.nothing"), None);
    assert!(p.problems().is_empty(), "{:?}", p.problems());
}

#[test]
fn round_trip_is_stable() {
    let p = Profile::from_toml(SAMPLE).unwrap();
    let text = p.to_toml();
    let q = Profile::from_toml(&text).unwrap();
    assert_eq!(p, q);
    assert_eq!(q.to_toml(), text, "writing is deterministic");
    let empty = Profile::default();
    assert_eq!(empty.to_toml(), "");
    assert_eq!(Profile::from_toml("").unwrap(), empty);
    assert!(empty.is_provisional());
}

#[test]
fn unknown_sections_survive() {
    let p = Profile::from_toml("[computed]\nmem_used = 1\n\n[later]\nx = 1\n").unwrap();
    let again = Profile::from_toml(&p.to_toml()).unwrap();
    assert_eq!(p, again);
    assert!(p.to_toml().contains("[later]"));
}

#[test]
fn committed_text_has_no_computed_section() {
    let p = Profile::from_toml(SAMPLE).unwrap();
    let c = p.to_committed_toml();
    assert!(!c.contains("[computed]") && !c.contains("pool_size"));
    assert!(c.contains("[declared]") && c.contains("[rules]"));
}

#[test]
fn rules_of_the_design() {
    let p = Profile::from_toml("[declared]\nop_mode = \"docked\"\ndepth_mode = \"zero\"\n\n[rules]\ncuts = [{ bind = \"g\", fingerprint = \"f\", bytes = \"00\", why = \" \" }]\n").unwrap();
    let pr = p.problems();
    assert_eq!(pr.len(), 3, "{pr:?}");
    assert!(pr.iter().any(|m| m.contains("declared.op_mode needs")));
    assert!(
        pr.iter()
            .any(|m| m.contains("depth_mode is decided by a capability"))
    );
    assert!(pr.iter().any(|m| m.contains("rules.cuts[0] has no reason")));
}

#[test]
fn wrong_shapes_name_the_key() {
    let e = Profile::from_toml("[computed]\npool_size = \"big\"\n").unwrap_err();
    assert_eq!(
        e,
        ProfileError::Shape("computed.pool_size: a non-negative integer expected".into())
    );
    assert!(Profile::from_toml("[declared]\npresent = \"both\"\n").is_err());
    assert!(Profile::from_toml("[rules]\ncuts = [{ bind = \"g\" }]\n").is_err());
    assert!(matches!(
        Profile::from_toml("[a\n"),
        Err(ProfileError::Parse(_))
    ));
}

#[test]
fn toml_subset() {
    let t = toml::parse(
        "a = 1\nb.c = true\n[x.y]\nz = [1, 2,\n 3,]\ns = 'lit\\n'\nf = 1.5\nq = \"\\u00e9\\t\"\n",
    )
    .unwrap();
    assert_eq!(t["a"], Value::Int(1));
    let Value::Table(b) = &t["b"] else { return };
    assert_eq!(b["c"], Value::Bool(true));
    let Value::Table(x) = &t["x"] else { return };
    let Value::Table(y) = &x["y"] else { return };
    assert_eq!(
        y["z"],
        Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)])
    );
    assert_eq!(y["s"], Value::Str("lit\\n".into()));
    assert_eq!(y["f"], Value::Float(1.5));
    assert_eq!(y["q"], Value::Str("\u{e9}\t".into()));
    assert_eq!(toml::parse(&toml::write(&t)).unwrap(), t);
    for bad in [
        "a = 1\na = 2\n",
        "[t]\n[t]\n",
        "[[t]]\n",
        "a = \n",
        "a = \"x\n",
        "a = 1 2\n",
        "a = [1\n",
    ] {
        let r = toml::parse(bad);
        assert!(r.is_err(), "{bad:?} gave {r:?}");
        if let Err(e) = r {
            assert!(e.line >= 1);
        }
    }
}

#[test]
fn tsv_round_trip_is_lossless() {
    use crate::tsv::TsvFile;
    for t in [
        "",
        "\n",
        "# c\n\nkey\tv  \n  # x\nk2\ta\tb",
        "a\tb\r\nc\t\td\r\n",
        "x\n\n\n",
    ] {
        assert_eq!(TsvFile::parse(t).to_text(), t, "{t:?}");
    }
}

#[test]
fn tsv_edits_and_the_typed_profile() {
    use crate::tsv::TsvFile;
    let mut t = TsvFile::parse(
        "# a\ntitle_id\t0100000000010000\nname\tG\napp_pool\t0xCD500000\nsys_resource\t0x1000000\nshader_groups\t/a/=a /b/=b\nmodules\tmain sdk\nbuild\tv0\t1\tmain=ab\nbuild\tv1\t2\n",
    );
    assert_eq!(t.get("build").as_deref(), Some("v0\t1\tmain=ab"));
    assert_eq!(t.rows_of("build").count(), 2);
    let p = Profile::from_game_tsv(&t);
    assert_eq!(p.computed.pool_size, Some(0xCD500000));
    assert_eq!(p.computed.address_space.as_deref(), Some("virtual"));
    assert_eq!(p.computed.shader_groups, ["/a/=a", "/b/=b"]);
    // the TOML form keeps all of it
    let q = Profile::from_toml(&p.to_toml()).unwrap();
    assert_eq!(p, q);
    let mut u = TsvFile::new();
    q.computed_into_game_tsv(&mut u);
    assert_eq!(u.get("modules").as_deref(), Some("main sdk"));
    assert_eq!(u.get("app_pool").as_deref(), Some("0xcd500000"));
    t.set("name", &["H"]);
    assert!(t.to_text().contains("\nname\tH\n"));
    assert_eq!(t.remove("build"), 2);
}

/// Every table of every profile of the engine (`NXB_ORACLE_ENGINE` = the engine folder) reads and writes back byte
/// for byte, and `game.tsv` reads into the typed profile.
#[test]
fn engine_tables_round_trip() {
    use crate::tsv::TsvFile;
    let Some(engine) = std::env::var_os("NXB_ORACLE_ENGINE") else {
        return;
    };
    let games = std::path::Path::new(&engine).join("dm/games");
    let (mut files, mut profiles) = (0, 0);
    for d in std::fs::read_dir(&games).unwrap().flatten() {
        let d = d.path();
        if !d.join("game.tsv").is_file() {
            continue;
        }
        profiles += 1;
        for f in std::fs::read_dir(&d).unwrap().flatten() {
            let f = f.path();
            if f.extension().is_none_or(|e| e != "tsv") {
                continue;
            }
            let raw = std::fs::read(&f).unwrap();
            let t = TsvFile::load(&f).unwrap();
            assert!(t.to_text().as_bytes() == raw.as_slice(), "{}", f.display());
            files += 1;
        }
        let p = Profile::from_game_tsv(&TsvFile::load(&d.join("game.tsv")).unwrap());
        assert_eq!(Profile::from_toml(&p.to_toml()).unwrap(), p);
    }
    eprintln!("{profiles} profiles, {files} tables identical");
    assert!(profiles > 100);
}
