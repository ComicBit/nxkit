//! The input of the build's Unreal decode (`decode_ue_shaders`): the paks and IoStore containers of a version's
//! `RomFS.img`, its stamp, and the job of their shader libraries and shader code archives
//! ([`nxb_split::jobs::uejob`]).

use std::os::unix::fs::FileExt;
use std::path::Path;
use std::sync::Arc;

use nxb_split::jobs::uejob::{UeJob, toc_has_shader_libraries, ue_job};
use nxb_split::src::Src;

use crate::error::{ExtractError, Result};
use crate::listing::{ImageFile, image_files};

/// The largest IoStore TOC read.
const TOC_MAX: u64 = 1 << 30;

fn containers(files: &[ImageFile]) -> Vec<&ImageFile> {
    let mut v: Vec<&ImageFile> = files
        .iter()
        .filter(|f| f.0.ends_with(".pak") || f.0.ends_with(".utoc"))
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

/// `ue_stamp`: what a version's decoded archives were made from, a line `<path>\t<size>` per pak and IoStore TOC of
/// its `RomFS.img` (`dir/RomFS.img`), in path order.
pub fn stamp(dir: &Path) -> Result<String> {
    let (_, files) = image_files(&dir.join("RomFS.img"))?;
    Ok(containers(&files)
        .iter()
        .map(|(p, _, n)| format!("{p}\t{n}\n"))
        .collect())
}

/// Whether a file is an input of the job: a pak, an IoStore TOC or partition, or a manifest at the top.
fn input(p: &str) -> bool {
    p.ends_with(".pak")
        || p.ends_with(".utoc")
        || p.ends_with(".ucas")
        || (!p.contains('/') && p.starts_with("Manifest_") && p.ends_with(".txt"))
}

/// Whether an IoStore TOC of the version folder `dir` (`RomFS.img`) holds Unreal Engine 5 shader libraries
/// (`io_shader_tocs`). Their decode needs the game's decoder.
pub fn has_shader_libraries(dir: &Path) -> Result<bool> {
    let (f, files) = image_files(&dir.join("RomFS.img"))?;
    for (p, off, size) in containers(&files) {
        if !p.ends_with(".utoc") || *size > TOC_MAX {
            continue;
        }
        let mut b = vec![0u8; usize::try_from(*size).unwrap_or(0)];
        f.read_exact_at(&mut b, *off)
            .map_err(|e| ExtractError::io(format!("read {p}"), e))?;
        if toc_has_shader_libraries(&b, p) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `uepak.job` over the version folder `dir` (`RomFS.img`, `ExeFS/main`): the shader libraries of its IoStore
/// containers and the shader code archives of its paks. The libraries (Unreal Engine 5.6) need the game's decoder
/// for two layers (the container blocks, then each shader group): the caller installs it
/// ([`crate::oodle_host::Installed`]).
pub fn job(dir: &Path) -> Result<UeJob> {
    let (f, files) = image_files(&dir.join("RomFS.img"))?;
    let f = Arc::new(f);
    let mut v: Vec<&ImageFile> = files.iter().filter(|x| input(&x.0)).collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    let inputs: Vec<(String, Arc<dyn Src>)> = v
        .into_iter()
        .map(|(p, off, size)| {
            let s: Arc<dyn Src> = Arc::new(crate::post::ImgSrc {
                file: f.clone(),
                off: *off,
                size: *size,
            });
            (p.clone(), s)
        })
        .collect();
    let mp = dir.join("ExeFS/main");
    let main =
        std::fs::read(&mp).map_err(|e| ExtractError::io(format!("read {}", mp.display()), e))?;
    ue_job(&inputs, &main).map_err(|e| ExtractError::format(e.to_string()))
}
