//! `Main` of `dmsh_xlate`: translate each job of a job file, with the retries, and write the outputs.

use crate::accessor::{Accessor, Binding, image_types_of};
use crate::job::{Job, parse_jobs};
use crate::lowering;
use crate::res::{CBuf, Res, SBuf, Tex};
use crate::types::SamplerType;
use std::collections::BTreeMap;
use std::path::Path;

/// A buffer binding of the program info.
#[derive(Clone, Debug, Default)]
pub struct BufferInfo {
    pub binding: i64,
    pub slot: i64,
    pub cb_slot: i64,
    pub cb_offset: i64,
    pub flags: i64,
}

/// A texture or image of the program info.
#[derive(Clone, Debug)]
pub struct TexInfo {
    pub binding: i64,
    pub ty: u32,
    pub format: usize,
    pub cbuf_slot: i64,
    pub handle: i64,
    pub array_length: i64,
    pub separate: bool,
    pub flags: i64,
}

/// What a translation returns (`ShaderProgram`).
#[derive(Clone, Debug, Default)]
pub struct Program {
    pub spirv: Vec<u8>,
    pub cbufs: Vec<BufferInfo>,
    pub sbufs: Vec<BufferInfo>,
    pub textures: Vec<TexInfo>,
    pub images: Vec<TexInfo>,
    pub uses_frag_coord: bool,
    pub frag_out_map: i64,
    /// `GetResourceReservations().OutputSizePerInvocation` (geometry)
    pub geometry_stride: i64,
}

/// The translation core: decoders, IR, emitters, SPIR-V. One call is one `Translator.CreateContext(...).Translate`.
pub trait Core {
    /// Translate `code` for the job with the accessor's answers.
    fn translate(&self, code: &[u8], acc: &mut Accessor, geometry: bool)
    -> Result<Program, String>;
    /// `GenerateVertexPassthroughForCompute` of a fresh context: the geometry reader SPIR-V.
    fn passthrough_reader(&self, code: &[u8], acc: &mut Accessor) -> Result<Vec<u8>, String>;
}

fn bindings(l: &[TexInfo]) -> Vec<Binding> {
    l.iter()
        .map(|t| Binding {
            ty: crate::types::SamplerType(t.ty),
            cbuf_slot: t.cbuf_slot as i32,
            handle: t.handle as i32,
            array_length: t.array_length as i32,
        })
        .collect()
}

fn tex(l: &[TexInfo]) -> Vec<Tex> {
    l.iter()
        .map(|t| Tex {
            binding: t.binding,
            ty: t.ty as i64,
            format: crate::types::TEXTURE_FORMATS[t.format].to_string(),
            cbuf_slot: t.cbuf_slot,
            handle: t.handle,
            array: t.array_length,
            separate: t.separate,
            flags: t.flags,
        })
        .collect()
}

/// The outputs of one job.
pub struct Outputs {
    pub spv: Vec<u8>,
    pub reader: Option<Vec<u8>>,
    pub res: Res,
}

/// The result of the retry loop of one job.
pub struct Retried<T> {
    /// The job that produced the result (`softfloat64` set after the FP64 retry).
    pub job: Job,
    /// The accessor's notes.
    pub notes: Vec<String>,
    /// The texel fetches of 1D textures the accessor recorded.
    pub fetch1d: std::collections::BTreeSet<i32>,
    /// What the step returned.
    pub value: T,
    /// The program info of the step.
    pub program: Program,
    /// The guessed image types of the second pass.
    pub image_types: Option<BTreeMap<i32, SamplerType>>,
    /// The guessed texture types of the second pass.
    pub texture_types: Option<BTreeMap<i32, SamplerType>>,
}

/// One translation of a job: the value, its program info and whether it uses 64 bit floats.
pub type Step<'a, T> = dyn Fn(&Job, &mut Accessor) -> Result<(T, Program, bool), String> + 'a;

/// The retry loop of `Main`: translate, translate again with the guessed image and texture types, and translate
/// once more with `softfloat64` when the result still uses 64 bit floats.
pub fn retry<T>(job: &Job, step: &Step<'_, T>) -> Result<Retried<T>, String> {
    let mut acc = Accessor::new(job);
    let (mut v, mut p, mut fp64) = step(job, &mut acc)?;
    let mut image_types = None;
    let mut texture_types = None;
    if acc.image_query_guessed {
        let t = image_types_of(&bindings(&p.images));
        if !t.is_empty() {
            image_types = Some(t);
        }
    }
    let tt = acc.texture_types_of(&bindings(&p.textures));
    if !tt.is_empty() {
        texture_types = Some(tt);
    }
    if image_types.is_some() || texture_types.is_some() {
        acc = Accessor::with_types(job, image_types.clone(), texture_types.clone());
        (v, p, fp64) = step(job, &mut acc)?;
    }
    if fp64 {
        let mut j2 = job.clone();
        j2.softfloat64 = true;
        let mut acc2 = Accessor::with_types(&j2, image_types.clone(), texture_types.clone());
        let (v2, p2, _) = step(&j2, &mut acc2)?;
        acc2.notes
            .push("FP64 lowered to FP32 (DoubleToFloat)".into());
        let (notes, fetch1d) = (std::mem::take(&mut acc2.notes), acc2.fetch1d.clone());
        drop(acc2);
        return Ok(Retried {
            job: j2,
            notes,
            fetch1d,
            value: v2,
            program: p2,
            image_types,
            texture_types,
        });
    }
    Ok(Retried {
        job: job.clone(),
        notes: acc.notes,
        fetch1d: acc.fetch1d,
        value: v,
        program: p,
        image_types,
        texture_types,
    })
}

