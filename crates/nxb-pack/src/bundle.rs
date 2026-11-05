//! Signing a bundle: nested code inside out, the resource seal, then the main executable.
//!
//! Three layouts are recognised ([`Layout::detect`]):
//! - deep (`X.app/Contents/…`, macOS): sealed root `Contents/`, Info.plist `Contents/Info.plist`,
//!   executable `Contents/MacOS/<CFBundleExecutable>`, macOS rules.
//! - versioned framework (`X.framework/Versions/Current -> A`): sealed root `Versions/A/`,
//!   Info.plist `Resources/Info.plist`, executable `Versions/A/<name>`, macOS rules.
//! - shallow (iOS, tvOS, ...): everything at the bundle root, shallow rules.
//!
//! Steps (as `codesign`, with nested code signed first the way the reference build script `pack_app.py` and
//! Xcode do it with separate `codesign` calls):
//! 1. Nested code, inside out: nested bundles (`Frameworks/X.framework`, `Helpers/X.app`,
//!    `PlugIns/X.appex`, ...) are signed recursively, nested Mach-O files standalone.
//! 2. Plan: walk the sealed root, classify every path by the rules.
//! 3. Digests: from the [`DigestSource`] when known, the rest hashed in parallel.
//! 4. Write `_CodeSignature/CodeResources`.
//! 5. Sign the main executable with Info.plist, requirements and CodeResources bound.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::digest::{self, DigestSource, FileDigests, HashJob};
use crate::error::{Error, Result, format_err};
use crate::hash::HashType;
use crate::macho;
use crate::macho_sign::{self, Identifier, SignParams};
use crate::plist;
use crate::resources::{self, Action, BundlePlan, PlanEntry, PlanKind, RuleSet, SealItem};

/// Options for [`sign_bundle`].
pub struct BundleOptions<'a> {
    /// Entitlements XML for the main executable (nested code gets none).
    pub entitlements: Option<Vec<u8>>,
    /// Sign nested code (as separate `codesign -f -s -` calls would, inside out) before
    /// sealing. When false it must already be signed and its existing cdhashes are sealed.
    pub sign_nested: bool,
    /// Hardened runtime (`--options runtime`) for everything this call signs.
    pub runtime: bool,
    /// Hashing threads.
    pub threads: usize,
    /// Digests already known, keyed by path relative to the sealed root.
    pub digests: &'a dyn DigestSource,
}

impl Default for BundleOptions<'_> {
    fn default() -> Self {
        BundleOptions {
            entitlements: None,
            sign_nested: true,
            runtime: false,
            threads: digest::default_threads(),
            digests: &digest::NoDigests,
        }
    }
}

/// What [`sign_bundle`] did.
#[derive(Clone, Debug, Default)]
pub struct BundleReport {
    /// Paths written to `files2` or `files` (after the rules).
    pub sealed: usize,
    /// Files read and hashed.
    pub hashed_files: usize,
    /// Bytes read and hashed.
    pub hashed_bytes: u64,
    /// Files whose digests came from the [`DigestSource`].
    pub reused_digests: usize,
    /// Nested Mach-O files and nested bundles signed.
    pub nested_signed: usize,
    /// Files this call wrote, relative to the sealed root (nested ones included): their old
    /// digests are stale, so they are never taken from the [`DigestSource`].
    pub written: Vec<String>,
    /// Size of the `CodeResources` written.
    pub code_resources_len: usize,
    /// The main executable's signing identifier.
    pub identifier: String,
    /// The main executable's cdhashes (hex), primary first.
    pub cdhashes: Vec<String>,
    /// Time per phase: nested, plan, digests, seal, main executable.
    pub timings: Vec<(&'static str, Duration)>,
}

/// Where a bundle's parts are.
#[derive(Clone, Debug)]
pub struct Layout {
    /// The directory whose content is sealed (holds `_CodeSignature/`).
    pub root: PathBuf,
    /// Info.plist, relative to `root`.
    pub info_plist: String,
    /// The main executable, relative to `root`.
    pub executable: String,
    /// The rule set.
    pub rules: RuleSet,
}

impl Layout {
    /// Recognises the layout of the bundle directory `bundle`.
    pub fn detect(bundle: &Path) -> Result<Layout> {
        let (root, info_plist, exe_dir, rules) = if bundle.join("Contents").is_dir() {
            (
                bundle.join("Contents"),
                "Info.plist",
                "MacOS/",
                RuleSet::MacOs,
            )
        } else if let Ok(target) = fs::read_link(bundle.join("Versions/Current")) {
            let t = target
                .to_str()
                .ok_or_else(|| Error::Unsupported("non-UTF-8 framework version".into()))?;
            if t.contains('/') {
                return Err(Error::Unsupported(format!("Versions/Current -> {t}")));
            }
            (
                bundle.join("Versions").join(t),
                "Resources/Info.plist",
                "",
                RuleSet::MacOs,
            )
        } else {
            (bundle.to_path_buf(), "Info.plist", "", RuleSet::Shallow)
        };
        let info_path = root.join(info_plist);
        let info = plist::parse(&fs::read(&info_path).map_err(|e| Error::io(&info_path, e))?)?;
        let exe = info
            .get("CFBundleExecutable")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                Error::Unsupported(format!("{} has no CFBundleExecutable", info_path.display()))
            })?;
        Ok(Layout {
            root,
            info_plist: info_plist.into(),
            executable: format!("{exe_dir}{exe}"),
            rules,
        })
    }

    /// The main executable's path.
    pub fn executable_path(&self) -> PathBuf {
        self.root.join(&self.executable)
    }
}

