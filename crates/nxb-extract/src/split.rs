//! The split stage over extracted versions: walks each version's `RomFS.img` as `shader_split.py` walks it through
//! `romfs.py` (sorted paths, versions in order, later versions prefixed with their directory name), scans `main` of each
//! version first, scans the files with `threads` workers, and keeps the results in the Python's order, so the outputs
//! (`nxb_split::records`) are byte-identical.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use nxb_split::SplitError;
use nxb_split::records::{FileResult, Outputs, build, uniq};
use nxb_split::shaders::{BahsVar, Found, Profile, memfind, scan_blob, var_pairs};

use nxb_model::{Digest, sha256};

use crate::error::{ExtractError, Result};

/// A file of an image: (relative path, offset, size).
pub type ImageFile = (String, u64, u64);

/// Lists the files of a `DMROMFS1` image with their paths relative to the RomFS root.
pub fn image_files(path: &Path) -> Result<(File, Vec<ImageFile>)> {
    let f =
        File::open(path).map_err(|e| ExtractError::io(format!("open {}", path.display()), e))?;
    let mut h = [0u8; 32];
    f.read_exact_at(&mut h, 0)
        .map_err(|e| ExtractError::io("read image header", e))?;
    if &h[..8] != b"DMROMFS1" {
        return Err(ExtractError::format(format!(
            "{}: not a DMROMFS1 image",
            path.display()
        )));
    }
    let rd = |o: usize| {
        u64::from_le_bytes([
            h[o],
            h[o + 1],
            h[o + 2],
            h[o + 3],
            h[o + 4],
            h[o + 5],
            h[o + 6],
            h[o + 7],
        ])
    };
    let (index, size, count) = (rd(8), rd(16), rd(24) as usize);
    if size > (1 << 30) || count.checked_mul(24).is_none_or(|n| n as u64 > size) {
        return Err(ExtractError::format("image index size"));
    }
    let mut idx = vec![0u8; size as usize];
    f.read_exact_at(&mut idx, index)
        .map_err(|e| ExtractError::io("read image index", e))?;
    let names = count * 24;
    let ent = |i: usize| {
        let e = &idx[i * 24..i * 24 + 24];
        let a = u64::from_le_bytes([e[0], e[1], e[2], e[3], e[4], e[5], e[6], e[7]]);
        let b = u64::from_le_bytes([e[8], e[9], e[10], e[11], e[12], e[13], e[14], e[15]]);
        let no = u32::from_le_bytes([e[16], e[17], e[18], e[19]]) as usize;
        let nl = u16::from_le_bytes([e[20], e[21]]) as usize;
        (a, b, no, nl, e[22] == 0)
    };
    let mut out = Vec::new();
    // (entry, path)
    let mut stack = vec![(0usize, String::new())];
    while let Some((i, p)) = stack.pop() {
        let (a, b, _, _, dir) = ent(i);
        if !dir {
            continue;
        }
        for c in a as usize..(a + b) as usize {
            if c >= count {
                return Err(ExtractError::format("image entry outside the index"));
            }
            let (ca, cb, no, nl, cdir) = ent(c);
            let name = idx
                .get(names + no..names + no + nl)
                .ok_or_else(|| ExtractError::format("image name outside its table"))?;
            let name = String::from_utf8_lossy(name);
            let cp = if p.is_empty() {
                name.into_owned()
            } else {
                format!("{p}/{name}")
            };
            if cdir {
                stack.push((c, cp));
            } else {
                out.push((cp, ca, cb));
            }
        }
    }
    Ok((f, out))
}

/// Where the split writes.
#[derive(Clone, Debug)]
pub struct SplitOutputs {
    /// Package directory.
    pub pkg: PathBuf,
    /// `shader_map.tsv`.
    pub map: PathBuf,
    /// `shader_facts.tsv`.
    pub facts: PathBuf,
    /// `shader_pairs.tsv`.
    pub pairs: PathBuf,
    /// `pso_pairs.tsv`.
    pub pso_pairs: PathBuf,
}