/// Translate one job (the body of the `try` in `Main`).
pub fn run_job(core: &dyn Core, job: &Job, code: &[u8]) -> Result<Outputs, String> {
    let geometry = job.geometry_topology.is_some();
    let r = retry(job, &|_: &Job, acc: &mut Accessor| {
        let p = core.translate(code, acc, geometry)?;
        let fp64 = lowering::uses_float64(&p.spirv);
        Ok(((), p, fp64))
    })?;
    let mut acc = Accessor::new(&r.job);
    acc.notes = r.notes;
    acc.fetch1d = r.fetch1d;
    finish(
        core,
        &r.job,
        code,
        acc,
        r.program,
        geometry,
        r.image_types,
        r.texture_types,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish(
    core: &dyn Core,
    job: &Job,
    code: &[u8],
    mut acc: Accessor,
    p: Program,
    geometry: bool,
    image_types: Option<std::collections::BTreeMap<i32, crate::types::SamplerType>>,
    texture_types: Option<std::collections::BTreeMap<i32, crate::types::SamplerType>>,
) -> Result<Outputs, String> {
    let reader = if geometry {
        let mut fresh = Accessor::with_types(job, image_types, texture_types);
        Some(core.passthrough_reader(code, &mut fresh)?)
    } else {
        None
    };
    let spv = lowering::apply(code, &job.stage, &p.spirv, &mut acc.notes)?;
    let res = Res {
        key: job.key.clone(),
        stage: job.stage.clone(),
        notes: acc.notes.clone(),
        cbufs: p
            .cbufs
            .iter()
            .map(|b| CBuf {
                binding: b.binding,
                slot: b.slot,
            })
            .collect(),
        sbufs: p
            .sbufs
            .iter()
            .map(|b| SBuf {
                binding: b.binding,
                slot: b.slot,
                cb_slot: b.cb_slot,
                cb_offset: b.cb_offset,
                flags: b.flags,
            })
            .collect(),
        textures: tex(&p.textures),
        images: tex(&p.images),
        geometry_stride: if geometry { p.geometry_stride } else { 0 },
        uses_frag_coord: p.uses_frag_coord,
        frag_out_map: p.frag_out_map,
        fetch1d: acc.fetch1d.iter().map(|&h| h as i64).collect(),
    };
    Ok(Outputs { spv, reader, res })
}

/// `dmsh_xlate <jobs.json> <outdir>`: the exit code (0, or 1 when a job failed). Red lines go to stderr.
pub fn main_jobs(core: &dyn Core, jobs_path: &Path, out_dir: &Path) -> Result<i32, String> {
    let text =
        std::fs::read_to_string(jobs_path).map_err(|e| format!("{}: {e}", jobs_path.display()))?;
    let jobs = parse_jobs(&text)?;
    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let (mut ok, mut bad) = (0, 0);
    for job in &jobs {
        let r = std::fs::read(&job.code_path)
            .map_err(|e| e.to_string())
            .and_then(|code| run_job(core, job, &code));
        match r {
            Ok(o) => {
                if let Some(rd) = &o.reader {
                    std::fs::write(out_dir.join(format!("{}.reader.spv", job.key)), rd)
                        .map_err(|e| e.to_string())?;
                }
                std::fs::write(out_dir.join(format!("{}.spv", job.key)), &o.spv)
                    .map_err(|e| e.to_string())?;
                std::fs::write(
                    out_dir.join(format!("{}.res.json", job.key)),
                    o.res.to_text(),
                )
                .map_err(|e| e.to_string())?;
                std::fs::write(
                    out_dir.join(format!("{}.iface.json", job.key)),
                    crate::iface::ShaderInterface::build(job, &o.res).to_text(),
                )
                .map_err(|e| e.to_string())?;
                ok += 1;
            }
            Err(e) => {
                bad += 1;
                eprintln!("[RED] {}: {}", job.key, e.lines().next().unwrap_or(""));
            }
        }
    }
    println!("translated: {ok} ok, {bad} failed");
    Ok(if bad == 0 { 0 } else { 1 })
}

/// A core that translates nothing yet: every job is red with a stable message.
pub struct Unported;

impl Core for Unported {
    fn translate(&self, _: &[u8], _: &mut Accessor, _: bool) -> Result<Program, String> {
        Err("NotImplemented: translation core is not ported".into())
    }
    fn passthrough_reader(&self, _: &[u8], _: &mut Accessor) -> Result<Vec<u8>, String> {
        Err("NotImplemented: translation core is not ported".into())
    }
}
