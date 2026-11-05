//! What the app stage of `nxb build` takes from these stages: the companion app, the icon, the app settings and
//! the game language, made as the NSP script makes them before it calls `pack_app.py`.

use super::{Runner, companion, icon, settings};
use crate::Args;
use crate::graph::Build;
use std::path::{Path, PathBuf};

/// The parts of the app made here.
#[derive(Debug)]
pub struct AppParts {
    /// `Contents/MacOS/nxnative`.
    pub companion: PathBuf,
    /// `Contents/Resources/AppIcon.icns` (none: the game has no icon and there is no previous app).
    pub icns: Option<PathBuf>,
    /// `launcher.json`.
    pub settings: PathBuf,
    /// The game language (`DMLanguage`, `DM_LANG`).
    pub lang: String,
}

/// Make the parts in `<work>/app`: the companion app (given by `--companion-app`, else built and reused while its
/// inputs are unchanged), the icon from the packed version's `icon.jpg` (else the previous app's), the settings.
pub fn prepare(a: &Args, b: &Build, previous: Option<&Path>) -> Result<AppParts, String> {
    let w = &b.work;
    let dir = w.join("app");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let r = Runner::from_parent()?;
    let companion = match &a.companion {
        Some(c) => c.clone(),
        None => {
            let out = dir.join("nxnative");
            companion::ensure(&r, &b.engine, &out, &w.join("tools/dmhost"))?;
            out
        }
    };
    let ex = w.join("extract").join(&b.package);
    let icns = match ex.join("icon.jpg") {
        jpg if jpg.is_file() => {
            let out = dir.join("AppIcon.icns");
            icon::make_icns(&r, &jpg, &out)?;
            Some(out)
        }
        _ => previous
            .map(|p| p.join("Contents/Resources/AppIcon.icns"))
            .filter(|p| p.is_file()),
    };
    // the game's languages from the packed version's control data, else the base game's
    let base_ex = w.join("extract/v0");
    let control =
        settings::languages_control(&[&ex, &base_ex]).unwrap_or_else(|| ex.join("control.nacp"));
    let have = settings::languages(&control);
    let want = match &a.language {
        Some(l) => settings::language_code(l)
            .ok_or_else(|| format!("unknown language {l} (--language)"))?,
        None => "en-US",
    };
    let lang = settings::game_language(want, a.language.is_some(), &have)?;
    let out = dir.join(nxb_pack::macapp::SETTINGS_JSON);
    let env = |k: &str| match k {
        "GFXAPI" => Some("nvn".to_string()),
        "SETTINGS_PLATFORM" => Some("mac".to_string()),
        "DM_LANG" => Some(lang.clone()),
        _ => std::env::var(k).ok(),
    };
    let patched = w.join(format!("patched_{}", b.package));
    settings::write(&b.engine, &b.title, &patched, &control, &out, &env)?;
    Ok(AppParts {
        companion,
        icns,
        settings: out,
        lang,
    })
}