/// The split over extracted version directories (`.../v0` with `RomFS.img` and `ExeFS/main`), base first.
#[derive(Clone, Debug)]
pub struct SplitRequest {
    /// Version directories.
    pub versions: Vec<PathBuf>,
    /// The profile's `game.tsv`.
    pub game_tsv: PathBuf,
    /// Outputs.
    pub outputs: SplitOutputs,
    /// Workers.
    pub threads: usize,
}

/// Whether any of `pats` occurs in `d` (one pass, candidates by first byte).
fn any_of(d: &[u8], pats: &[&[u8]]) -> bool {
    let mut first = [false; 256];
    for p in pats {
        if let Some(&b) = p.first() {
            first[b as usize] = true;
        }
    }
    for (i, &b) in d.iter().enumerate() {
        if first[b as usize] && pats.iter().any(|p| d[i..].starts_with(p)) {
            return true;
        }
    }
    false
}

fn scan_file(
    p: &Profile,
    data: &[u8],
    label: &str,
) -> std::result::Result<Option<FileResult>, SplitError> {
    let d: std::borrow::Cow<'_, [u8]> = if data.get(..4) == Some(&[0x28, 0xb5, 0x2f, 0xfd][..]) {
        nxb_split::zstd::decompress(data)?.into()
    } else {
        data.into()
    };
    let zst = [0x28, 0xb5, 0x2f, 0xfd];
    let lz4f = [0x04, 0x22, 0x4d, 0x18];
    let sized =
        data.get(..4) != Some(&zst[..]) && data.get(4..8).is_some_and(|m| m == zst || m == lz4f);
    if sized || d.get(..8) == Some(b"UnityFS\0") {
        return Err(SplitError::Unsupported(format!(
            "{label}: a container this port does not read yet"
        )));
    }
    let d: std::borrow::Cow<'_, [u8]> = if d.get(..4) == Some(b"Yaz0") {
        nxb_split::yaz0::decode(&d, label)?.into()
    } else {
        d
    };
    let d = &*d;
    let head = d.get(..4).unwrap_or(&[]);
    // the prefilter's markers in one pass over the bytes (it was one search per marker)
    const MARKERS: [&[u8]; 7] = [
        b"BNSH\0\0\0\0",
        b"BAHS",
        &[0x91, 0x68, 0x86, 0x19],
        b"MGFX",
        b"HNSHV002",
        b"Ninty_NX\0\0\0\0\0\0\0\0",
        b"NVNsh\0",
    ];
    let any_marker = any_of(d, &MARKERS);
    let glslc = || memfind(d, &[0x91, 0x68, 0x86, 0x19]).is_some();
    let wanted = any_marker
        || head == b"MSHD"
        || head == b"NARC"
        || (head == b"SARC" && memfind(d, b"Yaz0").is_some())
        || d.get(..8) == Some(b"MuxFDMR\x02");
    if !wanted {
        return Ok(None);
    }
    let mut found: Vec<Found> = Vec::new();
    let mut var = BahsVar::new();
    scan_blob(d, label, &mut found, &mut var, 0)?;
    if found.is_empty() && head != b"MSHD" && glslc() {
        return Err(SplitError::Unsupported(format!("{label}: GLSLC outputs")));
    }
    let pairs = var_pairs(p, &found, &var);
    Ok(Some(FileResult {
        keys: uniq(p, found, false)?,
        pairs,
    }))
}

fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)
            .map_err(|e| ExtractError::io(format!("create {}", d.display()), e))?;
    }
    std::fs::write(path, bytes)
        .map_err(|e| ExtractError::io(format!("write {}", path.display()), e))
}

fn split_err(e: SplitError) -> ExtractError {
    match e {
        SplitError::Unsupported(m) => ExtractError::Unsupported(m),
        other => ExtractError::format(other.to_string()),
    }
}

/// One version for [`Templates::merge`]: its tag (empty for the base), `ExeFS/main` and its files with digests.
pub struct VersionIn {
    /// Directory name of a later version (`v327680`), empty for the base.
    pub tag: String,
    /// The `main` NSO file.
    pub main: Vec<u8>,
    /// (path relative to the RomFS root, SHA-256 of its bytes).
    pub files: Vec<(String, Digest)>,
}

