//! The extract stage: every version of a game from one invocation (see the crate docs).

use std::collections::HashMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use crate::cache::{BlockCache, CacheStats};
use crate::error::{ExtractError, Result};
use crate::image::ImagePlan;
use crate::keys::KeySet;
use crate::nca::{Nca, open_nsp};
use crate::storage::{FileStorage, Storage};
use crate::version::{
    base_data, display_version, open_romfs, pick_programs, read_control, read_exefs, title_name,
};
use crate::write::{ImageJob, ImageResult, write_images};

/// What to extract.
#[derive(Clone, Debug)]
pub struct Request {
    /// `prod.keys`.
    pub keys: PathBuf,
    /// The base game's NSP.
    pub base: PathBuf,
    /// Output directory of `v0` (`ExeFS/`, `control.nacp`, `icon.jpg`, `RomFS.img`).
    pub out_base: PathBuf,
    /// Updates, each with its output directory (each is layered on the base alone, as `nx_extract` does).
    pub updates: Vec<(PathBuf, PathBuf)>,
    /// Worker threads for the images.
    pub threads: usize,
    /// Bytes of file data in flight at most.
    pub mem_limit: u64,
    /// Block cache capacity in 1 MiB chunks.
    pub cache_chunks: usize,
    /// Optional TSV of per-file digests (`version`, path, offset, size, sha256) and the image digests.
    pub digests: Option<PathBuf>,
    /// Write the base's own outputs (`v0`), false: only the updates (the base is read for them).
    pub base_outputs: bool,
    /// Also write the offline HTML pages (`<out>/html/html<n>/`, `nx_extract --html`).
    pub html: bool,
    /// The split, fused: `(game.tsv, outputs)`, files are scanned from the extract's bytes in memory.
    pub split: Option<(PathBuf, crate::split::SplitOutputs)>,
}

/// What the run did.
#[derive(Debug)]
pub struct Summary {
    /// Per image.
    pub images: Vec<ImageResult>,
    /// Block cache counters.
    pub cache: CacheStats,
    /// Seconds: open + small files, images.
    pub seconds: (f64, f64),
    /// The images closing in the background, wait before the process ends.
    pub closing: crate::write::Closing,
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)
            .map_err(|e| ExtractError::io(format!("create {}", d.display()), e))?;
    }
    std::fs::write(path, bytes)
        .map_err(|e| ExtractError::io(format!("write {}", path.display()), e))
}

fn small_outputs(
    ks: &KeySet,
    prog: &Nca,
    exefs_prog: &Nca,
    ctrl: Option<&Nca>,
    out: &Path,
    cache: &Arc<BlockCache>,
    log: &mut String,
) -> Result<()> {
    if let Some(c) = ctrl {
        let control = read_control(ks, c, cache)?;
        log.push_str(&format!(
            "display version {}\n",
            display_version(&control.nacp)
        ));
        match title_name(&control.nacp) {
            Ok(n) => log.push_str(&format!("name {n}\n")),
            Err(e) => log.push_str(&format!("name (not read: {e})\n")),
        }
        write_file(&out.join("control.nacp"), &control.nacp)?;
        match &control.icon {
            Some((lang, v)) => {
                write_file(&out.join("icon.jpg"), v)?;
                log.push_str(&format!("icon {lang} ({} bytes)\n", v.len()));
            }
            None => log.push_str("no icon in the control data\n"),
        }
    }
    if prog.sections[0].is_none() && exefs_prog.sections[0].is_none() {
        return Err(ExtractError::format("no code section"));
    }
    let mut files = read_exefs(ks, exefs_prog, cache)?;
    let dir = out.join("ExeFS");
    let bytes: usize = files.iter().map(|f| f.1.len()).sum();
    for (name, data) in &files {
        write_file(&dir.join(name), data)?;
    }
    let mb = bytes as f64 / 1_048_576.0;
    log.push_str(&format!(
        "{}: {} files {mb:.1} MB, 0 identical to the compare dir (cloned), {mb:.1} MB newly written\n",
        dir.display(),
        files.len()
    ));
    files.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, h) in &files {
        if h.len() >= 0x50 && &h[0..4] == b"NSO0" {
            log.push_str(&format!(
                "{name} build id {}\n",
                nxb_model::to_hex(&h[0x40..0x50])
            ));
        }
    }
    Ok(())
}

