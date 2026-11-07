//! The input of the build's Unreal decode (`decode_ue_shaders`): the paks and IoStore TOCs of a version's
//! `RomFS.img`, its stamp, and the job of their shader code archives ([`nxb_split::jobs::uejob`]).

use std::os::unix::fs::FileExt;
use std::path::Path;
use std::sync::Arc;

use nxb_split::jobs::uejob::{UeJob, toc_has_shader_libraries, ue_job};
use nxb_split::src::Src;

use crate::error::{ExtractError, Result};
use crate::listing::{ImageFile, image_files};

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

/// `uepak.job` over the version folder `dir` (`RomFS.img`, `ExeFS/main`): the shader code archives of its paks.
/// IoStore shader libraries (Unreal Engine 5.6) need the game's Oodle over a pipe for two layers (the container
/// blocks, then each shader group): that reader is not ported, an error names the container.
pub fn job(dir: &Path) -> Result<UeJob> {
    let (f, files) = image_files(&dir.join("RomFS.img"))?;
    let f = Arc::new(f);
    let mut paks: Vec<(String, Arc<dyn Src>)> = Vec::new();
    for (p, off, size) in containers(&files) {
        if p.ends_with(".utoc") {
            let mut b = vec![0u8; usize::try_from(*size).unwrap_or(0)];
            f.read_exact_at(&mut b, *off)
                .map_err(|e| ExtractError::io(format!("read {p}"), e))?;
            if toc_has_shader_libraries(&b, p) {
                return Err(ExtractError::format(format!(
                    "{p}: IoStore shader libraries (Unreal Engine 5.6) need the game's decoder for the container blocks and the shader groups (uepak.py OodleHost); that reader is not ported to nxb"
                )));
            }
            continue;
        }
        paks.push((
            p.clone(),
            Arc::new(crate::post::ImgSrc {
                file: f.clone(),
                off: *off,
                size: *size,
            }),
        ));
    }
    let mp = dir.join("ExeFS/main");
    let main =
        std::fs::read(&mp).map_err(|e| ExtractError::io(format!("read {}", mp.display()), e))?;
    ue_job(&paks, &main).map_err(|e| ExtractError::format(e.to_string()))
}
