//! A text report of the facts and the guess of one extraction. The Python oracle (`profile_guess.py`) prints the same
//! lines from a small driver, so a difference between the two is a line difference.

use super::guess::{Facts, Guess, facts_of, guess, sort_load_order};
use super::guess_src::{main_modules, nro_modules, shader_sources, survey_inputs};
use super::nsofacts::Counter;
use nxb_model::sha256;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::Path;

fn digest(set: &std::collections::BTreeSet<String>) -> String {
    let mut t = String::new();
    for s in set {
        t.push_str(s);
        t.push('\n');
    }
    sha256(t.as_bytes()).to_string()[..16].to_string()
}

fn counter(c: &Counter, n: usize) -> String {
    c.most_common(n)
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("|")
}

/// The report lines of the facts.
pub fn facts_text(f: &Facts) -> String {
    let mut s = String::new();
    for (n, m) in &f.mods {
        let sizes: Vec<String> = m.sizes.iter().map(|x| x.to_string()).collect();
        let comps: Vec<&str> = m.comps.iter().map(String::as_str).collect();
        let _ = writeln!(
            s,
            "mod {n} kind={} bid={} sizes={} bss={} far={} imports={}:{} exports={}:{} comps={}",
            m.kind,
            m.bid,
            sizes.join(","),
            m.bss,
            u8::from(m.far),
            m.imports.len(),
            digest(&m.imports),
            m.exports.len(),
            digest(&m.exports),
            comps.join(",")
        );
    }
    let j =
        |s: &std::collections::BTreeSet<String>| s.iter().cloned().collect::<Vec<_>>().join(",");
    let markers: Vec<String> = f
        .markers
        .iter()
        .map(|(k, v)| format!("{k}:{}", v.join("+")))
        .collect();
    let _ = writeln!(
        s,
        "sdkver={} comps={} unity={} markers={}",
        j(&f.sdkver),
        j(&f.comps),
        j(&f.unity),
        markers.join(";")
    );
    let _ = writeln!(
        s,
        "glslc={} glsl_src={} gl_names={} gl_src={} sysres={} program={} sdk={} ounce={} main_markers={}",
        f.glslc,
        f.glsl_src,
        f.gl_names,
        f.gl_src,
        f.sysres,
        f.program.map_or("None".into(), |p| format!("{p:016x}")),
        f.sdk,
        u8::from(f.ounce),
        j(&f.main_markers)
    );
    let r = &f.romfs;
    let _ = writeln!(
        s,
        "romfs n={} bytes={} biggest={}:{} root={}",
        r.n,
        r.bytes,
        r.biggest.0,
        r.biggest.1,
        j(&r.root)
    );
    for (name, c, n) in [
        ("tops", &r.tops, 30),
        ("topb", &r.topb, 30),
        ("exts", &r.exts, 40),
        ("extb", &r.extb, 40),
        ("extc", &r.extc, 40),
        ("mags", &r.mags, 40),
        ("magb", &r.magb, 40),
        ("dirs", &r.dirs, 30),
    ] {
        let _ = writeln!(s, "{name} {}", counter(c, n));
    }
    s
}

/// The report lines of a guess.
pub fn guess_text(g: &Guess) -> String {
    let mut s = String::new();
    for (k, (v, why)) in &g.0 {
        let _ = writeln!(s, "guess {k}\t{v}\t{why}");
    }
    s
}

