//! `dm/ios/tools/gen_build.py`: the iOS project's build inputs from the Mac host's real compile line. `make` builds the
//! profile header and the host's Metal shaders for iOS (`MSL_SDK=iphoneos`), `make -n -B dmhost` prints the compile
//! commands, and from them come `Gen/src/<flat name>` (symlinks to the sources, `main.c` as `Gen/main/main.c`, the
//! `_mac` sources and the iOS project's own sources left out) and the `Gen/dm.xcconfig` text (header paths, defines,
//! frameworks). Both `make` runs go through [`Runner`] (the [`nxb_tool::Tool`] trait).
//!
//! The text returned is what the script's standard output holds, which `build_ipa.sh` redirects into
//! `Gen/dm.xcconfig`: the first `make`'s standard output (the script lets it through) and then the three lines.

use super::normpath;
use crate::host::Runner;
use crate::host::pyfmt::{shlex_split, splitlines};
use std::path::Path;

/// `os.path.join(a, b)` for text paths.
fn join(a: &str, b: &str) -> String {
    if b.starts_with('/') || a.is_empty() {
        b.to_string()
    } else if a.ends_with('/') {
        format!("{a}{b}")
    } else {
        format!("{a}/{b}")
    }
}

/// `os.path.dirname`.
fn dirname(p: &str) -> &str {
    match p.rfind('/') {
        Some(0) => "/",
        Some(i) => {
            let d = &p[..i];
            let t = d.trim_end_matches('/');
            if t.is_empty() { "/" } else { t }
        }
        None => "",
    }
}

/// `os.path.basename`.
fn basename(p: &str) -> &str {
    p.rsplit('/').next().unwrap_or(p)
}

/// `os.path.splitext(b)[0]` (leading dots belong to the name).
fn stem(b: &str) -> &str {
    match b.rfind('.') {
        Some(i) if b[..i].chars().any(|c| c != '.') => &b[..i],
        _ => b,
    }
}

/// `os.path.relpath(p, start)` of two absolute, normalised paths.
fn relpath(p: &str, start: &str) -> String {
    let a: Vec<&str> = p.split('/').filter(|c| !c.is_empty()).collect();
    let b: Vec<&str> = start.split('/').filter(|c| !c.is_empty()).collect();
    let n = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut out: Vec<&str> = vec![".."; b.len() - n];
    out.extend(&a[n..]);
    if out.is_empty() {
        ".".into()
    } else {
        out.join("/")
    }
}

/// What the compile commands give.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Inputs {
    /// Source files (`.c`, `.m`, `.S`), normalised, in command order.
    pub srcs: Vec<String>,
    /// `-I` folders, first occurrence order.
    pub incs: Vec<String>,
    /// `-D` flags.
    pub defs: Vec<String>,
    /// `-framework` names.
    pub fws: Vec<String>,
}

/// The host's compile commands (`make -n -B` output) read as the script reads them.
pub fn parse(host: &str, cmd: &str) -> Result<Inputs, String> {
    let mut lines: Vec<Vec<String>> = Vec::new();
    for l in splitlines(cmd) {
        if l.trim_start().starts_with("clang") {
            lines.push(shlex_split(l)?);
        }
    }
    // the host's own link command (-o dmhost) first, stable, as Python's sort
    lines.sort_by_key(|t| !t.iter().any(|x| x == "dmhost"));
    let mut r = Inputs::default();
    for toks in &lines {
        let mut i = 0;
        while i < toks.len() {
            let t = &toks[i];
            if t == "-framework" {
                let f = toks
                    .get(i + 1)
                    .ok_or("gen_build: -framework without a name")?;
                r.fws.push(f.clone());
                i += 2;
                continue;
            }
            if let Some(d) = t.strip_prefix("-I") {
                let d = normpath(&join(host, d));
                if !r.incs.contains(&d) {
                    r.incs.push(d);
                }
            } else if t.starts_with("-D") {
                r.defs.push(t.clone());
            } else if t.ends_with(".c") || t.ends_with(".m") || t.ends_with(".S") {
                r.srcs.push(normpath(&join(host, t)));
            }
            i += 1;
        }
    }
    if r.srcs.is_empty() {
        return Err("gen_build: no source files in the compile command".into());
    }
    let mut dup: Vec<&String> = r
        .srcs
        .iter()
        .filter(|s| r.srcs.iter().filter(|x| x == s).count() > 1)
        .collect();
    dup.sort();
    dup.dedup();
    if !dup.is_empty() {
        let d: Vec<&str> = dup.iter().map(|s| s.as_str()).collect();
        return Err(format!(
            "duplicate source files in the compile command (explicit Makefile entries overlapping a wildcard?): {}",
            d.join(" ")
        ));
    }
    Ok(r)
}

