//! Incremental staging: update the previous app in place instead of staging all of it again. A file whose digest
//! equals the one the previous seal (`CodeResources`) records for its path stays as it is (its digests come from the
//! seal, so it is not read), changed and new files are staged, files that left the plan are removed, then the bundle
//! is signed. The result is the bundle a fresh stage would give: signing depends only on the files.
//!
//! Executables (`Contents/MacOS/*`) are always staged again: the bundle holds their signed form, never the input.
//! The old `.app` is moved to the new path first (a rename on the same volume), a failure before the end leaves an
//! unsigned bundle, which the caller must not reuse (as `pack_app.py`, it drops its reuse key first).

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::path::Path;
use std::time::Instant;

use crate::appstage::{Content, StagePlan};
use crate::bundle::{self, BundleOptions, Layout};
use crate::digest::{self, DigestSource, FileDigests, HashJob};
use crate::error::{Error, Result, format_err};
use crate::plist::{self, Value};

/// What [`update`] did.
#[derive(Clone, Debug, Default)]
pub struct UpdateReport {
    /// Files kept as they were.
    pub kept: usize,
    /// Files staged (new or changed).
    pub staged: usize,
    /// Files removed.
    pub removed: usize,
    /// Files hashed to compare.
    pub hashed: usize,
    /// Seconds: compare, stage, sign.
    pub seconds: (f64, f64, f64),
}

impl fmt::Display for UpdateReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "updated in place: {} kept, {} staged, {} removed, {} hashed; compare {:.2} s, stage {:.2} s, sign {:.2} s",
            self.kept,
            self.staged,
            self.removed,
            self.hashed,
            self.seconds.0,
            self.seconds.1,
            self.seconds.2
        )
    }
}

fn data_of(v: &Value) -> Option<&[u8]> {
    match v {
        Value::Data(d) => Some(d),
        Value::Dict(d) => d
            .iter()
            .find(|(k, _)| k == "hash" || k == "hash2")
            .and_then(|(_, v)| data_of(v)),
        _ => None,
    }
}

/// The digests the previous seal records, by path relative to the sealed root.
fn sealed(root: &Path) -> Result<HashMap<String, FileDigests>> {
    let p = root.join("_CodeSignature/CodeResources");
    let v = plist::parse(&fs::read(&p).map_err(|e| Error::io(&p, e))?)?;
    let Value::Dict(top) = v else {
        return Err(format_err("CodeResources is not a dictionary"));
    };
    let get = |name: &str| top.iter().find(|(k, _)| k == name).map(|(_, v)| v);
    let mut out: HashMap<String, FileDigests> = HashMap::new();
    if let Some(Value::Dict(files2)) = get("files2") {
        for (k, v) in files2 {
            if let Some(d) = data_of(v).and_then(|d| <[u8; 32]>::try_from(d).ok()) {
                out.insert(
                    k.clone(),
                    FileDigests {
                        sha1: None,
                        sha256: d,
                    },
                );
            }
        }
    }
    if let Some(Value::Dict(files)) = get("files") {
        for (k, v) in files {
            if let (Some(e), Some(d)) = (
                out.get_mut(k),
                data_of(v).and_then(|d| <[u8; 20]>::try_from(d).ok()),
            ) {
                e.sha1 = Some(d);
            }
        }
    }
    Ok(out)
}

struct MapDigests(HashMap<String, (u64, FileDigests)>);

impl DigestSource for MapDigests {
    fn lookup(&self, path: &str, size: u64) -> Option<FileDigests> {
        self.0
            .get(path)
            .filter(|(s, _)| *s == size)
            .map(|(_, d)| *d)
    }
}

