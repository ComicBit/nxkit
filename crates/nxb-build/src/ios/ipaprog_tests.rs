//! The oracle of the multi-program flow: `build_ipa.sh --prepare-xcode` with a `PROGRAMS_TSV` of a title with three
//! programs (synthetic rewriter outputs, RomFS folders and shader folders, the profiles' own tables)
//! against [`run`](super::super::ipa::run).

use super::super::ipa::{Inputs, Target, run};
use crate::host::Runner;
use crate::host::profile::Profiles;
use crate::ios::oracle;
use std::path::Path;

const IDS: [&str; 3] = ["010049900F546000", "010049900F546001", "010049900F546002"];
const GFX: [&str; 3] = ["nvn", "nvn", "vulkan"];

fn write(p: &Path, t: &[u8]) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, t).unwrap();
}

/// The synthetic inputs of the programs under `d`: the profile overlay `d/games`, and per program `d/p<n>/{patched,
/// RomFS,msl,control.nacp}`; the `PROGRAMS_TSV` text.
fn fixture(e: &Path, d: &Path, v: &str) -> String {
    let games = d.join("games");
    let mut rows = String::from("# program\tprofile\tpatched\tRomFS\tshaders\tgraphics\n");
    let prof = Profiles::new(e, &|_| None);
    for (n, (id, gfx)) in IDS.iter().zip(GFX).enumerate() {
        let src = e.join("dm/games").join(id);
        for f in std::fs::read_dir(&src).unwrap().flatten() {
            let t = games.join(id).join(f.file_name());
            std::fs::create_dir_all(t.parent().unwrap()).unwrap();
            std::fs::copy(f.path(), t).unwrap();
        }
        let g = prof.load(id).unwrap();
        let p = d.join(format!("p{n}"));
        // the rewriter's outputs: every module of the profile, and the (empty) gate, cut and observation tables
        for m in g.get("modules").unwrap().split_whitespace() {
            let hdr = "# PATCH_OFF\t0xc000\n# KEY_PAGE_OFF\t0x8000\n# KEY_PAGE_SIZE\t0x4000\n# TEXT_END\t0x5010\n# BUILD_ID\tzz\n";
            write(&p.join(format!("patched/{m}.fixups.tsv")), hdr.as_bytes());
            write(
                &p.join(format!("patched/{m}.patched.bin")),
                &vec![n as u8 + 1; 0xe123],
            );
            for k in ["gates", "cuts", "obs"] {
                write(&p.join(format!("patched/{m}.{k}.tsv")), b"");
            }
        }
        write(&p.join("RomFS").join(g.get("romfs_marker").unwrap()), b"x");
        let mut nacp = vec![0u8; 0x4000];
        nacp[..4].copy_from_slice(b"Prog");
        nacp[0x3060..0x3063].copy_from_slice(b"1.0");
        write(&p.join("control.nacp"), &nacp);
        if gfx == "nvn" {
            write(&p.join(format!("msl/b-{v}/0000_b.metallib")), b"lib");
            // the overlay's own profile folder is the one every tool reads (DM_GAMES_DIR first)
            let og = Profiles::new(e, &|k| match k {
                "DM_GAMES_DIR" => Some(games.to_string_lossy().into_owned()),
                "DM_GAMES_FIRST" => Some("1".into()),
                _ => None,
            })
            .load(id)
            .unwrap();
            let inp = nxb_shader::chain::inputs(&og.dir).unwrap();
            write(
                &p.join(format!("msl/b-{v}/inputs.tsv")),
                format!("# stamp\ngame\t{id}\ninputs\t{inp}\n").as_bytes(),
            );
            write(
                &games.join(id).join("c1_full.tsv"),
                format!("# chain b-{v} inputs {inp} \n").as_bytes(),
            );
        } else {
            write(&p.join("msl/vk/vk_shaders.metallib"), b"lib");
        }
        rows.push_str(&format!(
            "{n}\t{id}\t{}\t{}\t{}\t{gfx}\n",
            p.join("patched").display(),
            p.join("RomFS").display(),
            p.join("msl").display()
        ));
    }
    rows
}

