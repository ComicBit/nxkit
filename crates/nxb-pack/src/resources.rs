//! Resource rules, the bundle plan and the `CodeResources` seal.
//!
//! `codesign` seals every file of a macOS bundle's `Contents/` directory in
//! `_CodeSignature/CodeResources`, a CoreFoundation XML plist with four keys:
//!
//! - `files`: the legacy (v1) seal, SHA-1 only, for files matched by the v1 `rules`
//!   (in practice `Resources/` and `version.plist`). Symbolic links are left out.
//! - `files2`: the v2 seal per `rules2`: `hash` (SHA-1, only when the main executable's
//!   signature uses SHA-1) and `hash2` (SHA-256).`optional` for `.lproj` content.
//!   `symlink` for links.`cdhash` + `requirement` for nested code.
//! - `rules`, `rules2`: the default rule sets, written verbatim.
//!
//! The rules are regular expressions in `codesign`, the default sets are fixed, so each is
//! implemented here as a plain matcher with the same semantics (see the unit tests), and the
//! rule with the highest weight wins.

use crate::error::{Error, Result};
use crate::hash::HashType;
use crate::plist::{self, Value};

/// What a resource rule says about a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Sealed as a resource.`optional` for localisations.
    Include {
        /// The entry carries `optional = true`.
        optional: bool,
    },
    /// Left out of the seal.
    Omit,
    /// Nested code: sealed by its cdhash.
    Nested,
}

/// Which default rule set a bundle uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleSet {
    /// Deep bundles (`Contents/`) and versioned frameworks (`Versions/A/`): resources under
    /// `Resources/`, nested code in `MacOS/`, `Frameworks/`, `Helpers/`, ...
    MacOs,
    /// Shallow bundles (iOS, tvOS, ...): everything at the bundle root, no nested-code rule
    /// (nested frameworks are sealed file by file after they are signed).
    Shallow,
}

/// The v1 (`rules`) decision for `path` under `set`.`None` = not sealed.
pub fn rule_v1_in(set: RuleSet, path: &str) -> Option<Action> {
    match set {
        RuleSet::MacOs => rule_v1(path),
        RuleSet::Shallow => {
            let mut best = (1u32, Action::Include { optional: false }); // `^.*`, `^version.plist$`
            let mut consider = |w: u32, a: Action| {
                if w > best.0 {
                    best = (w, a);
                }
            };
            if path.contains(".lproj/") {
                consider(1000, Action::Include { optional: true });
            }
            if is_locversion(path) {
                consider(1100, Action::Omit);
            }
            if path.starts_with("Base.lproj/") {
                consider(1010, Action::Include { optional: false });
            }
            Some(best.1)
        }
    }
}

/// The v2 (`rules2`) decision for `path` under `set`.
pub fn rule_v2_in(set: RuleSet, path: &str) -> Action {
    match set {
        RuleSet::MacOs => rule_v2(path),
        RuleSet::Shallow => {
            let mut best = (1u32, Action::Include { optional: false }); // `^.*`
            let mut consider = |w: u32, a: Action| {
                if w > best.0 {
                    best = (w, a);
                }
            };
            if path.contains(".dSYM/") || path.ends_with(".dSYM") {
                consider(11, Action::Include { optional: false });
            }
            if path == ".DS_Store" || path.ends_with("/.DS_Store") {
                consider(2000, Action::Omit);
            }
            if path.contains(".lproj/") {
                consider(1000, Action::Include { optional: true });
            }
            if is_locversion(path) {
                consider(1100, Action::Omit);
            }
            if path.starts_with("Base.lproj/") {
                consider(1010, Action::Include { optional: false });
            }
            if path == "Info.plist" || path == "PkgInfo" {
                consider(20, Action::Omit);
            }
            if path == "embedded.provisionprofile" || path == "version.plist" {
                consider(20, Action::Include { optional: false });
            }
            best.1
        }
    }
}

/// The v1 (`rules`) decision for `path` (relative to `Contents/`).`None` = not sealed.
pub fn rule_v1(path: &str) -> Option<Action> {
    let mut best: Option<(u32, Action)> = None;
    let mut consider = |w: u32, a: Action| {
        if best.is_none_or(|(bw, _)| w > bw) {
            best = Some((w, a));
        }
    };
    if let Some(rest) = path.strip_prefix("Resources/") {
        consider(1, Action::Include { optional: false });
        if rest.contains(".lproj/") {
            consider(1000, Action::Include { optional: true });
        }
        if is_locversion(rest) {
            consider(1100, Action::Omit);
        }
        if rest.starts_with("Base.lproj/") {
            consider(1010, Action::Include { optional: false });
        }
    }
    // `^version.plist$`: the dot is a wildcard in the default v1 rule.
    let b = path.as_bytes();
    if b.len() == 13 && b.starts_with(b"version") && b.ends_with(b"plist") {
        consider(1, Action::Include { optional: false });
    }
    best.map(|(_, a)| a)
}