/// Folders of a shallow bundle whose sub-bundles are code (with a trailing `/`).
const SHALLOW_CODE_DIRS: [&str; 6] = [
    "Frameworks/",
    "PlugIns/",
    "Extensions/",
    "Watch/",
    "AppClips/",
    "XPCServices/",
];

/// Whether the directory `path` (relative to the sealed root) is a nested bundle position.
fn is_nested_bundle_dir(rules: RuleSet, path: &str, name: &str) -> bool {
    if !name.contains('.') {
        return false;
    }
    match rules {
        RuleSet::MacOs => resources::rule_v2(path) == Action::Nested,
        // Shallow bundles have no nested rule, code bundles live one level below these folders.
        RuleSet::Shallow => SHALLOW_CODE_DIRS
            .iter()
            .any(|d| path.strip_prefix(d).is_some_and(|rest| !rest.contains('/'))),
    }
}

fn utf8(p: &Path) -> Result<&str> {
    p.to_str()
        .ok_or_else(|| Error::Unsupported(format!("non-UTF-8 path {}", p.display())))
}

/// Walks the sealed root of `layout` and builds the plan.
pub fn plan_bundle(layout: &Layout) -> Result<BundlePlan> {
    let main = layout.executable.as_str();
    let mut plan = BundlePlan {
        main_executable: main.to_string(),
        entries: Vec::new(),
    };
    let mut stack = vec![(layout.root.clone(), String::new())];
    while let Some((dir, rel)) = stack.pop() {
        let rd = fs::read_dir(&dir).map_err(|e| Error::io(&dir, e))?;
        for ent in rd {
            let ent = ent.map_err(|e| Error::io(&dir, e))?;
            let name = ent.file_name();
            let name = name.to_str().ok_or_else(|| {
                Error::Unsupported(format!("non-UTF-8 name in {}", dir.display()))
            })?;
            let path = if rel.is_empty() {
                name.to_string()
            } else {
                format!("{rel}/{name}")
            };
            if path == "_CodeSignature"
                || path == main
                || (layout.rules == RuleSet::MacOs && path == "CodeResources")
            {
                continue;
            }
            let full = ent.path();
            let meta = fs::symlink_metadata(&full).map_err(|e| Error::io(&full, e))?;
            let ft = meta.file_type();
            let nested_rule =
                layout.rules == RuleSet::MacOs && resources::rule_v2(&path) == Action::Nested;
            if ft.is_symlink() {
                let target = fs::read_link(&full).map_err(|e| Error::io(&full, e))?;
                plan.entries.push(PlanEntry {
                    path,
                    kind: PlanKind::Symlink(utf8(&target)?.to_string()),
                    size: 0,
                });
            } else if ft.is_dir() {
                if nested_rule && name.contains('.') {
                    plan.entries.push(PlanEntry {
                        path,
                        kind: PlanKind::NestedBundle,
                        size: 0,
                    });
                } else {
                    stack.push((full, path));
                }
            } else if ft.is_file() {
                let kind = if nested_rule {
                    PlanKind::NestedCode
                } else {
                    PlanKind::File
                };
                plan.entries.push(PlanEntry {
                    path,
                    kind,
                    size: meta.len(),
                });
            } else {
                return Err(Error::Unsupported(format!("special file {path}")));
            }
        }
    }
    Ok(plan)
}