type Template = Arc<OnceLock<std::result::Result<Option<FileResult>, ()>>>;

/// Scan results by content: a file's bytes are scanned once whatever its path or version (its shaders are labelled
/// at merge time), so an update's unchanged files, and copies of one file, cost one scan. Exact: a later copy of
/// content adds no key and no pair the first copy did not.
pub struct Templates {
    prof: Profile,
    game_tsv: String,
    manual: Option<String>,
    by_digest: Mutex<HashMap<Digest, Template>>,
    errors: Mutex<HashMap<String, String>>,
    unsupported: Mutex<Option<String>>,
    scanned: AtomicUsize,
}

fn relabel(r: &FileResult, label: &str) -> FileResult {
    FileResult {
        keys: r
            .keys
            .iter()
            .map(|(k, f)| {
                let mut f = f.clone();
                f.place = format!("{label}{}", f.place);
                (k.clone(), f)
            })
            .collect(),
        pairs: r.pairs.clone(),
    }
}

impl Templates {
    /// Reads the profile.
    pub fn new(game_tsv: &Path) -> Result<Templates> {
        let text = std::fs::read_to_string(game_tsv)
            .map_err(|e| ExtractError::io(format!("read {}", game_tsv.display()), e))?;
        Ok(Templates {
            prof: Profile::from_game_tsv(&text),
            game_tsv: text,
            manual: game_tsv
                .parent()
                .and_then(|d| std::fs::read_to_string(d.join("shader_facts_manual.tsv")).ok()),
            by_digest: Mutex::new(HashMap::new()),
            errors: Mutex::new(HashMap::new()),
            unsupported: Mutex::new(None),
            scanned: AtomicUsize::new(0),
        })
    }

    /// Scans one file (any thread). `label` is `/` + its path as the Python names it.
    pub fn scan(&self, label: &str, data: &[u8], digest: Digest) {
        let slot = match self.by_digest.lock() {
            Ok(mut g) => g.entry(digest).or_default().clone(),
            Err(_) => return,
        };
        let r = slot.get_or_init(|| {
            self.scanned.fetch_add(1, Ordering::Relaxed);
            match scan_file(&self.prof, data, "") {
                Ok(r) => Ok(r),
                Err(SplitError::Unsupported(m)) => {
                    if let Ok(mut g) = self.unsupported.lock() {
                        g.get_or_insert(format!("{label}{m}"));
                    }
                    Err(())
                }
                Err(_) => Err(()),
            }
        });
        if r.is_err() {
            // the message names this path: scan again with the real label
            if let Err(e) = scan_file(&self.prof, data, label) {
                let why = e.to_string();
                let rel = label.trim_start_matches('/');
                let why = if why.trim_start_matches('/').starts_with(rel) {
                    why
                } else {
                    format!("{rel}: {why}")
                };
                if let Ok(mut g) = self.errors.lock() {
                    g.insert(label.to_string(), why);
                }
            }
        }
    }

    /// Distinct contents scanned.
    pub fn scanned(&self) -> usize {
        self.scanned.load(Ordering::Relaxed)
    }