/// The reference script builds the host in the engine's folder: the oracle runs take turns.
static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct Setup {
    _turn: std::sync::MutexGuard<'static, ()>,
    /// The program the one-program runs use (`GAME`).
    idx: usize,
    e: std::path::PathBuf,
    d: std::path::PathBuf,
    tsv: std::path::PathBuf,
    games: String,
    font_py: String,
    user: String,
    r: Runner,
}

fn setup(e: std::path::PathBuf, idx: usize) -> Setup {
    let turn = TURN
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let d = oracle::scratch("progs");
    let v = crate::ios::script::chain_version(&e.join("dm/gfx/nvn/dmsh_chain.h"));
    let rows = fixture(&e, &d, &v);
    let tsv = d.join("programs.tsv");
    std::fs::write(&tsv, rows).unwrap();
    Setup {
        _turn: turn,
        idx,
        games: d.join("games").to_string_lossy().into_owned(),
        font_py: std::env::var("NXB_ORACLE_PYTHON").unwrap_or_else(|_| "python3".into()),
        user: std::env::var("USER").unwrap_or_else(|_| "tester".into()),
        r: Runner::from_parent().unwrap(),
        e,
        d,
        tsv,
    }
}

impl Setup {
    /// `build_ipa.sh <arg>` with the script's environment, work folder `w`.
    fn script(
        &self,
        arg: &std::ffi::OsStr,
        w: &Path,
        programs: bool,
        extra: &[(&str, &str)],
    ) -> std::process::Output {
        let p0 = self.d.join(format!("p{}", self.idx));
        let path = self
            .r
            .env()
            .get("PATH")
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let mut c = std::process::Command::new("bash");
        c.arg(self.e.join("dm/ios/tools/build_ipa.sh")).arg(arg);
        if programs {
            c.env("PROGRAMS_TSV", &self.tsv);
        }
        c.envs(extra.iter().copied())
            .env("GAME", IDS[self.idx])
            .env("W", w)
            .env("PATCHED", p0.join("patched"))
            .env("ROMFS", p0.join("RomFS"))
            .env("MSL_IOS", p0.join("msl"))
            .env("GFXAPI", GFX[self.idx])
            .env("DM_GAMES_DIR", &self.games)
            .env("DM_GAMES_FIRST", "1")
            .env("FONT_PY", &self.font_py)
            .env("PATH", &path)
            .env("USER", &self.user)
            .output()
            .unwrap()
    }

    fn inputs(&self, w: &Path, programs: bool) -> Inputs {
        let p0 = self.d.join(format!("p{}", self.idx));
        let s = |p: std::path::PathBuf| Some(p.to_string_lossy().into_owned());
        Inputs {
            engine: self.e.clone(),
            game: IDS[self.idx].into(),
            w: w.to_string_lossy().into_owned(),
            patched: s(p0.join("patched")),
            romfs: s(p0.join("RomFS")),
            msl_ios: s(p0.join("msl")),
            gfxapi: Some(GFX[self.idx].into()),
            programs_tsv: programs.then(|| self.tsv.to_string_lossy().into_owned()),
            games_dir: Some(self.games.clone()),
            games_first: Some("1".into()),
            font_py: Some(self.font_py.clone()),
            user: self.user.clone(),
            ..Inputs::default()
        }
    }
}

fn norm(s: &str, w: &Path) -> String {
    let real = std::fs::canonicalize(w).unwrap();
    s.replace(real.to_str().unwrap(), "<w>")
        .replace(w.to_str().unwrap(), "<w>")
}

/// The step lines of the script's output without the time stamps.
fn steps(out: &str, w: &Path) -> Vec<String> {
    out.lines()
        .filter_map(|l| l.get(11..).filter(|_| l.starts_with('[')))
        .map(|l| norm(l, w))
        .collect()
}

/// Oracle: `NXB_ORACLE_ENGINE` (and `NXB_ORACLE_PYTHON`): both sides prepare the project of the three programs, the
/// step lines and every file of the two project folders equal (the font's time aside).
#[test]
fn prepare_of_three_programs_equals_the_reference_script() {
    check_prepare(true, false);
}