/// Updates `prev` (moved to `out` when they differ) to `plan` and signs it.
pub fn update(plan: &StagePlan, prev: &Path, out: &Path, threads: usize) -> Result<UpdateReport> {
    let t0 = Instant::now();
    if prev != out {
        if out.exists() {
            return Err(Error::Refused(format!("{} exists", out.display())));
        }
        fs::rename(prev, out).map_err(|e| Error::io(prev, e))?;
    }
    let layout = Layout::detect(out)?;
    let prefix = match layout.root.strip_prefix(out).ok().and_then(|r| r.to_str()) {
        Some("") => String::new(),
        Some(r) => format!("{r}/"),
        None => return Err(Error::Unsupported("sealed root outside the bundle".into())),
    };
    let old = sealed(&layout.root)?;
    let mut report = UpdateReport::default();
    // new digests: given, else hashed (files) or computed (bytes)
    let mut digests: Vec<Option<FileDigests>> = plan.entries.iter().map(|e| e.digests).collect();
    let mut sizes = vec![0u64; plan.entries.len()];
    let mut jobs = Vec::new();
    let mut job_of = Vec::new();
    for (i, e) in plan.entries.iter().enumerate() {
        match &e.content {
            Content::Bytes(b) => {
                sizes[i] = b.len() as u64;
                if digests[i].is_none() {
                    let d = nxb_model::DualDigest::of(b);
                    digests[i] = Some(FileDigests {
                        sha1: Some(d.sha1.0),
                        sha256: d.file.sha256.0,
                    });
                }
            }
            Content::File(p) => {
                sizes[i] = fs::metadata(p).map_err(|x| Error::io(p, x))?.len();
                if digests[i].is_none() {
                    jobs.push(HashJob {
                        path: p.clone(),
                        size: sizes[i],
                        sha1: true,
                    });
                    job_of.push(i);
                }
            }
        }
    }
    report.hashed = jobs.len();
    for (d, i) in digest::hash_files(&jobs, threads)?.into_iter().zip(job_of) {
        digests[i] = Some(d);
    }
    let t1 = Instant::now();
    let mut map = HashMap::new();
    let mut keep_paths = HashSet::new();
    let mut stage = Vec::new();
    for (i, e) in plan.entries.iter().enumerate() {
        let d = digests[i].ok_or_else(|| format_err("missing digest"))?;
        keep_paths.insert(e.dest.clone());
        let rel = e.dest.strip_prefix(&prefix).map(str::to_string);
        let exe = e.dest.contains("/MacOS/");
        let same = !exe
            && rel
                .as_ref()
                .and_then(|r| old.get(r))
                .is_some_and(|o| o.sha256 == d.sha256)
            && fs::metadata(out.join(&e.dest)).is_ok_and(|m| m.len() == sizes[i]);
        // the old seal's SHA-1 when ours was not computed
        let d = match (d.sha1, rel.as_ref().and_then(|r| old.get(r))) {
            (None, Some(o)) if same => FileDigests {
                sha1: o.sha1,
                sha256: d.sha256,
            },
            _ => d,
        };
        if let Some(r) = rel {
            map.insert(r, (sizes[i], d));
        }
        if same {
            report.kept += 1;
        } else {
            stage.push(i);
        }
    }
    for &i in &stage {
        let e = &plan.entries[i];
        let dst = out.join(&e.dest);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent).map_err(|x| Error::io(parent, x))?;
        }
        let _ = fs::remove_file(&dst);
        match &e.content {
            Content::File(src) => fs::copy(src, &dst)
                .map(|_| ())
                .map_err(|x| Error::io(src, x))?,
            Content::Bytes(b) => fs::write(&dst, b).map_err(|x| Error::io(&dst, x))?,
        }
    }
    report.staged = stage.len();
    report.removed = remove_stale(out, out, &keep_paths)?;
    let t2 = Instant::now();
    let source = MapDigests(map);
    let opts = BundleOptions {
        entitlements: None,
        sign_nested: true,
        runtime: false,
        threads,
        digests: &source,
    };
    bundle::sign_layout(&Layout::detect(out)?, &opts)?;
    report.seconds = (
        (t1 - t0).as_secs_f64(),
        (t2 - t1).as_secs_f64(),
        t2.elapsed().as_secs_f64(),
    );
    Ok(report)
}

/// Removes files under `dir` that the plan does not hold (the signature folder aside) and folders left empty.
fn remove_stale(app: &Path, dir: &Path, keep: &HashSet<String>) -> Result<usize> {
    let mut n = 0;
    for e in fs::read_dir(dir).map_err(|x| Error::io(dir, x))? {
        let e = e.map_err(|x| Error::io(dir, x))?;
        let p = e.path();
        let rel = p
            .strip_prefix(app)
            .ok()
            .and_then(|r| r.to_str())
            .unwrap_or("")
            .to_string();
        if rel == "Contents/_CodeSignature" {
            continue;
        }
        let ft = e.file_type().map_err(|x| Error::io(&p, x))?;
        if ft.is_dir() {
            n += remove_stale(app, &p, keep)?;
            if fs::read_dir(&p)
                .map_err(|x| Error::io(&p, x))?
                .next()
                .is_none()
            {
                fs::remove_dir(&p).map_err(|x| Error::io(&p, x))?;
            }
        } else if !keep.contains(&rel) {
            fs::remove_file(&p).map_err(|x| Error::io(&p, x))?;
            n += 1;
        }
    }
    Ok(n)
}