    /// Orders the results as the Python does and builds and writes the outputs.
    pub fn merge(
        self,
        versions: &[VersionIn],
        req: &SplitOutputs,
    ) -> Result<(Outputs, Vec<String>)> {
        if let Some(m) = self.unsupported.into_inner().ok().flatten() {
            return Err(ExtractError::Unsupported(m));
        }
        let prof = &self.prof;
        let mut results = Vec::new();
        for v in versions {
            let img = nxb_split::nso::image(&v.main).map_err(split_err)?;
            let label = if v.tag.is_empty() {
                "main".to_string()
            } else {
                format!("main@{}", v.tag)
            };
            let mut found = Vec::new();
            let mut var = BahsVar::new();
            scan_blob(&img, &label, &mut found, &mut var, 0).map_err(split_err)?;
            let pairs = var_pairs(prof, &found, &var);
            results.push(FileResult {
                keys: uniq(prof, found, true).map_err(split_err)?,
                pairs,
            });
        }
        let by = self.by_digest.into_inner().unwrap_or_default();
        let errors = self.errors.into_inner().unwrap_or_default();
        let mut seen: HashSet<Digest> = HashSet::new();
        let mut bad = Vec::new();
        for v in versions {
            let mut files: Vec<&(String, Digest)> = v.files.iter().collect();
            files.sort_by(|a, b| a.0.cmp(&b.0));
            for (rel, d) in files {
                let label = if v.tag.is_empty() {
                    format!("/{rel}")
                } else {
                    format!("/{}/{rel}", v.tag)
                };
                if let Some(why) = errors.get(&label) {
                    bad.push(format!("[RED] {why} (its shaders are left out)"));
                    continue;
                }
                let t = by
                    .get(d)
                    .and_then(|t| t.get())
                    .ok_or_else(|| ExtractError::format(format!("{label}: not scanned")))?;
                if let Ok(Some(r)) = t
                    && seen.insert(*d)
                {
                    results.push(relabel(r, &label));
                }
            }
        }
        let out = build(prof, &results).map_err(split_err)?;
        write_outputs(req, &self.game_tsv, self.manual.as_deref(), &out, bad.len())?;
        Ok((out, bad))
    }
}

/// Runs the split on extracted version directories, returns the outputs and the `[RED]` lines of unreadable files.
pub fn split(req: &SplitRequest) -> Result<(Outputs, Vec<String>)> {
    let t = Templates::new(&req.game_tsv)?;
    let mut versions = Vec::new();
    let mut jobs: Vec<(usize, String, u64, u64)> = Vec::new();
    let mut images = Vec::new();
    for (i, v) in req.versions.iter().enumerate() {
        let tag = if i == 0 {
            String::new()
        } else {
            v.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        let mainp = v.join("ExeFS/main");
        let main = std::fs::read(&mainp)
            .map_err(|e| ExtractError::io(format!("read {}", mainp.display()), e))?;
        let (f, files) = image_files(&v.join("RomFS.img"))?;
        for (p, off, size) in &files {
            let label = if tag.is_empty() {
                format!("/{p}")
            } else {
                format!("/{tag}/{p}")
            };
            jobs.push((images.len(), label, *off, *size));
        }
        images.push(f);
        versions.push((tag, main, files));
    }
    let digests: Vec<Mutex<Option<Digest>>> = jobs.iter().map(|_| Mutex::new(None)).collect();
    let next = AtomicUsize::new(0);
    let fatal: Mutex<Option<ExtractError>> = Mutex::new(None);
    // NXB_SPLIT_LPT=1: largest first (results are merged in the Python's order by digest, not in completion order).
    // Measured on the reference title: warm cache 8.4 s against 9.6 s in input order, cold from the USB disk 31 s against 12 s
    // (all workers read big files at once), so input order stays the default.
    let mut order: Vec<usize> = (0..jobs.len()).collect();
    if std::env::var_os("NXB_SPLIT_LPT").is_some() {
        order.sort_by_key(|&j| std::cmp::Reverse(jobs[j].3));
    }
    std::thread::scope(|s| {
        for _ in 0..req.threads.max(1) {
            s.spawn(|| {
                loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&j) = order.get(k) else {
                        break;
                    };
                    let (im, label, off, size) = &jobs[j];
                    let mut data = vec![0u8; *size as usize];
                    if let Err(e) = images[*im].read_exact_at(&mut data, *off) {
                        if let Ok(mut g) = fatal.lock() {
                            *g = Some(ExtractError::io(format!("read {label}"), e));
                        }
                        break;
                    }
                    let d = sha256(&data);
                    t.scan(label, &data, d);
                    if let Ok(mut g) = digests[j].lock() {
                        *g = Some(d);
                    }
                }
            });
        }
    });
    if let Some(e) = fatal.into_inner().ok().flatten() {
        return Err(e);
    }
    let mut ds = digests.into_iter().map(|m| m.into_inner().ok().flatten());
    let mut vin = Vec::new();
    for (tag, main, files) in versions {
        let mut fl = Vec::new();
        for (p, _, _) in files {
            fl.push((
                p,
                ds.next()
                    .flatten()
                    .ok_or_else(|| ExtractError::format("a split job did not finish"))?,
            ));
        }
        vin.push(VersionIn {
            tag,
            main,
            files: fl,
        });
    }
    eprintln!(
        "nxb-split: {} distinct files scanned of {}",
        t.scanned(),
        jobs.len()
    );
    t.merge(&vin, &req.outputs)
}