fn write_atomic(path: &Path, data: &[u8], mode_from: Option<&Path>) -> Result<()> {
    let tmp = path.with_file_name(format!(
        ".{}.nxb-codesign.tmp",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("file")
    ));
    fs::write(&tmp, data).map_err(|e| Error::io(&tmp, e))?;
    if let Some(src) = mode_from {
        let perm = fs::metadata(src)
            .map_err(|e| Error::io(src, e))?
            .permissions();
        fs::set_permissions(&tmp, perm).map_err(|e| Error::io(&tmp, e))?;
    }
    fs::rename(&tmp, path).map_err(|e| Error::io(path, e))
}

/// Signs `path` standalone, as `codesign -f -s - path`.
pub fn sign_file(path: &Path, runtime: bool) -> Result<macho_sign::SignedMachO> {
    let data = fs::read(path).map_err(|e| Error::io(path, e))?;
    if !macho::is_macho(&data) {
        return Err(Error::Refused(format!(
            "code object is not signed at all: {}",
            path.display()
        )));
    }
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("a.out");
    let mut params = SignParams::standalone(name);
    params.runtime = runtime;
    let signed = macho_sign::sign_macho(&data, &params)?;
    write_atomic(path, &signed.data, Some(path))?;
    Ok(signed)
}

/// Digest lookups for a nested bundle, forwarded to the outer source with a path prefix.
struct Prefixed<'a> {
    inner: &'a dyn DigestSource,
    prefix: String,
}

impl DigestSource for Prefixed<'_> {
    fn lookup(&self, path: &str, size: u64) -> Option<FileDigests> {
        self.inner.lookup(&format!("{}{path}", self.prefix), size)
    }
}

/// Signs the nested code of `layout` inside out, returns how many items were signed and the
/// files written (relative to `layout.root`).
fn sign_nested_code(layout: &Layout, opts: &BundleOptions<'_>) -> Result<(usize, Vec<String>)> {
    let mut written = Vec::new();
    let mut bundles = Vec::new();
    let mut files = Vec::new();
    let mut stack = vec![(layout.root.clone(), String::new())];
    while let Some((dir, rel)) = stack.pop() {
        for ent in fs::read_dir(&dir).map_err(|e| Error::io(&dir, e))? {
            let ent = ent.map_err(|e| Error::io(&dir, e))?;
            let name = ent.file_name();
            let Some(name) = name.to_str() else { continue };
            let path = if rel.is_empty() {
                name.to_string()
            } else {
                format!("{rel}/{name}")
            };
            if path == "_CodeSignature" || path == layout.executable {
                continue;
            }
            let ft = ent.file_type().map_err(|e| Error::io(ent.path(), e))?;
            if ft.is_dir() {
                if is_nested_bundle_dir(layout.rules, &path, name) {
                    bundles.push(path);
                } else if layout.rules == RuleSet::MacOs
                    || (rel.is_empty()
                        && SHALLOW_CODE_DIRS
                            .iter()
                            .any(|d| d.strip_suffix('/') == Some(name)))
                {
                    stack.push((ent.path(), path));
                }
            } else if ft.is_file() {
                let nested_file = match layout.rules {
                    RuleSet::MacOs => resources::rule_v2(&path) == Action::Nested,
                    // Shallow: dylibs and other Mach-O files directly in Frameworks/.
                    RuleSet::Shallow => path
                        .strip_prefix("Frameworks/")
                        .is_some_and(|r| !r.contains('/')),
                };
                if nested_file {
                    files.push(path);
                }
            }
        }
    }
    let mut count = 0;
    for b in &bundles {
        let dir = layout.root.join(b);
        let inner_layout = Layout::detect(&dir)?;
        let inner_rel = inner_layout
            .root
            .strip_prefix(&layout.root)
            .map_err(|_| format_err("nested root"))?;
        let prefix = format!("{}/", utf8(inner_rel)?);
        let digests = Prefixed {
            inner: opts.digests,
            prefix: prefix.clone(),
        };
        let inner = BundleOptions {
            entitlements: None,
            sign_nested: true,
            runtime: opts.runtime,
            threads: opts.threads,
            digests: &digests,
        };
        let r = sign_layout(&inner_layout, &inner)?;
        count += 1 + r.nested_signed;
        written.extend(r.written.iter().map(|w| format!("{prefix}{w}")));
    }
    // Standalone Mach-O files, in parallel. On shallow bundles only Mach-O files are signed
    // (resource files in Frameworks/ stay as they are), on macOS a non-Mach-O file in a nested
    // location is refused, as codesign refuses it.
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: Vec<Result<Vec<String>>> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..opts.threads.clamp(1, files.len().max(1)))
            .map(|_| {
                s.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(f) = files.get(i) else { break };
                        let full = layout.root.join(f);
                        if layout.rules == RuleSet::Shallow {
                            let mut head = [0u8; 4];
                            let is = fs::File::open(&full)
                                .and_then(|mut h| std::io::Read::read(&mut h, &mut head))
                                .map(|k| k == 4 && macho::is_macho(&head))
                                .map_err(|e| Error::io(&full, e))?;
                            if !is {
                                continue;
                            }
                        }
                        sign_file(&full, opts.runtime)?;
                        done.push(f.clone());
                    }
                    Ok(done)
                })
            })
            .collect();
        hs.into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err(format_err("nested signer panicked")))
            })
            .collect()
    });
    for r in results {
        let done = r?;
        count += done.len();
        written.extend(done);
    }
    Ok((count, written))
}

