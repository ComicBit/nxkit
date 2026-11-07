//! `nxb build --platform ios`: the NSP script's iOS branch as stages. Extract and split, then the shader step (the
//! iOS libraries `<work>/shader/ios/b-<V>`, from the same translation as the Mac ones), the "JIT capture" of a game
//! that imports `nn::jit` ([`crate::boot::jit`]), the "app build" (the script's
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
    let mut nodes = vec![
        Node {
            name: b.nodes[0].name.clone(),
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
            name: "JIT capture".into(),
            deps: vec![2],
            key: key("jit", &[("work", b.work.display().to_string())]),
            reused: None,
        },
    ];
    let one = |name: &str| {
        vec![Node {
            name: name.into(),
            deps: vec![3],
            key: key(name, &[("out", b.out.display().to_string())]),
            reused: None,
        }]
    };
    if a.shell_framework.is_some() {
        nodes.extend(one("game framework"));
    } else if a.prepare_xcode {
        nodes.extend(one("prepare Xcode"));
    } else {
        nodes.extend(vec![
            Node {
                name: "app build".into(),
                deps: vec![3],
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
                deps: vec![4],
                key: key("pack", &[("out", b.out.display().to_string())]),
                reused: None,
            },
        ]);
    }
    nodes
}

/// The script's environment for `build_ipa.sh`, from the build's plan.
fn inputs(a: &Args, b: &Build) -> Result<ipa::Inputs, String> {
    let w = &b.work;
    let ex = w.join("extract").join(&b.package);
    let base_ex = w.join("extract/v0");
    let p = |x: PathBuf| Some(x.to_string_lossy().into_owned());
    if crate::layer::gfx(b)? != crate::layer::Gfx::Nvn {
        return Err(
            "unsupported: a Vulkan or OpenGL game in nxb build --platform ios (run the script)"
                .into(),
        );
    }
    if !ex.join("jit/jit.img").is_file() && crate::boot::jit::uses_jit(&ex.join("ExeFS"))? {
        return Err(format!(
            "main imports nn::jit, and the JIT capture made no image of the code the game's JIT plugin makes ({})",
            ex.join("jit/jit.img").display()
        ));
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
        guard_tables: crate::pipeline::guard_tables(a, b),
        gfxapi: Some("nvn".into()),
        display_name: name,
        bundle_id: a.bundle_id.clone(),
        lang: Some(lang),
        control: p(control),
        icon: icon.and_then(p),
        app_version: version,
        xcode_dd: p(w.join("xcode-derived-data")),
        team_id: a
            .team_id
            .clone()
            .or_else(|| std::env::var("DM_TEAM_ID").ok()),
        // the script's default is on: the counter shows unless --no-show-fps
        show_fps: Some(if a.show_fps.unwrap_or(true) { "1" } else { "0" }.into()),
        env_file: crate::options::env_file(a, w)?.and_then(p),
        copy_romfs: (a.romfs.as_deref() == Some("separate") || a.shell_framework.is_some())
            .then(|| "NO".into()),
        full_romfs: std::env::var("DM_FULL_ROMFS").ok(),
        framework: a.framework.then(|| "1".into()),
        framework_only: a
            .shell_framework
            .as_deref()
            .map(|f| crate::options::expand(f).to_string_lossy().into_owned()),
        pso_gpus: Some(gpus),
        aoc: p(w.join("aoc")).filter(|_| w.join("aoc").is_dir()),
        user: std::env::var("USER").unwrap_or_default(),
        ..ipa::Inputs::default()
    })
}