/// The v2 (`rules2`) decision for `path` (relative to `Contents/`).
pub fn rule_v2(path: &str) -> Action {
    let mut best = (1u32, Action::Include { optional: false }); // `^.*`
    let mut consider = |w: u32, a: Action| {
        if w > best.0 {
            best = (w, a);
        }
    };
    if path.contains(".dSYM/") || path.ends_with(".dSYM") {
        consider(11, Action::Include { optional: false });
    }
    if path == ".DS_Store" || path.ends_with("/.DS_Store") {
        consider(2000, Action::Omit);
    }
    const NESTED_DIRS: [&str; 10] = [
        "Frameworks/",
        "SharedFrameworks/",
        "PlugIns/",
        "Plug-ins/",
        "XPCServices/",
        "Helpers/",
        "MacOS/",
        "Library/Automator/",
        "Library/Spotlight/",
        "Library/LoginItems/",
    ];
    if NESTED_DIRS.iter().any(|d| path.starts_with(d)) {
        consider(10, Action::Nested);
    }
    if path == "Info.plist" || path == "PkgInfo" {
        consider(20, Action::Omit);
    }
    if let Some(rest) = path.strip_prefix("Resources/") {
        consider(20, Action::Include { optional: false });
        if rest.contains(".lproj/") {
            consider(1000, Action::Include { optional: true });
        }
        if is_locversion(rest) {
            consider(1100, Action::Omit);
        }
        if rest.starts_with("Base.lproj/") {
            consider(1010, Action::Include { optional: false });
        }
    }
    if !path.contains('/') {
        consider(10, Action::Nested);
    }
    if path == "embedded.provisionprofile" || path == "version.plist" {
        consider(20, Action::Include { optional: false });
    }
    best.1
}

/// `.*\.lproj/locversion.plist$` on `rest` (the part after `Resources/` for macOS rules, the
/// whole path for shallow ones, the last dot is a wildcard).
fn is_locversion(rest: &str) -> bool {
    let b = rest.as_bytes();
    if b.len() < 16 {
        return false;
    }
    let (head, tail) = b.split_at(b.len() - 16);
    tail.starts_with(b"locversion") && tail.ends_with(b"plist") && head.ends_with(b".lproj/")
}

/// What one sealed path is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanKind {
    /// A regular file.
    File,
    /// A symbolic link with its target.
    Symlink(String),
    /// A Mach-O file in a nested-code location.
    NestedCode,
    /// A nested bundle directory (`Helpers/X.app`, `Frameworks/X.framework`), sealed by the
    /// cdhash of its main executable.
    NestedBundle,
}

/// One path of the bundle plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanEntry {
    /// Path relative to `Contents/`, `/`-separated.
    pub path: String,
    /// What it is.
    pub kind: PlanKind,
    /// File size (0 for links).
    pub size: u64,
}

impl PlanEntry {
    /// Whether the v1 seal under `set` needs this entry's SHA-1.
    pub fn needs_sha1_v1(&self, set: RuleSet) -> bool {
        self.kind == PlanKind::File
            && matches!(rule_v1_in(set, &self.path), Some(Action::Include { .. }))
    }

    /// Whether the v2 seal under `set` hashes this entry's content.
    pub fn hashed_v2(&self, set: RuleSet) -> bool {
        self.kind == PlanKind::File && matches!(rule_v2_in(set, &self.path), Action::Include { .. })
    }
}

/// The paths of a bundle to seal, with the main executable and Info.plist.
#[derive(Clone, Debug, Default)]
pub struct BundlePlan {
    /// The main executable, relative to `Contents/` (excluded from the seal).
    pub main_executable: String,
    /// Every other path to consider, in any order.
    pub entries: Vec<PlanEntry>,
}

/// A sealed item's content digests or cdhashes, as [`seal`] consumes them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SealItem {
    /// A regular file's digests (`sha1` required when it is in the v1 seal or SHA-1 is in use).
    File {
        /// SHA-1 of the content.
        sha1: Option<[u8; 20]>,
        /// SHA-256 of the content.
        sha256: [u8; 32],
    },
    /// A symbolic link and its target.
    Symlink(String),
    /// Nested code: `(hash type, cdhash)` per CodeDirectory, in its superblob order.
    Nested(Vec<(HashType, [u8; 20])>),
}