/// As above for one program with `DM_FRAMEWORK=1` (route C: `Gen/framework`, the framework project template).
#[test]
fn prepare_of_a_framework_equals_the_reference_script() {
    check_prepare(false, true);
}

fn check_prepare(programs: bool, fw: bool) {
    let Some(e) = oracle::engine() else { return };
    let t = setup(e, if programs { 0 } else { 1 });
    let (pw, rw) = (t.d.join("py/w"), t.d.join("rs/w"));
    let o = t.script(
        "--prepare-xcode".as_ref(),
        &pw,
        programs,
        if fw { &[("DM_FRAMEWORK", "1")] } else { &[] },
    );
    let script_out = String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(
        o.status.success(),
        "{script_out}{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let mut lines = Vec::new();
    let mut inp = t.inputs(&rw, programs);
    inp.framework = fw.then(|| "1".to_string());
    run(&t.r, Target::Prepare, &inp, &mut |l| {
        lines.push(l.to_string())
    })
    .unwrap();
    let want = steps(&script_out, &pw);
    let got: Vec<String> = lines.iter().map(|l| norm(l, &rw)).collect();
    assert_eq!(got, want);
    let (a, b) = (
        oracle::tree(&pw.join("xcode")),
        oracle::tree(&rw.join("xcode")),
    );
    let names = |v: &[(String, Vec<u8>)]| v.iter().map(|x| x.0.clone()).collect::<Vec<_>>();
    eprintln!("oracle: {} step lines, {} files", want.len(), a.len());
    assert_eq!(names(&b), names(&a));
    for ((n, x), (_, y)) in b.iter().zip(&a) {
        if n.ends_with("nintendo_ext.ttf") {
            // the font carries its creation time, and `oracle::tree` reads text lossily: the bytes (and their length) differ by run
            let _ = (x, y);
            continue;
        }
        let (x, y) = (
            norm(&String::from_utf8_lossy(x), &rw),
            norm(&String::from_utf8_lossy(y), &pw),
        );
        assert!(x == y, "{n} differs");
    }
    let _ = std::fs::remove_dir_all(&t.d);
}

/// Oracle (two `xcodebuild` runs), with `NXB_ORACLE_IOS_IPA=1`: the IPA of the three programs, the script's and ours:
/// the same entries (name, method, sizes, CRC, attributes), the executable equal in the code before `__LINKEDIT`.
#[test]
fn ipa_of_three_programs_equals_the_reference_script() {
    check_ipa(true, false);
}

/// The IPA of one program with `DM_FRAMEWORK=1`: the framework signed first, then the executable.
#[test]
fn ipa_with_a_framework_equals_the_reference_script() {
    check_ipa(false, true);
}

fn check_ipa(programs: bool, fw: bool) {
    let (Some(e), Ok(_)) = (oracle::engine(), std::env::var("NXB_ORACLE_IOS_IPA")) else {
        return;
    };
    let t = setup(e, if programs { 0 } else { 1 });
    let (pw, rw) = (t.d.join("py/w"), t.d.join("rs/w"));
    let (pi, ri) = (t.d.join("py.ipa"), t.d.join("rs.ipa"));
    let o = t.script(
        pi.as_os_str(),
        &pw,
        programs,
        if fw { &[("DM_FRAMEWORK", "1")] } else { &[] },
    );
    let script_out = String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(
        o.status.success(),
        "{script_out}{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let mut lines = Vec::new();
    let mut inp = t.inputs(&rw, programs);
    inp.framework = fw.then(|| "1".to_string());
    run(&t.r, Target::Ipa(Some(&ri)), &inp, &mut |l| {
        lines.push(l.to_string())
    })
    .unwrap();
    let loose = |s: String| -> String {
        s.lines()
            .map(|l| {
                let f: Vec<&str> = l.split(' ').collect();
                if f[0].ends_with("/DmIOS")
                    || f[0].ends_with("/DmGame")
                    || f[0].ends_with("nintendo_ext.ttf")
                {
                    format!("{} {} {}", f[0], f[1], f[3])
                } else {
                    l.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let (a, b) = (
        crate::ios::ipa::tests::entries(&pi),
        crate::ios::ipa::tests::entries(&ri),
    );
    eprintln!("oracle: {} entries", a.lines().count());
    assert_eq!(loose(b), loose(a));
    let step = |s: &str, w: &Path| {
        s.replace(w.to_str().unwrap(), "<w>")
            .replace(&*pi.to_string_lossy(), "<ipa>")
            .replace(&*ri.to_string_lossy(), "<ipa>")
    };
    let want: Vec<String> = script_out
        .lines()
        .filter_map(|l| l.get(11..).filter(|_| l.starts_with('[')))
        .map(|l| step(l, &pw))
        .collect();
    let got: Vec<String> = lines
        .iter()
        .filter(|l| !l.starts_with("pack: "))
        .map(|l| step(l, &rw))
        .collect();
    eprintln!(
        "ipa sizes: script {} ours {}",
        std::fs::metadata(&pi).unwrap().len(),
        std::fs::metadata(&ri).unwrap().len()
    );
    for (k, (g, w)) in got.iter().zip(&want).enumerate() {
        // `du -h` of the new IPA counts allocated blocks: the size shown can differ by a unit with the same bytes
        let cut = |x: &str| x.split(" (").next().unwrap_or("").to_string();
        assert_eq!(cut(g), cut(w), "step line {k}");
    }
    assert_eq!(got.len(), want.len());
    let _ = std::fs::remove_dir_all(&t.d);
}

/// The files of a built framework folder without the binary's debug paths: names, sizes of the text files and the
/// executable's code before `__LINKEDIT`.
fn framework_tree(d: &Path, name: &str) -> Vec<(String, Vec<u8>)> {
    oracle::tree(d)
        .into_iter()
        .map(|(n, b)| {
            if n.ends_with(&format!("/{name}")) || n == name {
                (n, crate::host::machost::code_bytes(&b).unwrap_or_default())
            } else if n.ends_with("nintendo_ext.ttf") {
                (n, vec![b.len() as u8])
            } else {
                (n, b)
            }
        })
        .collect()
}

/// Oracle (two `xcodebuild` runs), with `NXB_ORACLE_IOS_IPA=1`: `DM_FRAMEWORK_ONLY=<folder>`, the shell's framework
/// of one game, built by the script and by us: the same files (the executable by its code, the framework's own
/// signature files included).
#[test]
fn framework_only_equals_the_reference_script() {
    let (Some(e), Ok(_)) = (oracle::engine(), std::env::var("NXB_ORACLE_IOS_IPA")) else {
        return;
    };
    let t = setup(e, 1);
    let (pw, rw) = (t.d.join("py/w"), t.d.join("rs/w"));
    let (pf, rf) = (t.d.join("py-fw"), t.d.join("rs-fw"));
    let name = format!("DmGame{}", IDS[1]);
    let o = t.script(
        "".as_ref(),
        &pw,
        false,
        &[("DM_FRAMEWORK_ONLY", pf.to_str().unwrap())],
    );
    let script_out = String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(
        o.status.success(),
        "{script_out}{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let mut inp = t.inputs(&rw, false);
    inp.framework_only = Some(rf.to_string_lossy().into_owned());
    let mut lines = Vec::new();
    run(&t.r, Target::Ipa(None), &inp, &mut |l| {
        lines.push(l.to_string())
    })
    .unwrap();
    let (a, b) = (
        framework_tree(&pf.join(format!("{name}.framework")), &name),
        framework_tree(&rf.join(format!("{name}.framework")), &name),
    );
    eprintln!("oracle: {} framework files", a.len());
    let names = |v: &[(String, Vec<u8>)]| v.iter().map(|x| x.0.clone()).collect::<Vec<_>>();
    assert_eq!(names(&b), names(&a));
    for ((n, x), (_, y)) in b.iter().zip(&a) {
        assert!(x == y, "{n} differs");
    }
    let _ = std::fs::remove_dir_all(&t.d);
}
