//! `nx_extract --aoc`: the RomFS of an add-on content package.

use std::sync::Arc;
use std::time::Instant;

use crate::cache::BlockCache;
use crate::error::{ExtractError, Result};
use crate::image::ImagePlan;
use crate::keys::KeySet;
use crate::nca::open_package;
use crate::run::{Request, Summary};
use crate::storage::{FileStorage, Storage};
use crate::version::{base_data, open_romfs_with};
use crate::write::{ImageJob, write_images};

/// `nx_extract --aoc`: the RomFS of an add-on content package's data NCA (a PublicData or Data NCA, the one of title
/// `id` when given) as `<out_base>/RomFS.img`. `req.base` is the DLC package.`req.out_base` the output folder.
pub fn aoc(req: &Request, id: Option<u64>) -> Result<Summary> {
    use crate::nca::ContentType;
    let t0 = Instant::now();
    let text = std::fs::read_to_string(&req.keys)
        .map_err(|e| ExtractError::io(format!("read {}", req.keys.display()), e))?;
    let mut ks = KeySet::parse(&text);
    drop(text);
    let cache = Arc::new(BlockCache::new(req.cache_chunks));
    let file = req
        .base
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let pkg: Arc<dyn Storage> = Arc::new(FileStorage::open(&req.base)?);
    let ncas = open_package(&mut ks, pkg, &file)?;
    let data = crate::nca::pick(&ncas, ContentType::PublicData, id)
        .or_else(|| crate::nca::pick(&ncas, ContentType::Data, id))
        .ok_or_else(|| {
            ExtractError::format(match id {
                None => "no data NCA in the DLC NSP".to_string(),
                Some(i) => format!("no data NCA of add-on content {i:016x} in {file}"),
            })
        })?;
    let mut log = format!("add-on content {:016x}\n", data.title_id);
    let shared = base_data(&ks, data, &cache)?;
    let fs = open_romfs_with(&ks, shared, None, &cache, req.allow_damaged)?;
    let mut plan = ImagePlan::plan(&fs.entries)?;
    let mut names = Vec::new();
    if req.allow_damaged {
        fs.fs.scan()?;
        for e in plan.entries.iter_mut().filter(|e| !e.dir) {
            if fs.fs.damaged_in(e.src, e.b) {
                e.damaged = true;
                names.push(String::from_utf8_lossy(&e.path).into_owned());
            }
        }
    }
    let keys = (0..plan.entries.len() as u64).collect();
    let src: Arc<dyn Storage> = fs.fs.clone();
    let t1 = Instant::now();
    let (results, closing) = write_images(
        vec![ImageJob {
            dst: req.out_base.join("RomFS.img"),
            plan,
            src,
            keys,
            discard: req.discard_images,
        }],
        req.threads,
        req.mem_limit,
        None,
    )?;
    let r = &results[0];
    log.push_str(&format!(
        "{}: {} files {:.1} MB, image with {} entries\n",
        r.dst.display(),
        r.files,
        r.bytes as f64 / 1_048_576.0,
        r.entries
    ));
    print!("{log}");
    if !names.is_empty() {
        for p in &names {
            println!("[RED] hash check failed: {p}");
        }
        println!(
            "[RED] the NSP is damaged: {} files written as read (--allow-damaged)",
            names.len()
        );
    }
    Ok(Summary {
        images: results,
        cache: cache.stats(),
        seconds: (
            t1.duration_since(t0).as_secs_f64(),
            t1.elapsed().as_secs_f64(),
        ),
        closing,
    })
}