/// The symlinks under `gen` and the three `xcconfig` lines (the script's second half).
pub fn link_and_config(host: &str, gdir: &str, r: &Inputs) -> Result<String, String> {
    let mk = |p: &str| std::fs::create_dir(p).map_err(|e| format!("{p}: {e}"));
    mk(&format!("{gdir}/src"))?;
    mk(&format!("{gdir}/main"))?;
    let ios_sources = normpath(&join(host, "../ios/Sources"));
    let main_c = join(host, "main.c");
    let parent = dirname(host);
    let mut dirs: Vec<&str> = Vec::new();
    for s in &r.srcs {
        let d = dirname(s);
        if !dirs.contains(&d) {
            dirs.push(d);
        }
        let b = basename(s);
        if stem(b).ends_with("_mac") || d == ios_sources {
            continue;
        }
        let link = if *s == main_c {
            format!("{gdir}/main/main.c")
        } else {
            format!("{gdir}/src/{}", relpath(s, parent).replace('/', "__"))
        };
        std::os::unix::fs::symlink(s, &link).map_err(|e| format!("{link}: {e}"))?;
    }
    let paths: Vec<String> = r
        .incs
        .iter()
        .map(String::as_str)
        .chain(dirs)
        .map(|d| format!("\"{d}\""))
        .collect();
    let fws: Vec<String> = r
        .fws
        .iter()
        .filter(|f| *f != "Cocoa")
        .map(|f| format!("-framework {f}"))
        .collect();
    Ok(format!(
        "HEADER_SEARCH_PATHS = $(inherited) {}\nOTHER_CFLAGS = $(inherited) {}\nDM_FRAMEWORKS = {}\n",
        paths.join(" "),
        r.defs.join(" "),
        fws.join(" ")
    ))
}

