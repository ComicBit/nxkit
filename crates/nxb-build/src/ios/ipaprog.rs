//! Games of several programs (`PROGRAMS_TSV` of `build_ipa.sh`) and the per-program table step shared with the
//! one-program flow.

use super::ipa::{Inputs, copy};
use super::script::die;
use crate::host::Runner;
use crate::host::profile::{Game, Profiles};
use std::path::{Path, PathBuf};

/// What the tables step reads from the generation.
pub(crate) struct TablesCtx<'a> {
    pub i: &'a Inputs,
    pub r: &'a Runner,
    pub exports: &'a [(&'a str, String)],
}

/// The inputs of one program's tables: its profile id and profile, rewriter output folder, graphics API and the
/// control data that lists its languages.
#[derive(Clone, Copy)]
pub(crate) struct ProgIn<'a> {
    pub game: &'a str,
    pub g: &'a Game,
    pub patched: &'a str,
    pub gfx: &'a str,
    pub vnacp: &'a str,
}

/// One program of a game of several (`prog_set`).
#[derive(Clone, Debug)]
pub(crate) struct Prog {
    pub n: String,
    pub game: String,
    pub g: Game,
    pub patched: String,
    pub romfs: String,
    pub msl: String,
    pub gfx: String,
    /// `gp modules` (the profile's list, as `prog_set` sets it).
    pub mods: Vec<String>,
    pub marker: String,
}

impl Prog {
    /// The folder of the shader libraries in the app (`vk`, `gl` or `b-<chain>`).
    pub(crate) fn msldir(&self, v: &str) -> String {
        match self.gfx.as_str() {
            "vulkan" => "vk".into(),
            "gl" => "gl".into(),
            _ => format!("b-{v}"),
        }
    }
}

/// `prog_rows` and `prog_set` for every row: index, profile, rewritten modules, RomFS, iOS shaders, graphics API.
pub(crate) fn load(tsv: &str, profiles: &Profiles) -> Result<Vec<Prog>, String> {
    let text = std::fs::read_to_string(tsv).map_err(|_| die(format!("no PROGRAMS_TSV {tsv}")))?;
    let mut out = Vec::new();
    for row in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        // `IFS=<tab> read -r`: runs of tabs are one separator, the last name takes the rest
        let f: Vec<&str> = row.split('\t').filter(|x| !x.is_empty()).collect();
        let at = |k: usize| f.get(k).copied().unwrap_or("").to_string();
        let game = at(1);
        let g = profiles
            .load(&game)
            .map_err(|_| die(format!("no game profile {game}")))?;
        let gd = g.dir.display().to_string();
        let gp = |k: &str| g.get(k).map_err(|_| die(format!("game profile {gd}: {k}")));
        out.push(Prog {
            n: at(0),
            patched: at(2),
            romfs: at(3),
            msl: at(4),
            gfx: f.get(5..).map(|r| r.join("\t")).unwrap_or_default(),
            mods: gp("modules")?
                .split_whitespace()
                .map(str::to_string)
                .collect(),
            marker: gp("romfs_marker")?,
            game,
            g,
        });
    }
    Ok(out)
}