fn write_outputs(
    req: &SplitOutputs,
    game_tsv: &str,
    manual: Option<&str>,
    out: &Outputs,
    bad: usize,
) -> Result<()> {
    let mut facts = out.facts.clone();
    if let Some(m) = manual {
        crate::facts::merge_manual(&mut facts, m).map_err(ExtractError::Format)?;
    }
    std::fs::create_dir_all(&req.pkg)
        .map_err(|e| ExtractError::io(format!("create {}", req.pkg.display()), e))?;
    if let Ok(rd) = std::fs::read_dir(&req.pkg) {
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().ends_with(".bin") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    for (name, bytes) in &out.packages {
        write(&req.pkg.join(name), bytes)?;
    }
    let note = game_tsv
        .lines()
        .find_map(|l| l.strip_prefix("shader_note\t"))
        .unwrap_or("None");
    let kinds = if out
        .rows
        .iter()
        .any(|r| r.4.contains("#BNSH@") || r.4.contains("#BAHS@"))
    {
        "BNSH / BAHS"
    } else {
        ""
    };
    let mut m = format!(
        "# kind: evidence  {note} ({kinds} in RomFS + main) \u{2192} control section + code area keys (dm/gfx/tools/shader_split.py)\n# column 5 records where each key first appears; no PC shader pairing supplied\npackage\tindex\tstage\tdm_ctl_sha16\tsource\n"
    );
    for r in &out.rows {
        m.push_str(&format!("{}\t{}\t{}\t{}\t{}\n", r.0, r.1, r.2, r.3, r.4));
    }
    write(&req.map, m.as_bytes())?;
    let mut f = String::from(
        "# Answer table for translation queries (generated by dm/gfx/tools/shader_split.py, do not edit): key <TAB> fact JSON. No PC shader pairing supplied: only the thread group / shared memory of compute shaders (taken from the control section); other queries get the translator defaults\n",
    );
    for (k, v) in &facts {
        f.push_str(&format!("{k}\t{v}\n"));
    }
    write(&req.facts, f.as_bytes())?;
    let mut p = String::from(
        "# kind: evidence  vertex / pixel shader pairs within BNSH variations (generated by dm/gfx/tools/shader_split.py, do not edit): vertex key <TAB> pixel key\n",
    );
    for (a, b) in &out.pairs {
        p.push_str(&format!("{a}\t{b}\n"));
    }
    write(&req.pairs, p.as_bytes())?;
    let mut q = String::from(
        "# kind: evidence  supplemental vertex / pixel shader pairs (BAHS program variations and legacy Unity pass-input covers) for vertex output padding and the pipeline prebuild (generated by dm/gfx/tools/shader_split.py, do not edit): vertex key <TAB> pixel key\n",
    );
    for (a, b) in &out.pso_pairs {
        q.push_str(&format!("{a}\t{b}\n"));
    }
    write(&req.pso_pairs, q.as_bytes())?;
    if bad > 0 {
        println!("[RED] {bad} file(s) could not be read, their shaders are left out");
    }
    let (stat, src) = &out.stats;
    let stages: Vec<String> = stat
        .iter()
        .map(|(s, n)| format!("{} {n}", stage_long(s)))
        .collect();
    let sources: Vec<String> = src.iter().map(|(g, n)| format!("{g} {n}")).collect();
    println!(
        "shaders: {} ({}); sources {}; compute shader answers {} rows; variation pairs {}",
        out.rows.len(),
        stages.join(", "),
        sources.join(", "),
        facts.len(),
        out.pairs.len()
    );
    Ok(())
}

fn stage_long(s: &str) -> &'static str {
    match s {
        "vs" => "vertex",
        "ps" => "pixel",
        "cs" => "compute",
        "gs" => "geometry",
        "hs" => "hull",
        "ds" => "domain",
        _ => "?",
    }
}