/// `gen_build.py <dm/host> <Gen>`: run both `make`s (with `GAME` from the runner's environment) and generate, the
/// returned text is the script's standard output.
pub fn generate(r: &Runner, host: &Path, gdir: &Path) -> Result<String, String> {
    let utf8 = |p: &Path| {
        p.to_str()
            .map(str::to_string)
            .ok_or_else(|| format!("{}: not UTF-8", p.display()))
    };
    let host = super::abspath(&utf8(host)?)?;
    let gdir = super::abspath(&utf8(gdir)?)?;
    let first = r.run(
        "make",
        [
            "-s",
            "-C",
            &host,
            "game_profile",
            "host_msl",
            "MSL_SDK=iphoneos",
        ],
        None,
    )?;
    let cmd = r.stdout(
        "make",
        ["-s", "-n", "-B", "-C", &host, "dmhost", "MSL_SDK=iphoneos"],
        None,
    )?;
    let inputs = parse(&host, &cmd)?;
    let mut out = String::from_utf8_lossy(&first.stdout).into_owned();
    out.push_str(&link_and_config(&host, &gdir, &inputs)?);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ios::oracle;
    use std::collections::BTreeMap;

    const CMD: &str = "make: Entering\n  clang -O2 -I../vpx/include -Iinc -DLIB=1 -c ../vpx/a.c -o a.o\nclang -g -I. -I../gfx -I./inc -DX=1 '-DNAME=\"a b\"' -framework Cocoa -framework Metal -o dmhost main.c front_mac.m touch_mac.c ../gfx/nvn/b.c ../ios/Sources/touch.m x.S\necho done\n";

    #[test]
    fn reads_the_compile_line() {
        let r = parse("/e/dm/host", CMD).unwrap();
        assert_eq!(
            r.incs,
            [
                "/e/dm/host",
                "/e/dm/gfx",
                "/e/dm/host/inc",
                "/e/dm/vpx/include"
            ]
        );
        assert_eq!(r.defs, ["-DX=1", "-DNAME=\"a b\"", "-DLIB=1"]);
        assert_eq!(r.fws, ["Cocoa", "Metal"]);
        assert_eq!(r.srcs.first().unwrap(), "/e/dm/host/main.c");
        assert_eq!(r.srcs.last().unwrap(), "/e/dm/vpx/a.c");
        assert!(parse("/e/dm/host", "clang -o x a.c a.c\n").is_err());
        assert!(parse("/e/dm/host", "gcc a.c\n").is_err());
        assert_eq!(relpath("/e/dm/gfx/nvn/b.c", "/e/dm"), "gfx/nvn/b.c");
        assert_eq!(relpath("/e/x", "/e/dm"), "../x");
        assert_eq!(stem(".hidden"), ".hidden");
        assert_eq!(stem("front_mac.m"), "front_mac");
    }

    #[test]
    fn links_and_config() {
        let d = oracle::scratch("genbuild");
        let host = format!("{}/dm/host", d.to_str().unwrap());
        let gdir = format!("{}/Gen", d.to_str().unwrap());
        std::fs::create_dir_all(&gdir).unwrap();
        let r = parse(&host, CMD).unwrap();
        let text = link_and_config(&host, &gdir, &r).unwrap();
        assert!(text.contains("\nDM_FRAMEWORKS = -framework Metal\n"));
        assert_eq!(
            std::fs::read_link(format!("{gdir}/main/main.c")).unwrap(),
            Path::new(&format!("{host}/main.c"))
        );
        assert!(Path::new(&format!("{gdir}/src/gfx__nvn__b.c")).is_symlink());
        assert!(Path::new(&format!("{gdir}/src/vpx__a.c")).is_symlink());
        assert!(!Path::new(&format!("{gdir}/src/host__front_mac.m")).is_symlink());
        assert!(!Path::new(&format!("{gdir}/src/ios__Sources__touch.m")).is_symlink());
        let _ = std::fs::remove_dir_all(&d);
    }

    fn links(gdir: &Path) -> BTreeMap<String, String> {
        let mut m = BTreeMap::new();
        for sub in ["src", "main"] {
            for e in std::fs::read_dir(gdir.join(sub)).unwrap().flatten() {
                m.insert(
                    format!("{sub}/{}", e.file_name().to_string_lossy()),
                    std::fs::read_link(e.path())
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
        m
    }

    /// Oracle: `NXB_ORACLE_ENGINE` and `NXB_ORACLE_GAME` (a profile): the engine's `gen_build.py` and [`generate`]
    /// against the engine's `dm/host`, the standard output and every symlink (name and target) equal.
    #[test]
    fn equals_the_reference_script() {
        let (Some(e), Ok(game)) = (oracle::engine(), std::env::var("NXB_ORACLE_GAME")) else {
            return;
        };
        let d = oracle::scratch("genbuild-oracle");
        let host = e.join("dm/host");
        let (pg, rg) = (d.join("py/Gen"), d.join("rs/Gen"));
        std::fs::create_dir_all(&pg).unwrap();
        std::fs::create_dir_all(&rg).unwrap();
        let o = oracle::python()
            .arg(e.join("dm/ios/tools/gen_build.py"))
            .arg(&host)
            .arg(&pg)
            .env("GAME", &game)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let r = Runner::from_parent()
            .unwrap()
            .with_var("GAME", &game)
            .unwrap();
        let text = generate(&r, &host, &rg).unwrap();
        assert_eq!(text, String::from_utf8(o.stdout).unwrap());
        assert_eq!(links(&rg), links(&pg));
        let _ = std::fs::remove_dir_all(&d);
    }
}