/// Writes `CodeResources` for `items` (paths relative to `Contents/`, already filtered by
/// [`BundlePlan`] rules is not required: omitted paths are dropped here).
///
/// `digest_types` are the main executable's CodeDirectory types: with SHA-1 among them, each
/// `files2` entry also carries `hash`.
pub fn seal(items: &[(String, SealItem)], digest_types: &[HashType]) -> Result<Vec<u8>> {
    seal_in(RuleSet::MacOs, items, digest_types)
}

/// [`seal`] under the rule set `set`.
pub fn seal_in(
    set: RuleSet,
    items: &[(String, SealItem)],
    digest_types: &[HashType],
) -> Result<Vec<u8>> {
    let with_sha1 = digest_types.contains(&HashType::Sha1);
    let mut files: Vec<(String, Value)> = Vec::new();
    let mut files2: Vec<(String, Value)> = Vec::new();
    for (path, item) in items {
        if let (SealItem::File { sha1, .. }, Some(Action::Include { optional })) =
            (item, rule_v1_in(set, path))
        {
            let h = sha1.ok_or_else(|| Error::Format(format!("missing SHA-1 for {path}")))?;
            let v = if optional {
                Value::Dict(vec![
                    ("hash".into(), Value::Data(h.to_vec())),
                    ("optional".into(), Value::Bool(true)),
                ])
            } else {
                Value::Data(h.to_vec())
            };
            files.push((path.clone(), v));
        }
        let action = rule_v2_in(set, path);
        let entry = match (action, item) {
            (Action::Omit, _) => continue,
            (_, SealItem::Symlink(target)) => {
                let mut d = vec![("symlink".into(), Value::String(target.clone()))];
                if let Action::Include { optional: true } = action {
                    d.push(("optional".into(), Value::Bool(true)));
                }
                Value::Dict(d)
            }
            (Action::Nested, SealItem::Nested(cds)) => {
                let best = cds
                    .iter()
                    .max_by_key(|(t, _)| *t)
                    .ok_or_else(|| Error::Format(format!("no cdhash for {path}")))?;
                let req = cds
                    .iter()
                    .map(|(_, h)| format!("cdhash H\"{}\"", crate::hash::hex(h)))
                    .collect::<Vec<_>>()
                    .join(" or ");
                Value::Dict(vec![
                    ("cdhash".into(), Value::Data(best.1.to_vec())),
                    ("requirement".into(), Value::String(req)),
                ])
            }
            (Action::Nested, SealItem::File { .. }) => {
                return Err(Error::Refused(format!(
                    "code object is not signed at all: {path}"
                )));
            }
            (Action::Include { optional }, SealItem::File { sha1, sha256 }) => {
                let mut d = Vec::new();
                if with_sha1 {
                    let h =
                        sha1.ok_or_else(|| Error::Format(format!("missing SHA-1 for {path}")))?;
                    d.push(("hash".into(), Value::Data(h.to_vec())));
                }
                d.push(("hash2".into(), Value::Data(sha256.to_vec())));
                if optional {
                    d.push(("optional".into(), Value::Bool(true)));
                }
                Value::Dict(d)
            }
            (Action::Include { .. }, SealItem::Nested(_)) => {
                return Err(Error::Unsupported(format!(
                    "nested code outside a nested location: {path}"
                )));
            }
        };
        files2.push((path.clone(), entry));
    }
    let root = Value::Dict(vec![
        ("files".into(), Value::Dict(files)),
        ("files2".into(), Value::Dict(files2)),
        (
            "rules".into(),
            if set == RuleSet::MacOs {
                default_rules_v1()
            } else {
                shallow_rules_v1()
            },
        ),
        (
            "rules2".into(),
            if set == RuleSet::MacOs {
                default_rules_v2()
            } else {
                shallow_rules_v2()
            },
        ),
    ]);
    Ok(plist::to_cf_xml(&root)?.into_bytes())
}

fn rule_dict(entries: &[(&str, Value)]) -> Value {
    Value::Dict(
        entries
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect(),
    )
}

fn weighted(w: f64, extra: &[(&str, Value)]) -> Value {
    let mut e: Vec<(&str, Value)> = extra.to_vec();
    e.push(("weight", Value::Real(w)));
    rule_dict(&e)
}

