//! Dump folders (`scripts/pack_dump.py`): a game's files already extracted, with no `prod.keys`:
//!
//! - `base/exefs/` (`main`, `main.npdm`, ...) and `base/romfs/` (optional when an update has its RomFS),
//! - `updates/<version>/exefs/` and `updates/<version>/romfs/` (the patched RomFS, packed on its own),
//! - `dlc/<DLC title ID>/<n>/romfs/` (or `dlc/<DLC title ID>/romfs/`).
//!
//! One version's `ExeFS/` and `RomFS.img` and every DLC's `aoc/<index>.img` are written, in the layout of the NSP
//! extraction, with the same `DMROMFS1` image as `romfs_image.py` (entries breadth-first, children sorted by name bytes).
//! A dump has no control data. The files are read in place (a virtual storage over the folder's files), so no file is
//! copied twice.

use std::io::Write as _;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::error::{ExtractError, Result};
use crate::image::ImagePlan;
use crate::nup::program_id;
use crate::romfs::RomEntry;
use crate::storage::Storage;
use crate::write::{ImageJob, ImageResult, digests_text, write_images_counted};

/// What to pack.
#[derive(Clone, Debug)]
pub struct DumpRequest {
    /// The dump folder.
    pub dump: PathBuf,
    /// The version to pack (none: the newest in `updates/`, else the base game, 0 is the base game).
    pub version: Option<u32>,
    /// The build folder (empty: `<dump>/<title ID>_build`).
    pub work: PathBuf,
    /// Image writer threads and memory.
    pub threads: usize,
    /// Bytes of file data in flight at most.
    pub mem_limit: u64,
    /// Digests only: no image is written.
    pub discard_images: bool,
    /// Optional TSV of the images' digests.
    pub digests: Option<PathBuf>,
    /// Where to count the image bytes written, for a progress display.
    pub progress: Option<Arc<crate::run::Counter>>,
}

fn bad(m: impl Into<String>) -> ExtractError {
    ExtractError::format(m)
}

/// Whether a folder has the layout above (`is_dump` of `pack_dump.py`).
pub fn is_dump(folder: &Path) -> bool {
    folder.join("base/exefs/main.npdm").is_file()
        && (folder.join("base/romfs").is_dir()
            || std::fs::read_dir(folder.join("updates"))
                .into_iter()
                .flatten()
                .flatten()
                .any(|e| e.path().join("romfs").is_dir()))
}

fn dirs(path: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    v.sort();
    v
}

/// What a dump holds: the title ID, the update versions with their folders, the DLC title IDs with their RomFS folders.
pub struct Scan {
    /// The game's title ID (the program ID in `base/exefs/main.npdm`).
    pub app: u64,
    /// `updates/<version>/`.
    pub updates: std::collections::BTreeMap<u32, PathBuf>,
    /// `dlc/<title ID>/.../romfs`.
    pub dlc: std::collections::BTreeMap<u64, PathBuf>,
}

/// Reads the folder's layout.
pub fn scan(dump: &Path) -> Result<Scan> {
    if !is_dump(dump) {
        return Err(bad(format!(
            "{}: no base/exefs/main.npdm, or no RomFS in base/ or updates/<version>/",
            dump.display()
        )));
    }
    let npdm = dump.join("base/exefs/main.npdm");
    let app = program_id(&read(&npdm)?, &npdm.display().to_string())?;
    if app & 0xFFF != 0 {
        return Err(bad(format!(
            "{}: base program ID {app:016X} is not a base title ID",
            dump.display()
        )));
    }
    let mut updates = std::collections::BTreeMap::new();
    for p in dirs(&dump.join("updates")) {
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned());
        if let Some(v) = name
            .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|n| n.parse::<u32>().ok())
        {
            updates.insert(v, p);
        }
    }
    let mut dlc = std::collections::BTreeMap::new();
    for d in dirs(&dump.join("dlc")) {
        let name = d
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let tid = u64::from_str_radix(&name, 16)
            .map_err(|_| bad(format!("{}: not a DLC title ID", d.display())))?;
        let index = tid.wrapping_sub(app).wrapping_sub(0x1000);
        if tid <= app || !(1..=2000).contains(&index) {
            return Err(bad(format!(
                "{}: {tid:016X} is not a DLC of {app:016X}",
                d.display()
            )));
        }
        let mut roms: Vec<PathBuf> = dirs(&d)
            .into_iter()
            .map(|p| p.join("romfs"))
            .filter(|p| p.exists())
            .collect();
        if roms.is_empty() && d.join("romfs").is_dir() {
            roms.push(d.join("romfs"));
        }
        if roms.len() != 1 {
            return Err(bad(format!(
                "{}: expected one <n>/romfs folder, found {}",
                d.display(),
                roms.len()
            )));
        }
        dlc.insert(tid, roms.remove(0));
    }
    Ok(Scan { app, updates, dlc })
}