/// Signs the bundle directory `bundle` in place.
pub fn sign_bundle(bundle: &Path, opts: &BundleOptions<'_>) -> Result<BundleReport> {
    sign_layout(&Layout::detect(bundle)?, opts)
}

/// Signs a bundle whose layout is known.
pub fn sign_layout(layout: &Layout, opts: &BundleOptions<'_>) -> Result<BundleReport> {
    let mut report = BundleReport::default();
    let mut t = Instant::now();
    let mut lap = |report: &mut BundleReport, name: &'static str| {
        report.timings.push((name, t.elapsed()));
        t = Instant::now();
    };
    let set = layout.rules;
    let info_path = layout.root.join(&layout.info_plist);
    let info_bytes = fs::read(&info_path).map_err(|e| Error::io(&info_path, e))?;
    let info = plist::parse(&info_bytes)?;
    let exe_name = layout
        .executable
        .rsplit('/')
        .next()
        .unwrap_or(&layout.executable)
        .to_string();
    let identifier = match info.get("CFBundleIdentifier").and_then(|v| v.as_str()) {
        Some(id) => Identifier::Derived(id.to_string()),
        None => Identifier::FileName(exe_name),
    };
    let main_path = layout.executable_path();
    let main_data = fs::read(&main_path).map_err(|e| Error::io(&main_path, e))?;
    let digest_types = macho_sign::main_image(&main_data)?.digest_types();
    let with_sha1 = digest_types.contains(&HashType::Sha1);

    if opts.sign_nested {
        (report.nested_signed, report.written) = sign_nested_code(layout, opts)?;
    }
    lap(&mut report, "nested");

    let plan = plan_bundle(layout)?;
    lap(&mut report, "plan");

    let written: std::collections::HashSet<&str> =
        report.written.iter().map(String::as_str).collect();
    let mut items: Vec<(String, SealItem)> = Vec::with_capacity(plan.entries.len());
    let mut jobs = Vec::new();
    let mut job_slots = Vec::new();
    for e in &plan.entries {
        let item = match &e.kind {
            PlanKind::Symlink(target) => SealItem::Symlink(target.clone()),
            PlanKind::NestedCode => {
                let full = layout.root.join(&e.path);
                let data = fs::read(&full).map_err(|err| Error::io(&full, err))?;
                if !macho::is_macho(&data) {
                    return Err(Error::Refused(format!(
                        "code object is not signed at all: {}",
                        e.path
                    )));
                }
                SealItem::Nested(macho_sign::cdhashes_of(&data)?)
            }
            PlanKind::NestedBundle => {
                let nl = Layout::detect(&layout.root.join(&e.path))?;
                let exe = nl.executable_path();
                SealItem::Nested(macho_sign::cdhashes_of(
                    &fs::read(&exe).map_err(|err| Error::io(&exe, err))?,
                )?)
            }
            PlanKind::File => {
                let (v1, v2) = (e.needs_sha1_v1(set), e.hashed_v2(set));
                if !v1 && !v2 {
                    continue;
                }
                let need1 = v1 || (with_sha1 && v2);
                let known = if written.contains(e.path.as_str()) {
                    None
                } else {
                    opts.digests.lookup(&e.path, e.size)
                };
                match known {
                    Some(d) if !need1 || d.sha1.is_some() => {
                        report.reused_digests += 1;
                        SealItem::File {
                            sha1: d.sha1,
                            sha256: d.sha256,
                        }
                    }
                    _ => {
                        job_slots.push(items.len());
                        jobs.push(HashJob {
                            path: layout.root.join(&e.path),
                            size: e.size,
                            sha1: need1,
                        });
                        SealItem::File {
                            sha1: None,
                            sha256: [0; 32],
                        }
                    }
                }
            }
        };
        items.push((e.path.clone(), item));
    }
    let digests: Vec<FileDigests> = digest::hash_files(&jobs, opts.threads)?;
    report.hashed_files = jobs.len();
    report.hashed_bytes = jobs.iter().map(|j| j.size).sum();
    for (slot, d) in job_slots.into_iter().zip(digests) {
        items[slot].1 = SealItem::File {
            sha1: d.sha1,
            sha256: d.sha256,
        };
    }
    report.sealed = items.len();
    drop(written);
    lap(&mut report, "digests");

    let code_resources = resources::seal_in(set, &items, &digest_types)?;
    let sig_dir = layout.root.join("_CodeSignature");
    fs::create_dir_all(&sig_dir).map_err(|e| Error::io(&sig_dir, e))?;
    write_atomic(&sig_dir.join("CodeResources"), &code_resources, None)?;
    report.code_resources_len = code_resources.len();
    lap(&mut report, "seal");

    let params = SignParams {
        identifier,
        info_plist: Some(info_bytes),
        code_resources: Some(code_resources),
        entitlements: opts.entitlements.clone(),
        runtime: opts.runtime,
    };
    let signed = macho_sign::sign_macho(&main_data, &params)?;
    write_atomic(&main_path, &signed.data, Some(&main_path))?;
    report.written.push("_CodeSignature/CodeResources".into());
    report.written.push(layout.executable.clone());
    report.identifier = signed.identifier;
    report.cdhashes = signed
        .cdhashes
        .iter()
        .map(|(_, h)| crate::hash::hex(h))
        .collect();
    lap(&mut report, "main");
    Ok(report)
}

