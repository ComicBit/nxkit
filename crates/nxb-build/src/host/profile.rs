//! Game profiles as `dm/games/game.py` reads them: `game.tsv` rows, `get` / `words` / `int`, `builds`,
//! `build_modules`, and the profile search (`dm/games`, then `DM_GAMES_DIR`.`DM_GAMES_FIRST` the other way round).

use super::pyfmt::{int0, splitlines, strip, words};
use std::path::{Path, PathBuf};

/// One version's build row: (module or `display`, value), in `dict` order.
pub type BuildRow = Vec<(String, String)>;

/// One game profile.
#[derive(Clone, Debug)]
pub struct Game {
    /// The profile folder (resolved).
    pub dir: PathBuf,
    /// Its name (the base game's title ID).
    pub id: String,
    rows: Vec<(String, Vec<String>)>,
}

/// `dict[k] = v`: a new key goes last, an old key keeps its place.
pub fn dict_set<V>(d: &mut Vec<(String, V)>, k: &str, v: V) {
    match d.iter_mut().find(|(x, _)| x == k) {
        Some(e) => e.1 = v,
        None => d.push((k.to_string(), v)),
    }
}

impl Game {
    /// Read `<dir>/game.tsv`.
    pub fn open(dir: &Path) -> Result<Game, String> {
        let dir = std::fs::canonicalize(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let id = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let path = dir.join("game.tsv");
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut rows = Vec::new();
        for line in splitlines(&text) {
            if strip(line).is_empty()
                || line
                    .trim_start_matches(super::pyfmt::is_space)
                    .starts_with('#')
            {
                continue;
            }
            let mut c = line.split('\t');
            let key = strip(c.next().unwrap_or("")).to_string();
            rows.push((key, c.map(|v| strip(v).to_string()).collect()));
        }
        Ok(Game { dir, id, rows })
    }

    /// Every row of this key.
    pub fn values(&self, key: &str) -> impl Iterator<Item = &[String]> {
        self.rows
            .iter()
            .filter(move |(k, _)| k == key)
            .map(|(_, v)| v.as_slice())
    }

    /// The first row's values joined by tabs, a missing key is an error.
    pub fn get(&self, key: &str) -> Result<String, String> {
        self.values(key)
            .next()
            .map(|v| v.join("\t"))
            .ok_or_else(|| format!("{} has no key {key}", self.dir.join("game.tsv").display()))
    }

    /// The first row's values joined by tabs, else `default`.
    pub fn get_or(&self, key: &str, default: &str) -> String {
        self.get(key).unwrap_or_else(|_| default.to_string())
    }

    /// `g.words(key)`.
    pub fn words(&self, key: &str) -> Result<Vec<String>, String> {
        Ok(words(&self.get(key)?)
            .into_iter()
            .map(str::to_string)
            .collect())
    }

    /// `g.words(key, default)`.
    pub fn words_or(&self, key: &str, default: &str) -> Vec<String> {
        words(&self.get_or(key, default))
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    /// `g.int(key)`: `int(value, 0)`.
    pub fn int(&self, key: &str) -> Result<i128, String> {
        int0(&self.get(key)?)
    }

    /// `g.table(name)`.
    pub fn table(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// `g.builds()`: {version: {"display": ..., module: build ID}}, in `dict` order.
    pub fn builds(&self) -> Result<Vec<(String, BuildRow)>, String> {
        let mut out: Vec<(String, BuildRow)> = Vec::new();
        for v in self.values("build") {
            let [ver, disp, mods @ ..] = v else {
                return Err(format!(
                    "{}: a build row needs a version and a display version",
                    self.id
                ));
            };
            let mut row: BuildRow = vec![("display".into(), disp.clone())];
            for m in mods {
                let (k, val) = m
                    .split_once('=')
                    .ok_or_else(|| format!("{}: build row module without '=': {m}", self.id))?;
                dict_set(&mut row, k, val.to_string());
            }
            dict_set(&mut out, ver, row);
        }
        Ok(out)
    }

    /// `g.build_modules()`: {main build ID: [modules in the order of `modules`]}.
    pub fn build_modules(&self) -> Result<Vec<(String, Vec<String>)>, String> {
        let order = self.words("modules")?;
        let mut out = Vec::new();
        for (_, b) in self.builds()? {
            if let Some((_, main)) = b.iter().find(|(k, _)| k == "main") {
                let ms = order
                    .iter()
                    .filter(|m| b.iter().any(|(k, _)| k == *m))
                    .cloned()
                    .collect();
                dict_set(&mut out, main, ms);
            }
        }
        Ok(out)
    }
}

/// Where profiles are looked up: the engine's `dm/games`, and `DM_GAMES_DIR` (`:`-separated) after it, or before it
/// with `DM_GAMES_FIRST`.
#[derive(Clone, Debug)]
pub struct Profiles {
    /// The engine's `dm/games`.
    pub here: PathBuf,
    /// `DM_GAMES_DIR`.
    pub extra: Vec<PathBuf>,
    /// `DM_GAMES_FIRST` set (non-empty).
    pub extra_first: bool,
}

impl Profiles {
    /// The engine's profiles with `DM_GAMES_DIR` / `DM_GAMES_FIRST` from `var`.
    pub fn new(engine: &Path, var: &dyn Fn(&str) -> Option<String>) -> Self {
        Profiles {
            here: engine.join("dm/games"),
            extra: var("DM_GAMES_DIR")
                .unwrap_or_default()
                .split(':')
                .filter(|p| !p.is_empty())
                .map(PathBuf::from)
                .collect(),
            extra_first: var("DM_GAMES_FIRST").is_some_and(|v| !v.is_empty()),
        }
    }

    fn search(&self) -> Vec<&Path> {
        let here = std::iter::once(self.here.as_path());
        let extra = self.extra.iter().map(PathBuf::as_path);
        if self.extra_first {
            extra.chain(here).collect()
        } else {
            here.chain(extra).collect()
        }
    }

    /// `game.load(name)`: a path is taken as is, else the first search folder holding `<name>/game.tsv`.
    pub fn load(&self, name: &str) -> Result<Game, String> {
        let d = if name.contains('/') {
            PathBuf::from(name)
        } else {
            self.search()
                .into_iter()
                .map(|d| d.join(name))
                .find(|d| d.join("game.tsv").exists())
                .unwrap_or_else(|| self.here.join(name))
        };
        if !d.join("game.tsv").exists() {
            return Err(format!(
                "[RED] no game profile {}",
                d.join("game.tsv").display()
            ));
        }
        Game::open(&d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn profile(tsv: &str) -> (PathBuf, Game) {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("nxb-profile-{}-{n}", std::process::id()));
        let d = root.join("dm/games/0100000000001000");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("game.tsv"), tsv).unwrap();
        let g = Game::open(&d).unwrap();
        (root, g)
    }

    #[test]
    fn rows_as_game_py() {
        let (root, g) = profile(
            "# c\n  # c2\nname\tA Game \nmodules\tmain sdk\nbuild\tv0\t1.0.0\tmain=aa\tsdk=bb\nbuild\tv1\t1.0.1\tmain=cc\nbuild\tv0\t1.0.2\tmain=dd\nempty\n",
        );
        assert_eq!(g.id, "0100000000001000");
        assert_eq!(g.get("name").unwrap(), "A Game");
        assert_eq!(g.get("empty").unwrap(), "");
        assert!(g.get("nope").is_err());
        assert_eq!(g.get_or("nope", "d"), "d");
        let b = g.builds().unwrap();
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].0, "v0");
        assert_eq!(b[0].1[0], ("display".into(), "1.0.2".into()));
        let bm = g.build_modules().unwrap();
        assert_eq!(
            bm,
            vec![
                ("dd".to_string(), vec!["main".to_string()]),
                ("cc".to_string(), vec!["main".to_string()])
            ]
        );
        let p = Profiles::new(&root, &|_| None);
        assert_eq!(p.load("0100000000001000").unwrap().id, g.id);
        assert!(p.load("0100000000002000").is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