fn read(p: &Path) -> Result<Vec<u8>> {
    std::fs::read(p).map_err(|e| ExtractError::io(format!("read {}", p.display()), e))
}

/// The files of a folder as one address space: file `k` lies at `starts[k]`, one after another.
struct DirStorage {
    files: Vec<(PathBuf, u64, u64)>, // path, start, size
    size: u64,
}

impl Storage for DirStorage {
    fn size(&self) -> u64 {
        self.size
    }
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<()> {
        let end = offset
            .checked_add(buf.len() as u64)
            .filter(|&e| e <= self.size)
            .ok_or_else(|| bad("read past the end of the dump"))?;
        let mut at = offset;
        while at < end {
            let k = self.files.partition_point(|f| f.1 <= at).saturating_sub(1);
            let (path, start, size) = &self.files[k];
            let n = (end.min(start + size) - at) as usize;
            if n == 0 {
                return Err(bad("read between files of the dump"));
            }
            let f = std::fs::File::open(path)
                .map_err(|e| ExtractError::io(format!("open {}", path.display()), e))?;
            let dst = &mut buf[(at - offset) as usize..(at - offset) as usize + n];
            f.read_exact_at(dst, at - start).map_err(|e| {
                ExtractError::io(
                    format!(
                        "read {} (it changed while packing, or is shorter than when listed)",
                        path.display()
                    ),
                    e,
                )
            })?;
            at += n as u64;
        }
        Ok(())
    }
}

/// Lists a RomFS folder: entries as the image plan wants them, and the storage that reads their bytes.
fn tree(root: &Path) -> Result<(Vec<RomEntry>, Arc<dyn Storage>)> {
    let mut entries = Vec::new();
    let mut files = Vec::new();
    let mut pos = 0u64;
    let mut stack = vec![(root.to_path_buf(), Vec::<u8>::new(), 0usize)];
    while let Some((dir, prefix, depth)) = stack.pop() {
        if depth > 128 {
            return Err(bad(format!(
                "{}: folders nested deeper than 128 levels (a link loop?)",
                root.display()
            )));
        }
        let rd = std::fs::read_dir(&dir)
            .map_err(|e| ExtractError::io(format!("read {}", dir.display()), e))?;
        for e in rd {
            let e = e.map_err(|e| ExtractError::io(format!("read {}", dir.display()), e))?;
            let name = std::os::unix::ffi::OsStrExt::as_bytes(e.file_name().as_os_str()).to_vec();
            if name.starts_with(b".dm-case-") {
                return Err(ExtractError::Unsupported(
                    "names chosen on a case collision (.dm-case-N, their guest name is in an xattr)"
                        .into(),
                ));
            }
            let mut path = prefix.clone();
            path.push(b'/');
            path.extend_from_slice(&name);
            let p = e.path();
            let md = std::fs::metadata(&p)
                .map_err(|e| ExtractError::io(format!("stat {}", p.display()), e))?;
            if md.is_dir() {
                entries.push(RomEntry {
                    path: path.clone(),
                    dir: true,
                    size: 0,
                    offset: 0,
                });
                stack.push((p, path, depth + 1));
            } else {
                entries.push(RomEntry {
                    path,
                    dir: false,
                    size: md.len(),
                    offset: pos,
                });
                files.push((p, pos, md.len()));
                pos = pos
                    .checked_add(md.len())
                    .ok_or_else(|| bad("the folder's files are over 2^64 bytes"))?;
            }
        }
    }
    Ok((entries, Arc::new(DirStorage { files, size: pos })))
}

fn name_of(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// `exefs()` of `pack_dump.py`: the regular files of `src` that do not start with `.`, copied to `dst`.
fn exefs(src: &Path, dst: &Path) -> Result<()> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(src)
        .map_err(|e| ExtractError::io(format!("read {}", src.display()), e))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && !name_of(p).starts_with('.'))
        .collect();
    files.sort();
    let tmp = dst.with_file_name(format!("{}.partial", name_of(dst)));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)
        .map_err(|e| ExtractError::io(format!("create {}", tmp.display()), e))?;
    for f in &files {
        std::fs::copy(f, tmp.join(name_of(f)))
            .map_err(|e| ExtractError::io(format!("copy {}", f.display()), e))?;
    }
    let _ = std::fs::remove_dir_all(dst);
    std::fs::rename(&tmp, dst).map_err(|e| ExtractError::io(format!("rename {}", tmp.display()), e))
}