/// The default v1 rule set (`rules`), as `codesign` writes it.
pub fn default_rules_v1() -> Value {
    let t = Value::Bool(true);
    rule_dict(&[
        ("^Resources/", t.clone()),
        (
            "^Resources/.*\\.lproj/",
            weighted(1000.0, &[("optional", t.clone())]),
        ),
        (
            "^Resources/.*\\.lproj/locversion.plist$",
            weighted(1100.0, &[("omit", t.clone())]),
        ),
        ("^Resources/Base\\.lproj/", weighted(1010.0, &[])),
        ("^version.plist$", t),
    ])
}

/// The shallow-bundle v1 rule set (`rules`), as `codesign` writes it.
pub fn shallow_rules_v1() -> Value {
    let t = Value::Bool(true);
    rule_dict(&[
        ("^.*", t.clone()),
        ("^.*\\.lproj/", weighted(1000.0, &[("optional", t.clone())])),
        (
            "^.*\\.lproj/locversion.plist$",
            weighted(1100.0, &[("omit", t.clone())]),
        ),
        ("^Base\\.lproj/", weighted(1010.0, &[])),
        ("^version.plist$", t),
    ])
}

/// The shallow-bundle v2 rule set (`rules2`), as `codesign` writes it.
pub fn shallow_rules_v2() -> Value {
    let t = Value::Bool(true);
    rule_dict(&[
        (".*\\.dSYM($|/)", weighted(11.0, &[])),
        (
            "^(.*/)?\\.DS_Store$",
            weighted(2000.0, &[("omit", t.clone())]),
        ),
        ("^.*", t.clone()),
        ("^.*\\.lproj/", weighted(1000.0, &[("optional", t.clone())])),
        (
            "^.*\\.lproj/locversion.plist$",
            weighted(1100.0, &[("omit", t.clone())]),
        ),
        ("^Base\\.lproj/", weighted(1010.0, &[])),
        ("^Info\\.plist$", weighted(20.0, &[("omit", t.clone())])),
        ("^PkgInfo$", weighted(20.0, &[("omit", t.clone())])),
        ("^embedded\\.provisionprofile$", weighted(20.0, &[])),
        ("^version\\.plist$", weighted(20.0, &[])),
    ])
}

