use super::*;
use crate::host::profile::Game;

/// A build in `target/iosbuild-tests` with the Mac plan's first stages, the profile's `graphics` key set to `api`.
fn build(dir: &str, api: Option<&str>) -> Build {
    let w = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/iosbuild-tests")
        .join(format!("{dir}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&w);
    std::fs::create_dir_all(&w).unwrap();
    let mut b = Build::for_test(w.clone(), w.join("profile"));
    b.shaders = w.join("shader/cache/b-1");
    b.out = w.join("Game.ipa");
    if let Some(g) = api {
        b.profile.1.insert("graphics".into(), g.into());
    }
    b.nodes = ["extract v0", "shader split", "Mac shaders"]
        .iter()
        .enumerate()
        .map(|(i, n)| Node {
            name: (*n).into(),
            deps: i.checked_sub(1).into_iter().collect(),
            key: ContentKey::builder(n).finish(),
            reused: None,
        })
        .collect();
    b
}

fn names(n: &[Node]) -> Vec<&str> {
    n.iter().map(|n| n.name.as_str()).collect()
}

fn deps(n: &[Node]) -> Vec<Vec<usize>> {
    n.iter().map(|n| n.deps.clone()).collect()
}

#[test]
fn a_vulkan_or_gl_game_has_the_layer_stage_and_no_nvn_stages() {
    let mut keys = Vec::new();
    for (api, stage) in [("vulkan", "Vulkan shaders"), ("gl", "GL shaders")] {
        let b = build(&format!("plan-{api}"), Some(api));
        let n = nodes(&Args::default(), &b);
        assert_eq!(names(&n), ["extract v0", stage, "app build", "IPA pack"]);
        assert_eq!(deps(&n), [vec![], vec![0], vec![1], vec![2]]);
        assert!(n[1].reused.is_none());
        keys.push(n[1].key);
        let inp = inputs(&Args::default(), &b).unwrap();
        assert_eq!(inp.gfxapi.as_deref(), Some(api));
        assert_eq!(
            inp.msl_ios.as_deref().map(PathBuf::from),
            Some(b.work.join("shader/ios"))
        );
        let _ = std::fs::remove_dir_all(&b.work);
    }
    assert_ne!(keys[0], keys[1]);
}

#[test]
fn the_layer_stage_feeds_the_framework_and_the_xcode_project() {
    let b = build("plan-route", Some("vulkan"));
    let a = Args {
        prepare_xcode: true,
        ..Args::default()
    };
    let n = nodes(&a, &b);
    assert_eq!(names(&n), ["extract v0", "Vulkan shaders", "prepare Xcode"]);
    assert_eq!(n[2].deps, [1]);
    let a = Args {
        shell_framework: Some("fw".into()),
        ..Args::default()
    };
    let n = nodes(&a, &b);
    assert_eq!(
        names(&n),
        ["extract v0", "Vulkan shaders", "game framework"]
    );
    assert_eq!(n[2].deps, [1]);
    let _ = std::fs::remove_dir_all(&b.work);
}

#[test]
fn an_nvn_game_keeps_the_split_and_the_chain() {
    let b = build("plan-nvn", None);
    let n = nodes(&Args::default(), &b);
    assert_eq!(
        names(&n),
        [
            "extract v0",
            "shader split",
            "iOS shaders",
            "app build",
            "IPA pack"
        ]
    );
    assert_eq!(deps(&n), [vec![], vec![0], vec![1], vec![2], vec![3]]);
    assert_eq!(n[1].key, b.nodes[1].key);
    assert_eq!(
        inputs(&Args::default(), &b).unwrap().gfxapi.as_deref(),
        Some("nvn")
    );
    let _ = std::fs::remove_dir_all(&b.work);
}

/// The entry names of a ZIP archive, from its central directory.
fn zip_names(f: &Path) -> Vec<String> {
    let d = std::fs::read(f).unwrap();
    let u16_at = |i: usize| u16::from_le_bytes([d[i], d[i + 1]]) as usize;
    let u32_at = |i: usize| u32::from_le_bytes([d[i], d[i + 1], d[i + 2], d[i + 3]]) as usize;
    let end = (0..=d.len() - 22)
        .rev()
        .find(|&i| d[i..i + 4] == [0x50, 0x4b, 5, 6])
        .unwrap();
    let mut at = u32_at(end + 16);
    let mut out = Vec::new();
    for _ in 0..u16_at(end + 10) {
        assert_eq!(d[at..at + 4], [0x50, 0x4b, 1, 2]);
        let (n, x, c) = (u16_at(at + 28), u16_at(at + 30), u16_at(at + 32));
        out.push(String::from_utf8_lossy(&d[at + 46..at + 46 + n]).into_owned());
        at += 46 + n + x + c;
    }
    out
}

#[test]
fn the_ipa_holds_the_vulkan_or_gl_library_where_the_iphone_reads_it() {
    for (api, dir) in [("vulkan", "vk"), ("gl", "gl")] {
        let b = build(&format!("pack-{api}"), Some(api));
        let w = &b.work;
        let mk = |p: &str, data: &str| {
            let f = w.join(p);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, data).unwrap();
        };
        mk(
            "engine/dm/host/gen/Game/game_profile.h",
            "#define DM_GAME_PSO_MTL4 0\n",
        );
        mk("profile/Game/game.tsv", "zip_store\t.bank\n");
        mk("romfs/data.bin", "game");
        mk("app/DmIOS", "binary");
        mk("app/msl/stale.metallib", "skipped");
        mk(&format!("shader/ios/{dir}/{dir}_shaders.metallib"), "lib");
        mk(&format!("shader/ios/{dir}/{dir}_shaders.tsv"), "index");
        mk("shader/ios/b-1/fs_b.metallib", "not this API");
        let s = |p: &str| w.join(p).to_string_lossy().into_owned();
        let inp = ipa::Inputs {
            engine: w.join("engine"),
            game: "Game".into(),
            gfxapi: Some(api.into()),
            copy_romfs: Some("YES".into()),
            ..ipa::Inputs::default()
        };
        let st = ipa::State {
            ios: w.join("ios"),
            runner: Runner::from_parent().unwrap(),
            g: Game::open(&w.join("profile/Game")).unwrap(),
            pkg: "v0".into(),
            romfs: s("romfs"),
            msl_ios: s("shader/ios"),
            msldir: ipa::msl_dir_name(api, "1"),
            aoc: String::new(),
            pso: String::new(),
            out_ipa: w.join("Game.ipa"),
            o: w.join("o"),
            dd: w.join("dd"),
            app_version: "1.0".into(),
            progs: Vec::new(),
            chain: String::new(),
            framework: false,
            fw_only: false,
            fw_name: String::new(),
            icon: String::new(),
        };
        let (maps, _, o) = ipapack::plan(&st, &inp, &w.join("app"), &mut |_| {}).unwrap();
        assert!(maps.contains(&(
            format!("Payload/DmIOS.app/msl/{dir}"),
            w.join(format!("shader/ios/{dir}"))
        )));
        let out = w.join("Game.ipa");
        nxb_pack::ipa::pack(&out, &maps, &o).unwrap();
        let mut got = zip_names(&out);
        got.retain(|n| n.contains("/msl/") && !n.ends_with('/'));
        got.sort();
        assert_eq!(
            got,
            [
                format!("Payload/DmIOS.app/msl/{dir}/{dir}_shaders.metallib"),
                format!("Payload/DmIOS.app/msl/{dir}/{dir}_shaders.tsv"),
            ]
        );
        let _ = std::fs::remove_dir_all(w);
    }
}