fn job(src: &Path, dst: PathBuf, discard: bool) -> Result<ImageJob> {
    let (entries, storage) = tree(src)?;
    let plan = ImagePlan::plan(&entries)?;
    let keys = (0..plan.entries.len() as u64).collect();
    Ok(ImageJob {
        dst,
        plan,
        src: storage,
        keys,
        discard,
    })
}

/// Packs one version (and every DLC) of a dump. Prints the lines of `pack_dump.py`, returns the image results.
pub fn extract_dump(r: &DumpRequest) -> Result<Vec<ImageResult>> {
    let dump = r.dump.canonicalize().unwrap_or_else(|_| r.dump.clone());
    let s = scan(&dump)?;
    let ver = r
        .version
        .unwrap_or_else(|| s.updates.keys().next_back().copied().unwrap_or(0));
    if ver != 0 && !s.updates.contains_key(&ver) {
        let have: Vec<String> = s.updates.keys().map(u32::to_string).collect();
        return Err(bad(format!(
            "no updates/{ver} (have: {})",
            if have.is_empty() {
                "none".into()
            } else {
                have.join(", ")
            }
        )));
    }
    let src = s.updates.get(&ver).cloned().unwrap_or(dump.join("base"));
    let work = if r.work.as_os_str().is_empty() {
        dump.join(format!("{:016X}_build", s.app))
    } else {
        r.work.clone()
    };
    let out = work.join("extract").join(format!("v{ver}"));
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout, "{:016X} version {ver}: {}", s.app, src.display());
    if !src.join("exefs/main").is_file()
        || !src.join("exefs/main.npdm").is_file()
        || !src.join("romfs").is_dir()
    {
        return Err(bad(format!(
            "{}: no exefs/main, exefs/main.npdm and romfs",
            src.display()
        )));
    }
    let npdm = src.join("exefs/main.npdm");
    let pid = program_id(&read(&npdm)?, &npdm.display().to_string())?;
    if pid != s.app {
        return Err(bad(format!(
            "{}: program ID {pid:016X}, the base is {:016X}",
            src.display(),
            s.app
        )));
    }
    std::fs::create_dir_all(&out)
        .map_err(|e| ExtractError::io(format!("create {}", out.display()), e))?;
    exefs(&src.join("exefs"), &out.join("ExeFS"))?;
    let mut jobs = vec![(
        src.join("romfs"),
        job(&src.join("romfs"), out.join("RomFS.img"), r.discard_images)?,
    )];
    let aoc = work.join("aoc");
    if !s.dlc.is_empty() || aoc.is_dir() {
        let want: Vec<String> = s
            .dlc
            .keys()
            .map(|t| format!("{}.img", t - s.app - 0x1000))
            .collect();
        if let Ok(rd) = std::fs::read_dir(&aoc) {
            for e in rd.flatten() {
                let n = name_of(&e.path());
                let stem = n.strip_suffix(".inputs.json").unwrap_or(&n);
                if !want.iter().any(|w| w == stem) && e.path().is_file() {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
    }
    if !s.dlc.is_empty() {
        std::fs::create_dir_all(&aoc)
            .map_err(|e| ExtractError::io(format!("create {}", aoc.display()), e))?;
    }
    for (tid, roms) in &s.dlc {
        let dst = aoc.join(format!("{}.img", tid - s.app - 0x1000));
        jobs.push((roms.clone(), job(roms, dst, r.discard_images)?));
    }
    for (from, j) in &jobs {
        let _ = writeln!(
            stdout,
            "packing {} \u{2192} {}",
            from.display(),
            j.dst.display()
        );
    }
    let (srcs, jobs): (Vec<PathBuf>, Vec<ImageJob>) = jobs.into_iter().unzip();
    let _ = srcs;
    let (results, closing) =
        write_images_counted(jobs, r.threads, r.mem_limit, r.progress.as_deref())?;
    closing.wait();
    for res in &results {
        let _ = writeln!(
            stdout,
            "  {} entries, {:.2} GB",
            res.entries,
            res.size as f64 / (1u64 << 30) as f64
        );
    }
    if let Some(p) = &r.digests {
        std::fs::write(p, digests_text(&results))
            .map_err(|e| ExtractError::io(format!("write {}", p.display()), e))?;
    }
    let _ = writeln!(
        stdout,
        "done: {}{}",
        out.display(),
        if s.dlc.is_empty() {
            String::new()
        } else {
            format!(", {} DLC in {}", s.dlc.len(), aoc.display())
        }
    );
    Ok(results)
}