/// The default v2 rule set (`rules2`), as `codesign` writes it.
pub fn default_rules_v2() -> Value {
    let t = Value::Bool(true);
    rule_dict(&[
        (".*\\.dSYM($|/)", weighted(11.0, &[])),
        (
            "^(.*/)?\\.DS_Store$",
            weighted(2000.0, &[("omit", t.clone())]),
        ),
        (
            "^(Frameworks|SharedFrameworks|PlugIns|Plug-ins|XPCServices|Helpers|MacOS|Library/(Automator|Spotlight|LoginItems))/",
            weighted(10.0, &[("nested", t.clone())]),
        ),
        ("^.*", t.clone()),
        ("^Info\\.plist$", weighted(20.0, &[("omit", t.clone())])),
        ("^PkgInfo$", weighted(20.0, &[("omit", t.clone())])),
        ("^Resources/", weighted(20.0, &[])),
        (
            "^Resources/.*\\.lproj/",
            weighted(1000.0, &[("optional", t.clone())]),
        ),
        (
            "^Resources/.*\\.lproj/locversion.plist$",
            weighted(1100.0, &[("omit", t.clone())]),
        ),
        ("^Resources/Base\\.lproj/", weighted(1010.0, &[])),
        ("^[^/]+$", weighted(10.0, &[("nested", t.clone())])),
        ("^embedded\\.provisionprofile$", weighted(20.0, &[])),
        ("^version\\.plist$", weighted(20.0, &[])),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v1_rules() {
        let inc = Some(Action::Include { optional: false });
        let opt = Some(Action::Include { optional: true });
        assert_eq!(rule_v1("Resources/a.txt"), inc);
        assert_eq!(rule_v1("Resources/.DS_Store"), inc);
        assert_eq!(rule_v1("Resources/en.lproj/x.strings"), opt);
        assert_eq!(rule_v1("Resources/a/b.lproj/x"), opt);
        assert_eq!(rule_v1("Resources/Base.lproj/x"), inc);
        assert_eq!(
            rule_v1("Resources/en.lproj/locversion.plist"),
            Some(Action::Omit)
        );
        assert_eq!(
            rule_v1("Resources/en.lproj/locversionXplist"),
            Some(Action::Omit)
        );
        assert_eq!(rule_v1("version.plist"), inc);
        assert_eq!(rule_v1("versionXplist"), inc);
        assert_eq!(rule_v1("MacOS/x"), None);
        assert_eq!(rule_v1("Info.plist"), None);
        assert_eq!(rule_v1("other/x"), None);
    }

    #[test]
    fn v2_rules() {
        let inc = Action::Include { optional: false };
        assert_eq!(rule_v2("Resources/a.txt"), inc);
        assert_eq!(rule_v2("Resources/.DS_Store"), Action::Omit);
        assert_eq!(rule_v2(".DS_Store"), Action::Omit);
        assert_eq!(
            rule_v2("Resources/en.lproj/x"),
            Action::Include { optional: true }
        );
        assert_eq!(rule_v2("Resources/en.lproj/locversion.plist"), Action::Omit);
        assert_eq!(rule_v2("Resources/Base.lproj/x"), inc);
        assert_eq!(rule_v2("MacOS/dmhost"), Action::Nested);
        assert_eq!(rule_v2("MacOS/x.dSYM/y"), inc);
        assert_eq!(rule_v2("Library/LoginItems/x"), Action::Nested);
        assert_eq!(rule_v2("Library/Other/x"), inc);
        assert_eq!(rule_v2("Info.plist"), Action::Omit);
        assert_eq!(rule_v2("PkgInfo"), Action::Omit);
        assert_eq!(rule_v2("top"), Action::Nested);
        assert_eq!(rule_v2("version.plist"), inc);
        assert_eq!(rule_v2("embedded.provisionprofile"), inc);
        assert_eq!(rule_v2("other/dir/x"), inc);
    }

    #[test]
    fn seal_entries() {
        let f = |b: u8| SealItem::File {
            sha1: Some([b; 20]),
            sha256: [b; 32],
        };
        let items = vec![
            ("Resources/b".to_string(), f(1)),
            ("Resources/en.lproj/l".to_string(), f(2)),
            ("Resources/link".to_string(), SealItem::Symlink("b".into())),
            (
                "MacOS/h".to_string(),
                SealItem::Nested(vec![(HashType::Sha256, [9; 20])]),
            ),
            ("Resources/.DS_Store".to_string(), f(3)),
        ];
        let s = String::from_utf8(seal(&items, &[HashType::Sha256]).unwrap()).unwrap();
        assert!(s.contains(
            "\t<key>files</key>\n\t<dict>\n\t\t<key>Resources/.DS_Store</key>\n\t\t<data>\n"
        ));
        assert!(s.contains("<key>symlink</key>\n\t\t\t<string>b</string>"));
        assert!(s.contains("cdhash H\"0909090909090909090909090909090909090909\""));
        assert!(!s.contains("<key>hash</key>\n\t\t\t<data>\n\t\t\tAQEB")); // no SHA-1 in files2
        let with1 =
            String::from_utf8(seal(&items, &[HashType::Sha1, HashType::Sha256]).unwrap()).unwrap();
        assert!(with1.contains("<key>hash</key>\n\t\t\t<data>\n\t\t\tAQEB"));
        let bad = vec![("MacOS/x".to_string(), f(1))];
        assert!(matches!(
            seal(&bad, &[HashType::Sha256]),
            Err(Error::Refused(_))
        ));
    }

    #[test]
    fn shallow_rules() {
        let inc = Action::Include { optional: false };
        let s = RuleSet::Shallow;
        assert_eq!(rule_v1_in(s, "Info.plist"), Some(inc));
        assert_eq!(rule_v2_in(s, "Info.plist"), Action::Omit);
        assert_eq!(rule_v2_in(s, "Frameworks/X.framework/X"), inc);
        assert_eq!(
            rule_v2_in(s, "en.lproj/a"),
            Action::Include { optional: true }
        );
        assert_eq!(rule_v2_in(s, "x/en.lproj/locversion.plist"), Action::Omit);
        assert_eq!(rule_v1_in(s, ".DS_Store"), Some(inc));
        assert_eq!(rule_v2_in(s, "a/.DS_Store"), Action::Omit);
        let text = String::from_utf8(seal_in(s, &[], &[HashType::Sha256]).unwrap()).unwrap();
        // The golden bundle had an Info.plist (sealed in v1): compare from the rules on.
        let golden = include_str!("../tests/golden/shallow-rules.plist");
        let tail = |t: &str| t[t.find("\t<key>rules</key>").unwrap()..].to_string();
        assert_eq!(tail(&text), tail(golden));
    }

    #[test]
    fn rules_text_matches_codesign() {
        let s = String::from_utf8(seal(&[], &[HashType::Sha256]).unwrap()).unwrap();
        let golden = include_str!("../tests/golden/empty-seal-rules.plist");
        assert_eq!(s, golden);
    }
}