/// The iOS build.
pub(crate) fn run(a: &Args, b: &Build, p: &Arc<Progress>) -> Result<(), String> {
    p.plan_more(b.nodes.len());
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
            (_, 0) => Work::Turn(Box::new(move || crate::extract_stage::run(a, b, p))),
            (_, 1) if b.split_fused() => Work::Reused("in the extract's pass".into()),
            (_, 1) => Work::Turn(Box::new(move || crate::extract_stage::split(a, b, p))),
            (_, 2) => Work::Turn(Box::new(move || {
                crate::pipeline::shaders_and_archive(a2, b, p).map(|o| o.shaders)
            })),
            (_, 3) => Work::Turn(Box::new(move || crate::boot::jit::stage(b, p))),
            (_, 4) if a.shell_framework.is_some() => Work::Turn(Box::new(move || {
                // route C's shell: the game's framework alone, the game files stay where they are
                let inp = inputs(a, b)?;
                let r = Runner::from_parent()?;
                let fw = ipa::run(&r, ipa::Target::Ipa(None), &inp, &mut |l| step(l))?;
                Ok(fw.display().to_string())
            })),
            (_, 4) if a.prepare_xcode => Work::Turn(Box::new(move || prepare_xcode(a, b, &step))),
            (_, 4) => Work::Turn(Box::new(move || {
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
    if a.keep_extracted == Some(false) && a.shell_framework.is_none() && !a.prepare_xcode {
        let ex = b.work.join("extract").join(&b.package);
        p.say(&format!(
            "deleting the extracted game files: {}",
            ex.display()
        ));
        std::fs::remove_dir_all(&ex).map_err(|e| format!("{}: {e}", ex.display()))?;
    }
    match (&a.shell_framework, a.prepare_xcode) {
        (Some(f), _) => p.say(&format!(
            "game framework: {}",
            crate::options::expand(f)
                .join(format!("DmGame{}.framework", b.title))
                .display()
        )),
        (None, true) => {}
        _ => p.say(&format!("done: {}", b.out.display())),
    }
    Ok(())
}

/// `--prepare-xcode`: the per-game Xcode project, registered in the shared workspace (the script's `prepare Xcode`).
fn prepare_xcode(a: &Args, b: &Build, step: &dyn Fn(&str)) -> Result<String, String> {
    let r = Runner::from_parent()?;
    let mut inp = inputs(a, b)?;
    let project = b.work.join("xcode/DmIOS.xcodeproj");
    let home = if a.nsp_dir.is_file() {
        a.nsp_dir.parent().map(PathBuf::from).unwrap_or_default()
    } else {
        a.nsp_dir.clone()
    };
    let workspace = a.xcode_workspace.as_deref().map_or_else(
        || {
            std::path::absolute(&home)
                .unwrap_or(home.clone())
                .parent()
                .map(PathBuf::from)
                .unwrap_or_default()
                .join("NxNative.xcworkspace")
        },
        crate::options::expand,
    );
    if workspace.extension().is_none_or(|e| e != "xcworkspace") {
        return Err(crate::exit::fail(
            2,
            "--xcode-workspace must end in .xcworkspace",
        ));
    }
    let saved = crate::ios::workspace::signing_settings(&r, &project)?;
    let get = |k: &str| saved.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
    let bundle = a
        .bundle_id
        .clone()
        .or_else(|| get("PRODUCT_BUNDLE_IDENTIFIER"))
        .unwrap_or_else(|| format!("org.nxnative.{}", b.title.to_lowercase()));
    inp.bundle_id = Some(bundle.clone());
    if inp.team_id.is_none() {
        inp.team_id = get("DEVELOPMENT_TEAM");
    }
    if let Some(note) = crate::ios::workspace::check_bundle_id(&r, &workspace, &project, &bundle)? {
        step(&note);
    }
    inp.xcode_project_dir = project.parent().map(|d| d.to_string_lossy().into_owned());
    ipa::run(&r, ipa::Target::Prepare, &inp, &mut |l| step(l))?;
    let derived = workspace.with_file_name(format!(
        "{}-derived-data",
        workspace
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    ));
    let name = inp.display_name.clone().unwrap_or_else(|| b.title.clone());
    crate::ios::workspace::register_project(
        &workspace,
        &project,
        &name,
        &derived.to_string_lossy(),
        &inp.user,
    )?;
    Ok(format!(
        "ready: {}; game project: {}",
        workspace.display(),
        project.display()
    ))
}
