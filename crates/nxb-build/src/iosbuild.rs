//! `nxb build --platform ios`: the NSP script's iOS branch as stages. Extract and split, then the shader step (the
//! iOS libraries `<work>/shader/ios/b-<V>`, from the same translation as the Mac ones), the "app build" (the script's
//! `build_ipa.sh` up to `xcodebuild` and the signature, in [`crate::ios::ipa`]) and the "IPA pack". The rewrite
//! (`patched_<package>`) comes from the work folder, as for the Mac app. Events are those of the other platform.

use crate::Args;
use crate::events::{Progress, Update};
use crate::graph::{Build, Node};
use crate::host::Runner;
use crate::ios::{ipa, ipapack};
use crate::order::{Stage, Work};
use nxb_model::ContentKey;
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Arc;

/// The stages of an iOS build.
pub(crate) fn nodes(a: &Args, b: &Build) -> Vec<Node> {
    let key = |name: &str, f: &[(&str, String)]| {
        let mut k = ContentKey::builder(&format!("nxb-build ios {name} 1"));
        for (n, v) in f {
            k = k.field_str(n, v);
        }
        k.finish()
    };
    let reuse = |i: usize| b.nodes.get(i).and_then(|n| n.reused.clone());
    vec![
        Node {
            name: "extract".into(),
            deps: vec![],
            key: b.nodes[0].key,
            reused: reuse(0),
        },
        Node {
            name: "shader split".into(),
            deps: vec![0],
            key: b.nodes[1].key,
            reused: reuse(1),
        },
        Node {
            name: "iOS shaders".into(),
            deps: vec![1],
            key: key("shaders", &[("libraries", b.shaders.display().to_string())]),
            reused: reuse(2),
        },
        Node {
            name: "app build".into(),
            deps: vec![2],
            key: key(
                "app",
                &[
                    ("out", b.out.display().to_string()),
                    ("package", b.package.clone()),
                    ("gpu", a.gpu.clone().unwrap_or_default()),
                ],
            ),
            reused: None,
        },
        Node {
            name: "IPA pack".into(),
            deps: vec![3],
            key: key("pack", &[("out", b.out.display().to_string())]),
            reused: None,
        },
    ]
}

/// The script's environment for `build_ipa.sh`, from the build's plan.
fn inputs(a: &Args, b: &Build) -> Result<ipa::Inputs, String> {
    let w = &b.work;
    let ex = w.join("extract").join(&b.package);
    let base_ex = w.join("extract/v0");
    let p = |x: PathBuf| Some(x.to_string_lossy().into_owned());
    let gfx = b.profile.1.get("graphics").cloned().unwrap_or_default();
    if matches!(gfx.split(' ').next(), Some("vulkan" | "gl")) {
        return Err(
            "unsupported: a Vulkan or OpenGL game in nxb build --platform ios (run the script)"
                .into(),
        );
    }
    let control = crate::host::settings::languages_control(&[&ex, &base_ex])
        .unwrap_or_else(|| ex.join("control.nacp"));
    let have = crate::host::settings::languages(&control);
    let want = match &a.language {
        Some(l) => crate::host::settings::language_code(l)
            .ok_or_else(|| format!("unknown language {l} (--language)"))?,
        None => "en-US",
    };
    let lang = crate::host::settings::game_language(want, a.language.is_some(), &have)?;
    let icon = [&ex, &base_ex]
        .iter()
        .map(|d| d.join("icon.jpg"))
        .find(|f| f.is_file());
    let nacp = [&ex, &base_ex]
        .iter()
        .map(|d| d.join("control.nacp"))
        .find(|f| f.is_file());
    let name = a
        .app_name
        .clone()
        .or_else(|| {
            nacp.as_ref()
                .map(|n| crate::ios::nacp::name(n))
                .filter(|s| !s.is_empty())
        })
        .or_else(|| b.profile.1.get("name").cloned());
    let version = [&ex, &base_ex]
        .iter()
        .map(|d| crate::ios::nacp::version(&d.join("control.nacp")))
        .find(|v| !v.is_empty());
    let gpus = match a.gpu.as_deref().map(str::trim).filter(|g| !g.is_empty()) {
        Some(g) => g.to_string(),
        None if b.profile.1.get("pso_mtl4").map(String::as_str) == Some("on") => {
            let pg = b.profile.1.get("pso_gpu").cloned().unwrap_or_default();
            let j = pg.split_whitespace().collect::<Vec<_>>().join(",");
            if j.is_empty() { "g18p".into() } else { j }
        }
        None => "none".into(),
    };
    let romfs = if ex.join("RomFS.img").is_file() {
        ex.join("RomFS.img")
    } else {
        ex.join("RomFS")
    };
    Ok(ipa::Inputs {
        engine: b.engine.clone(),
        game: b.title.clone(),
        w: w.join("ipa").to_string_lossy().into_owned(),
        patched: p(w.join(format!("patched_{}", b.package))),
        romfs: p(romfs),
        msl_ios: p(w.join("shader/ios")),
        gfxapi: Some("nvn".into()),
        display_name: name,
        bundle_id: a.bundle_id.clone(),
        lang: Some(lang),
        control: p(control),
        icon: icon.and_then(p),
        app_version: version,
        xcode_dd: p(w.join("xcode-derived-data")),
        team_id: std::env::var("DM_TEAM_ID").ok(),
        show_fps: Some("0".into()),
        pso_gpus: Some(gpus),
        aoc: p(w.join("aoc")).filter(|_| w.join("aoc").is_dir()),
        user: std::env::var("USER").unwrap_or_default(),
        ..ipa::Inputs::default()
    })
}