/// The whole report of an extraction folder (`ExeFS`, `romfs_manifest.tsv`, `romfs_nro`).
pub fn report(ex: &Path, hooks: &HashSet<String>) -> Result<String, String> {
    let f = facts_of(&ex.join("ExeFS"), &ex.join("romfs_manifest.tsv"))?;
    let g = guess(&f, hooks)?;
    let mut s = facts_text(&f);
    s.push_str(&guess_text(&g));
    let mut keep: Vec<String> = g
        .value("modules")
        .unwrap_or("")
        .split_whitespace()
        .map(str::to_string)
        .collect();
    let nros: Vec<_> = {
        let mut v = Vec::new();
        collect_nros(&ex.join("romfs_nro"), &mut v);
        v.sort();
        v
    };
    let add = if nros.is_empty() {
        Vec::new()
    } else {
        nro_modules(&ex.join("ExeFS"), &nros, &keep, hooks)?
    };
    let _ = writeln!(
        s,
        "nro_add {}",
        add.iter()
            .map(|(m, k)| format!("{m}={k}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    let madd = main_modules(&ex.join("ExeFS"), &keep, hooks)?;
    let _ = writeln!(
        s,
        "main_add {}",
        madd.iter()
            .map(|(m, k)| format!("{m}={k}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    sort_load_order(&mut keep);
    let nro_dir = ex.join("romfs_nro");
    let inp = survey_inputs(
        &f,
        &keep,
        nro_dir.is_dir().then_some(nro_dir.as_path()),
        &add,
    )?;
    let src = shader_sources(&inp);
    let _ = writeln!(s, "src graphics\t{}\t{}", src.graphics.0, src.graphics.1);
    for (k, (v, why)) in [
        ("vk_capture", &src.vk_capture),
        ("gl_capture", &src.gl_capture),
        ("glslc_capture", &src.glslc_capture),
        ("ue_decode", &src.ue_decode),
        ("oodle_packs", &src.oodle_packs),
        ("ue_legacy", &src.ue_legacy),
    ] {
        let _ = writeln!(s, "src {k}\t{}\t{why}", if *v { "True" } else { "False" });
    }
    Ok(s)
}

fn collect_nros(d: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(rd) = std::fs::read_dir(d) else { return };
    for p in rd.flatten().map(|e| e.path()) {
        if p.is_dir() {
            collect_nros(&p, out);
        } else if p.extension().is_some_and(|e| e == "nro") {
            out.push(p);
        }
    }
}

/// `check()` of one profile (`profile_guess.py --check`): the line it prints and whether a key differs. `id` is the
/// profile folder's name, `g` its `game.tsv`.
pub fn check_line(
    id: &str,
    g: &nxb_profile::tsv::TsvFile,
    f: &Facts,
    guess: &Guess,
) -> (String, bool) {
    const DEFAULTS: &[(&str, &str)] = &[("op_mode", "console"), ("pad_layout", "simple")];
    let dflt = |k: &str| DEFAULTS.iter().find(|d| d.0 == k).map_or("", |d| d.1);
    let (mut diff, mut unknown, mut later) = (Vec::new(), Vec::new(), Vec::new());
    let keys = [
        "op_mode",
        "pad_layout",
        "modules",
        "mem_reserve",
        "mem_alias",
        "mem_stack",
        "app_pool",
    ];
    for k in keys {
        let (want, got) = (
            g.get_or(k, dflt(k)),
            guess.value(k).unwrap_or(dflt(k)).to_string(),
        );
        let numeric = k.starts_with("mem_") || k == "app_pool";
        let (w, o) = if numeric {
            (
                super::pyfmt::int0(&want).map(|v| format!("{v:#x}")),
                super::pyfmt::int0(&got).map(|v| format!("{v:#x}")),
            )
        } else {
            (Ok(want.clone()), Ok(got.clone()))
        };
        let (Ok(w), Ok(o)) = (w, o) else {
            diff.push((k, got, want));
            continue;
        };
        if w == o {
            continue;
        }
        let entry = (k, o, w);
        if (k == "mem_alias" || k == "app_pool") && f.sysres != 0 {
            later.push(entry);
        } else if (k == "pad_layout" || k == "op_mode") && guess.get(k).is_none() {
            unknown.push(entry);
        } else {
            diff.push(entry);
        }
    }
    let list = |v: &[(&str, String, String)], sep: &str| {
        v.iter()
            .map(|(k, got, want)| format!("{k} {got} (profile {want})"))
            .collect::<Vec<_>>()
            .join(sep)
    };
    let mut s = format!("{id} {}: ", g.get_or("name", ""));
    s.push_str(&if diff.is_empty() {
        "ok".to_string()
    } else {
        format!("DIFF {}", list(&diff, "; "))
    });
    if !later.is_empty() {
        s.push_str(&format!(" | heap check / measured: {}", list(&later, ", ")));
    }
    if !unknown.is_empty() {
        let u: Vec<String> = unknown
            .iter()
            .map(|(k, _, want)| format!("{k} {want}"))
            .collect();
        s.push_str(&format!(" | no signal: {}", u.join(", ")));
    }
    (s, !diff.is_empty())
}
