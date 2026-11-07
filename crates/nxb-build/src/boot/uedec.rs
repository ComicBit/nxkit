//! `decode_ue_shaders`: a game with Unreal paks keeps its shaders in the shader code archives of its paks, each shader
//! compressed. LZ4 ones (Unreal Engine 4.26 and later by default) are decoded in nxb. Oodle ones (Octopath Traveler
//! 0) only the game's own Oodle decodes: nxb writes one job of them, the game's code decodes it in the Mac host
//! (`DM_OODLE_JOB`, `dm/host/hle_oodle.c`: right before nnMain, then the host exits) and the decoded archives go to
//! `<version dir>/ue_shaders`, where the split reads them. A folder whose stamp matches its paks is reused.

use std::path::Path;
use std::time::Instant;

use super::{Ctx, boot_mac, hms, rmtree, set_env, swap_in, tail_lines};
use crate::events::Update;

/// `UE_DECODE_SECS`: the longest Oodle decode run (Octopath Traveler 0's 10,596 shaders take about 10 s, the boot up
/// to nnMain included).
const UE_DECODE_SECS: f64 = 300.0;

fn reused(d: &Path) -> Result<bool, String> {
    let have = std::fs::read_to_string(d.join("ue_shaders/stamp")).ok();
    let now = nxb_extract::ue_decode::stamp(d).map_err(|e| e.to_string())?;
    Ok(have.as_deref() == Some(now.as_str()))
}

/// The Unreal decode of the split's versions (`<work>/extract/<version>`). The job runs the packed version's code
/// (Oodle's format does not change between builds).
pub(crate) fn decode_ue_shaders(c: &Ctx<'_>, versions: &[String]) -> Result<(), String> {
    let dirs: Vec<(String, std::path::PathBuf)> = versions
        .iter()
        .map(|v| (v.clone(), c.work.join("extract").join(v)))
        .collect();
    let mut todo = Vec::new();
    for (v, d) in &dirs {
        if !reused(d)? {
            todo.push((v, d));
        }
    }
    if todo.is_empty() {
        c.p.say(&format!(
            "reusing the decoded shader archives: {}",
            dirs.iter()
                .map(|(_, d)| d.join("ue_shaders").display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        return Ok(());
    }
    let t0 = Instant::now();
    let mut total = 0usize;
    let mut host = None;
    for (v, d) in todo {
        let jd = c.work.join("tmp").join(format!("ue_job_{v}"));
        rmtree(&jd)?;
        std::fs::create_dir_all(&jd).map_err(|e| format!("{}: {e}", jd.display()))?;
        c.p.set(Update {
            step: Some(&format!("{v}: shader archives out of the paks")),
            ..Update::default()
        });
        let job = nxb_extract::ue_decode::job(d).map_err(|e| e.to_string())?;
        if job.archives.is_empty() {
            return Err(format!(
                "the RomFS has Unreal paks, but none in {} holds a shader code archive: where the shaders are has to be found first",
                d.join("RomFS.img").display()
            ));
        }
        let n = job.records();
        c.p.say(&format!(
            "{v}: {}{}",
            job.archives
                .iter()
                .map(|x| {
                    let how = if x.compression.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", x.compression)
                    };
                    format!("{} {} shaders{how}", x.name, x.shaders)
                })
                .collect::<Vec<_>>()
                .join("; "),
            if n > 0 {
                format!("; decoding {n} with the game's Oodle in the Mac host")
            } else {
                String::new()
            }
        ));
        let out = if n > 0 {
            if host.is_none() {
                c.p.set(Update {
                    step: Some("building the Mac host"),
                    ..Update::default()
                });
                host = Some(c.host()?);
            }
            let jf = jd.join("oodle.job");
            std::fs::write(&jf, job.job_file()).map_err(|e| format!("{}: {e}", jf.display()))?;
            let (rd, mut env) = c.run_dir("ue_run", true)?;
            set_env(&mut env, "DM_OODLE_JOB", &jf);
            let log = c.logs().join(format!("ue_decode_{v}.log"));
            c.p.set(Update {
                step: Some(&format!("{v}: decoding {n} shaders in the Mac host")),
                ..Update::default()
            });
            let ran = boot_mac(
                host.as_deref().unwrap_or(Path::new("")),
                &c.patched(),
                &env,
                &rd,
                &log,
                UE_DECODE_SECS,
                &mut || false,
            )?;
            let of = jd.join("oodle.job.out");
            let rc = ran
                .rc
                .map_or("none (time limit)".to_string(), |r| r.to_string());
            if ran.rc != Some(0) || !of.exists() {
                c.p.say(&format!(
                    "---- last lines of {} ----\n{}",
                    log.display(),
                    tail_lines(&log, 20)
                ));
                return Err(format!(
                    "the Oodle decode run stopped with exit {rc} (host output: {})",
                    log.display()
                ));
            }
            rmtree(&rd)?;
            Some(std::fs::read(&of).map_err(|e| format!("{}: {e}", of.display()))?)
        } else {
            None
        };
        let arcs = job.finish(out.as_deref()).map_err(|e| e.to_string())?;
        let part = d.join("ue_shaders.partial");
        rmtree(&part)?;
        std::fs::create_dir_all(&part).map_err(|e| format!("{}: {e}", part.display()))?;
        for (name, b) in &arcs {
            let p = part.join(name);
            std::fs::write(&p, b).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        let stamp = nxb_extract::ue_decode::stamp(d).map_err(|e| e.to_string())?;
        std::fs::write(part.join("stamp"), stamp)
            .map_err(|e| format!("{}: {e}", part.display()))?;
        swap_in(&part, &d.join("ue_shaders"))?;
        rmtree(&jd)?;
        total += n;
    }
    c.p.say(&format!(
        "UE shader decode: {total} shaders in {}",
        hms(t0.elapsed().as_secs_f64())
    ));
    Ok(())
}