/// `program_tables`: one program's rewriter tables, shader tables and app binary settings into `td`, its settings.
pub(crate) fn program_tables(
    c: &TablesCtx<'_>,
    td: &Path,
    p: &ProgIn<'_>,
    say: &mut dyn FnMut(&str),
) -> Result<nxb_pyjson::Value, String> {
    let ProgIn {
        game,
        g,
        patched,
        gfx,
        vnacp,
    } = *p;
    let gd = g.dir.display().to_string();
    let mut tables: Vec<PathBuf> = Vec::new();
    for suf in [".fixups.tsv", ".gates.tsv", ".cuts.tsv", ".obs.tsv"] {
        let mut m: Vec<PathBuf> = std::fs::read_dir(patched)
            .map(|d| d.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        m.retain(|p| {
            let n = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            n.ends_with(suf) && !n.starts_with('.') && n.len() > suf.len()
        });
        if m.is_empty() {
            return Err(die("copy rewriter tables"));
        }
        tables.extend(m);
    }
    for t in &tables {
        copy(
            t,
            &td.join(t.file_name().unwrap_or_default()),
            "copy rewriter tables",
        )?;
    }
    let nro = Path::new(patched).join("nro.tsv");
    if nro.is_file() {
        copy(&nro, &td.join("nro.tsv"), "copy the NRO table")?;
    }
    let mut shader_tables = vec!["shader_map.tsv", "pso_manifest.tsv"];
    if !matches!(gfx, "vulkan" | "gl") {
        shader_tables.push("c1_full.tsv");
    }
    for t in &shader_tables {
        copy(&g.dir.join(t), &td.join(t), "copy shader tables")?;
    }
    if !matches!(gfx, "vulkan" | "gl") {
        if g.dir.join("cb_extent.tsv").is_file() {
            copy(
                &g.dir.join("cb_extent.tsv"),
                &td.join("cb_extent.tsv"),
                "copy cb_extent.tsv",
            )?;
        } else {
            say(&format!(
                "no {gd}/cb_extent.tsv: the host copies whole uniform bindings"
            ));
        }
        for f in ["shader_pairs.tsv", "pso_pairs.tsv"] {
            if g.dir.join(f).is_file() {
                copy(&g.dir.join(f), &td.join(f), &format!("copy {f}"))?;
            }
        }
    }
    let lj = td.join("launcher.json");
    let env_of = |k: &str| -> Option<String> {
        if k == "GFXAPI" {
            return Some(gfx.to_string());
        }
        c.exports
            .iter()
            .find(|(n, _)| *n == k)
            .map(|(_, v)| v.clone())
            .or_else(|| c.r.env().get(k).map(|v| v.to_string_lossy().into_owned()))
    };
    let settings = crate::host::settings::build(
        &c.i.engine,
        game,
        Path::new(patched),
        Path::new(vnacp),
        &env_of,
    )
    .map_err(|_| die("app binary settings (dm/games/settings.py)"))?;
    std::fs::write(&lj, format!("{}\n", nxb_pyjson::dumps_indent(&settings, 1)))
        .map_err(|_| die("app binary settings (dm/games/settings.py)"))?;
    let list = settings
        .get("settings")
        .and_then(|s| s.as_list())
        .unwrap_or(&[]);
    let envs: Vec<&str> = list.iter().filter_map(|s| s.get("env")?.as_str()).collect();
    say(&format!("app binary: {}", envs.join(" ")));
    Ok(settings)
}

/// The pack plan of a game of several programs: each program's RomFS, shader libraries, HTML pages and `nn::jit`
/// image under `p<index>/` (the script's `PROGRAMS_TSV` branch).
pub(crate) fn plan(
    st: &super::ipa::State,
    i: &Inputs,
    app: &Path,
    mut skip: Vec<String>,
    p_root: &str,
    say: &mut dyn FnMut(&str),
) -> Result<super::ipapack::Plan, String> {
    let mut maps: Vec<(String, PathBuf)> = vec![(p_root.to_string(), app.to_path_buf())];
    if !st.aoc.is_empty() && Path::new(&st.aoc).is_dir() {
        maps.push((format!("{p_root}/aoc"), st.aoc.clone().into()));
    }
    let mut zs: Vec<String> = Vec::new();
    for p in &st.progs {
        skip.push(format!("{p_root}/p{}", p.n));
        let n = &p.n;
        if Path::new(&p.romfs).is_file() {
            maps.push((format!("{p_root}/p{n}/romfs.img"), (&p.romfs).into()));
        } else {
            maps.push((format!("{p_root}/p{n}/romfs"), (&p.romfs).into()));
        }
        let d = p.msldir(&st.chain);
        maps.push((
            format!("{p_root}/msl/p{n}/{d}"),
            format!("{}/{d}", p.msl).into(),
        ));
        let rd = Path::new(&p.romfs)
            .parent()
            .map(|x| x.to_string_lossy().into_owned())
            .unwrap_or_else(|| ".".into());
        if Path::new(&format!("{rd}/html")).is_dir() {
            maps.push((format!("{p_root}/p{n}/html"), format!("{rd}/html").into()));
        }
        if Path::new(&format!("{rd}/jit")).is_dir() {
            maps.push((format!("{p_root}/p{n}/jit"), format!("{rd}/jit").into()));
            say(&format!("program {n}: nn::jit image {rd}/jit"));
        }
        let z =
            p.g.get("zip_store")
                .map_err(|_| die(format!("game profile {}: zip_store", p.g.dir.display())))?;
        zs.extend(
            z.split(':')
                .filter(|s| !s.is_empty())
                .map(str::to_lowercase),
        );
    }
    say("pack IPA (already-compressed game files stored, everything else deflated)");
    zs.sort();
    zs.dedup();
    let mut skip_suffix = Vec::new();
    // no program of such a game sets pso_mtl4 so far: the pulling libraries stay out unless IOS_PULL=1
    if st.pso.is_empty() && super::ipa::or(&i.ios_pull, "0") != "1" {
        skip_suffix.push("_bp.metallib".to_string());
    }
    let o = nxb_pack::ipa::PackOptions {
        store: zs,
        skip: skip
            .iter()
            .map(|s| s.trim_matches('/').to_string())
            .collect(),
        skip_suffix,
        compress: nxb_pack::ipa::ZLIB,
        reuse: true,
    };
    Ok((maps, skip, o))
}

#[cfg(test)]
#[path = "ipaprog_tests.rs"]
mod tests;