/// Digests of every file the seal of `bundle` hashes, as `(path, size, digests)`, paths
/// relative to its sealed root (for a build store, and to test the precomputed-digest path).
/// Files of nested bundles are listed with their path from the outer root.
pub fn bundle_digests(bundle: &Path, threads: usize) -> Result<Vec<(String, u64, FileDigests)>> {
    let layout = Layout::detect(bundle)?;
    let mut files: Vec<String> = Vec::new();
    collect_hashed(&layout, "", &mut files)?;
    let mut sizes = Vec::with_capacity(files.len());
    for f in &files {
        let p = layout.root.join(f);
        sizes.push(fs::metadata(&p).map_err(|e| Error::io(&p, e))?.len());
    }
    let jobs: Vec<HashJob> = files
        .iter()
        .zip(&sizes)
        .map(|(f, s)| HashJob {
            path: layout.root.join(f),
            size: *s,
            sha1: true,
        })
        .collect();
    let d = digest::hash_files(&jobs, threads)?;
    Ok(files
        .into_iter()
        .zip(sizes)
        .zip(d)
        .map(|((p, s), d)| (p, s, d))
        .collect())
}

fn collect_hashed(layout: &Layout, prefix: &str, out: &mut Vec<String>) -> Result<()> {
    let plan = plan_bundle(layout)?;
    for e in &plan.entries {
        match e.kind {
            PlanKind::File if e.hashed_v2(layout.rules) || e.needs_sha1_v1(layout.rules) => {
                out.push(format!("{prefix}{}", e.path));
            }
            PlanKind::NestedBundle => {
                let nl = Layout::detect(&layout.root.join(&e.path))?;
                let rel = nl
                    .root
                    .strip_prefix(&layout.root)
                    .map_err(|_| format_err("nested root"))?;
                collect_hashed(&nl, &format!("{prefix}{}/", utf8(rel)?), out)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// The path of the CodeResources file of a bundle.
pub fn code_resources_path(bundle: &Path) -> Result<PathBuf> {
    Ok(Layout::detect(bundle)?
        .root
        .join("_CodeSignature/CodeResources"))
}