/// The iOS build.
pub(crate) fn run(a: &Args, b: &Build, p: &Arc<Progress>) -> Result<(), String> {
    p.plan(b.nodes.len());
    let mut a2 = a.clone();
    a2.also_ios = true;
    let a2 = &a2;
    let state: RefCell<Option<(ipa::State, ipa::Inputs, PathBuf)>> = RefCell::new(None);
    let state = &state;
    let step = |s: &str| {
        let line = s.to_string();
        if line.starts_with("generate Gen/") {
            p.set(Update {
                step: Some("generating sources"),
                ..Update::default()
            });
        } else if line.starts_with("pipeline archive for") {
            p.set(Update {
                step: Some("pipeline archive (metal-tt)"),
                ..Update::default()
            });
        } else if line.starts_with("xcodebuild") {
            p.set(Update {
                step: Some("xcodebuild"),
                ..Update::default()
            });
        } else if line.starts_with("[RED]") {
            p.say(&line);
        }
    };
    let mut stages: Vec<Stage<'_>> = Vec::new();
    for (i, n) in b.nodes.iter().enumerate() {
        let work = match (&n.reused, i) {
            (Some(r), _) => Work::Reused(format!("reused: {r}")),
            (_, 0) => Work::Turn(Box::new(move || crate::graph::stage_extract(a, b, p))),
            (_, 1) => Work::Reused("in the extract's pass".into()),
            (_, 2) => Work::Turn(Box::new(move || {
                crate::pipeline::shaders_and_archive(a2, b, p).map(|o| o.shaders)
            })),
            (_, 3) => Work::Turn(Box::new(move || {
                let inp = inputs(a, b)?;
                let r = Runner::from_parent()?;
                let st = ipa::generate(&r, false, Some(&b.out), &inp, &mut |l| step(l))?;
                let app = ipapack::build_app(&st, &inp, &mut |l| step(l))?;
                *state.borrow_mut() = Some((st, inp, app));
                Ok(b.out.display().to_string())
            })),
            _ => Work::Turn(Box::new(move || {
                let (st, inp, app) = state
                    .borrow_mut()
                    .take()
                    .ok_or("the app build did not run")?;
                let out = ipapack::pack_ipa(&st, &inp, &app, &mut |l| step(l))?;
                let size = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
                Ok(format!("{}, {} MB", out.display(), size >> 20))
            })),
        };
        stages.push(Stage {
            name: Some(n.name.clone()),
            deps: n.deps.clone(),
            work,
        });
    }
    crate::order::run(p, stages)?;
    p.say(&format!("done: {}", b.out.display()));
    Ok(())
}