/// Runs the extract. Prints `nx_extract`'s lines per version (`== <out dir>` before each block).
pub fn extract(req: &Request) -> Result<Summary> {
    let t0 = Instant::now();
    let text = std::fs::read_to_string(&req.keys)
        .map_err(|e| ExtractError::io(format!("read {}", req.keys.display()), e))?;
    let mut ks = KeySet::parse(&text);
    drop(text);
    let cache = Arc::new(BlockCache::new(req.cache_chunks));
    let base_pkg: Arc<dyn Storage> = Arc::new(FileStorage::open(&req.base)?);
    let name = |p: &Path| {
        p.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let base = open_nsp(&mut ks, base_pkg)?;
    let (base, base_log) = crate::meta::filter(
        &ks,
        base,
        crate::meta::APPLICATION,
        None,
        &name(&req.base),
        &cache,
    )?;
    let mut updates = Vec::new();
    for (u, _) in &req.updates {
        let pkg: Arc<dyn Storage> = Arc::new(FileStorage::open(u)?);
        let ncas = open_nsp(&mut ks, pkg)?;
        updates.push(crate::meta::filter(
            &ks,
            ncas,
            crate::meta::PATCH,
            None,
            &name(u),
            &cache,
        )?);
    }

    // versions: (out dir, program, update program, control, log)
    let mut jobs = Vec::new();
    let mut logs = Vec::new();
    let base_shared;
    {
        let (bp, _, ctrl) = pick_programs(&base, None)?;
        let shared = base_data(&ks, bp, &cache)?;
        if req.base_outputs {
            let mut log = format!("{base_log}title {:016x}\n", bp.title_id);
            small_outputs(&ks, bp, bp, ctrl, &req.out_base, &cache, &mut log)?;
            if req.html {
                let h = crate::html::write_html(
                    &ks,
                    &base,
                    None,
                    bp.title_id,
                    &req.out_base.join("html"),
                    &cache,
                )?;
                log.push_str(&h);
            }
            let fs = open_romfs(&ks, shared.clone(), None, &cache)?;
            logs.push(log);
            jobs.push((req.out_base.clone(), fs));
        }
        base_shared = shared;
    }
    for ((u, u_log), (_, out)) in updates.iter().zip(&req.updates) {
        let (bp, up, ctrl) = pick_programs(&base, Some(u))?;
        let up = up.ok_or_else(|| ExtractError::format("no update program"))?;
        let mut log = format!(
            "{base_log}{u_log}title {:016x}, update {:016x}\n",
            bp.title_id, up.title_id
        );
        small_outputs(&ks, bp, up, ctrl, out, &cache, &mut log)?;
        if req.html {
            log.push_str(&crate::html::write_html(
                &ks,
                &base,
                Some(u.as_slice()),
                bp.title_id,
                &out.join("html"),
                &cache,
            )?);
        }
        let fs = open_romfs(&ks, base_shared.clone(), Some(up), &cache)?;
        logs.push(log);
        jobs.push((out.clone(), fs));
    }
    let t_small = t0.elapsed().as_secs_f64();

    // plans and merge keys: an update's file takes the index of the same path in v0
    let mut images = Vec::new();
    let mut v0_index: HashMap<Vec<u8>, u64> = HashMap::new();
    for (k, (out, fs)) in jobs.iter().enumerate() {
        let plan = ImagePlan::plan(&fs.entries)?;
        if k == 0 {
            v0_index = plan
                .entries
                .iter()
                .enumerate()
                .map(|(i, e)| (e.path.clone(), i as u64))
                .collect();
        }
        let mut last = 0u64;
        let keys = plan
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                last = if k == 0 {
                    i as u64
                } else {
                    v0_index.get(&e.path).copied().unwrap_or(last)
                };
                last
            })
            .collect();
        let src: Arc<dyn Storage> = fs.fs.clone();
        images.push(ImageJob {
            dst: out.join("RomFS.img"),
            plan,
            src,
            keys,
        });
    }
    let t1 = Instant::now();
    let templates = match &req.split {
        Some((game, _)) => Some(crate::split::Templates::new(game)?),
        None => None,
    };
    let tags: Vec<String> = jobs
        .iter()
        .enumerate()
        .map(|(k, (out, _))| {
            if k == 0 {
                String::new()
            } else {
                out.file_name()
                    .map(|n| format!("/{}", n.to_string_lossy()))
                    .unwrap_or_default()
            }
        })
        .collect();
    // The fused split: small files are scanned by the writer that has them, files of 1 MiB and more go to a scan
    // pool of their own (a bounded queue), so a writer never waits on a big zstd archive.
    let (big_tx, big_rx) =
        std::sync::mpsc::sync_channel::<(String, Vec<u8>, nxb_model::Digest)>(16);
    let big_rx = std::sync::Mutex::new(big_rx);
    let scan_threads = if templates.is_some() {
        (req.threads / 2).max(1)
    } else {
        0
    };
    let (results, closing) = std::thread::scope(|sc| {
        for _ in 0..scan_threads {
            sc.spawn(|| {
                loop {
                    let job = match big_rx.lock() {
                        Ok(rx) => rx.recv(),
                        Err(_) => break,
                    };
                    let Ok((label, data, d)) = job else { break };
                    if let Some(t) = &templates {
                        t.scan(&label, &data, d);
                    }
                }
            });
        }
        let (templates, tags) = (&templates, &tags);
        let hook = move |img: usize, path: &[u8], data: &[u8], d: &nxb_model::Digest| {
            if let Some(t) = templates {
                let label = format!("{}{}", tags[img], String::from_utf8_lossy(path));
                if data.len() >= 1 << 20 {
                    let _ = big_tx.send((label, data.to_vec(), *d));
                } else {
                    t.scan(&label, data, *d);
                }
            }
        };
        let hook_ref: &crate::write::FileHook<'_> = &hook;
        let r = write_images(
            images,
            req.threads,
            req.mem_limit,
            templates.as_ref().map(|_| hook_ref),
        );
        drop(hook);
        r
    })?;
    let t_images = t1.elapsed().as_secs_f64();
    if let (Some(t), Some((_, outs))) = (templates, &req.split) {
        let t2 = Instant::now();
        let mut vin = Vec::new();
        for (k, ((out, _), r)) in jobs.iter().zip(&results).enumerate() {
            let mainp = out.join("ExeFS/main");
            let main = std::fs::read(&mainp)
                .map_err(|e| ExtractError::io(format!("read {}", mainp.display()), e))?;
            let files = r
                .files_digests
                .iter()
                .map(|(p, _, _, d)| {
                    (
                        String::from_utf8_lossy(p)
                            .trim_start_matches('/')
                            .to_string(),
                        *d,
                    )
                })
                .collect();
            vin.push(crate::split::VersionIn {
                tag: tags[k].trim_start_matches('/').to_string(),
                main,
                files,
            });
        }
        let scanned = t.scanned();
        let (_, bad) = t.merge(&vin, outs)?;
        for b in bad {
            println!("{b}");
        }
        eprintln!(
            "nxb-extract: split fused, {scanned} distinct files scanned, merge {:.2} s",
            t2.elapsed().as_secs_f64()
        );
    }

    let mut stdout = std::io::stdout().lock();
    for ((out, _), (log, r)) in jobs.iter().zip(logs.iter().zip(&results)) {
        let _ = writeln!(stdout, "== {}", out.display());
        let _ = write!(stdout, "{log}");
        let _ = writeln!(
            stdout,
            "{}: {} files {:.1} MB, image with {} entries",
            r.dst.display(),
            r.files,
            r.bytes as f64 / 1_048_576.0,
            r.entries
        );
    }
    if let Some(p) = &req.digests {
        let mut t = String::from("# image\tpath\toffset\tsize\tsha256\n");
        for r in &results {
            let img = r.dst.display().to_string();
            t.push_str(&format!(
                "{img}\t\t0\t{}\t{}\tsha1={}\n",
                r.size,
                r.digest.file.sha256.to_hex(),
                r.digest.sha1.to_hex()
            ));
            for (path, off, size, d) in &r.files_digests {
                t.push_str(&format!(
                    "{img}\t{}\t{off}\t{size}\t{}\n",
                    String::from_utf8_lossy(path),
                    d.to_hex()
                ));
            }
        }
        write_file(p, t.as_bytes())?;
    }
    Ok(Summary {
        images: results,
        cache: cache.stats(),
        seconds: (t_small, t_images),
        closing,
    })
}

/// `nx_extract --contents`: one line per content meta, `content <type> <title ID> <version>`.
pub fn contents(keys: &Path, package: &Path) -> Result<String> {
    let text = std::fs::read_to_string(keys)
        .map_err(|e| ExtractError::io(format!("read {}", keys.display()), e))?;
    let mut ks = KeySet::parse(&text);
    let cache = Arc::new(BlockCache::new(64));
    let pkg: Arc<dyn Storage> = Arc::new(FileStorage::open(package)?);
    let ncas = open_nsp(&mut ks, pkg)?;
    let mut out = String::new();
    for c in crate::meta::metas(&ks, &ncas, &cache)? {
        let kind = match c.kind {
            0x80 => "Application".to_string(),
            0x81 => "Patch".to_string(),
            0x82 => "AddOnContent".to_string(),
            0x83 => "Delta".to_string(),
            x => format!("{x:#x}"),
        };
        out.push_str(&format!("content {kind} {:016x} {}\n", c.title, c.version));
    }
    for n in &ncas {
        out.push_str(&format!(
            "# nca {} {:?} {:016x}\n",
            n.id, n.content_type, n.title_id
        ));
    }
    Ok(out)
}
