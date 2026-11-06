//! A multi-program application (a profile with a `programs` key, Super Mario 3D All-Stars): the profile of its first
//! program lists the others as `<index>=<title ID>`. Each other program is built as a game of its own, in a work
//! folder of its own, up to its shaders (`other_programs` of the NSP script: extract, shader split and Mac shaders,
//! no host and no app). The Mac app packs them with the first program, a Mac-only build lists them in `programs.tsv`.

use std::sync::Arc;

use crate::Args;
use crate::events::Progress;
use crate::graph::{self, Build};
use crate::mac_run::Run;

/// The indexes of the other programs the profile of `b` lists (none for a program of its own and for a game of one
/// program).
fn listed(b: &Build) -> Vec<u64> {
    if b.program != 0 {
        return Vec::new();
    }
    b.profile
        .1
        .get("programs")
        .map(|v| {
            v.split(' ')
                .filter_map(|w| w.split_once('='))
                .filter_map(|(n, _)| n.parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// The sub-build of program `n`: its nodes up to the Mac shaders, run.
fn sub_build(a: &Args, n: u64, p: &Arc<Progress>) -> Result<Build, String> {
    let mut sa = a.clone();
    sa.program = Some(u32::try_from(n).map_err(|_| format!("program {n}: too large"))?);
    sa.out = None;
    sa.provisional = false;
    sa.gpu = Some("none".into());
    // the work folders of the programs must differ: the default `<title ID>_build` does, a given one gets a folder
    // for the program
    sa.work_dir = a.work_dir.as_ref().map(|d| d.join(format!("program-{n}")));
    let mut sb = graph::plan(&sa)?;
    if sb.prov {
        return Err(crate::exit::fail(
            2,
            format!(
                "program {n} ({}): the engine has no profile for it, and a program of a multi-program application is built from its own profile",
                sb.title
            ),
        ));
    }
    sb.nodes.truncate(3);
    sb.gpus.clear();
    if sa.seen_shaders {
        sa = crate::options::seen_shaders(&sa, &sb, p)?;
    }
    p.say(&format!("==== program {n}"));
    crate::stages::run_nodes(&sa, &sb, p, &[])?;
    Ok(sb)
}

/// What the Mac app takes of the other programs (`mac_programs` of the script's Mac app route): built first, as the
/// app is packed from all of them.
pub(crate) fn before_app(
    a: &Args,
    b: &Build,
    p: &Arc<Progress>,
) -> Result<Vec<nxb_pack::macapp::Program>, String> {
    if a.platform != "mac-app" {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for n in listed(b) {
        let s = sub_build(a, n, p)?;
        let ex = s.work.join("extract").join(&s.package);
        out.push(nxb_pack::macapp::Program {
            n: n.to_string(),
            patched: s.work.join(format!("patched_{}", s.package)),
            romfs: ex.join("RomFS.img"),
            msl: s.shaders.clone(),
            tables: s.profile.0.clone(),
            html: ex.join("html"),
        });
    }
    Ok(out)
}

/// The runs of the other programs of a Mac-only build, after the first program's: each is built here, and runs
/// with the first program's host and fonts.
pub(crate) fn mac_runs(
    a: &Args,
    b: &Build,
    host: &std::path::Path,
    font: &std::path::Path,
    p: &Arc<Progress>,
) -> Result<Vec<(u64, Run)>, String> {
    let mut out = Vec::new();
    for n in listed(b) {
        let s = sub_build(a, n, p)?;
        out.push((
            n,
            crate::mac_run::run_of(a, &s, host, font, a.seen_shaders)?,
        ));
    }
    Ok(out)
}
