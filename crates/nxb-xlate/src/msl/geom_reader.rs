//! The vertex shader `_bgv` that draws the vertices a geometry kernel wrote.

use super::emit::{Options, R};
use super::geom::{Layout, output_layout};
use super::library;
use super::refl::Raster;
use super::{Context, Library};
use crate::job::Job;
use crate::translation::msl_entry::structure_geometry_reader_job;
use nxb_pyjson::Value;

/// The library `_bgv`. It reads its vertex from `vb_input` and writes every output the geometry shader writes.
/// The entry `main0_z` keeps the depth range of the source for the host's zero-to-one clip mode.
pub fn library(job: &Job, code: &[u8], ctx: &Context<'_>, opt: &Options) -> R<Library> {
    let layout: Layout = output_layout(code, 0)?;
    let mut s = structure_geometry_reader_job(job, code)?;
    s.job.key = job.key.clone();
    s.notes.clear();
    let mut done = library::build(&s, ctx, opt)?;
    if done.libraries.is_empty() {
        return Err("geometry reader without a library".into());
    }
    let mut lib = done.libraries.swap_remove(0);
    let mut r = lib
        .reflection
        .take()
        .ok_or("geometry reader without reflection")?;
    r.key = format!("{}_bgv", job.key);
    r.c1 = String::new();
    r.drv_reads = Vec::new();
    r.drv_unfillable = Vec::new();
    r.vertex_inputs = Vec::new();
    r.vs_pad = Vec::new();
    r.raster = Raster::default();
    // The reader draws what the kernel wrote. It has the depth entry and nothing that belongs to a captured draw.
    r.entries.retain(|(k, _)| k == "entry_z");
    r.extra.clear();
    r.set_extra("geometry_reader", Value::Bool(true));
    r.set_extra("geometry_output", Value::Int(i64::from(layout.output)));
    lib.suffix = "_bgv".into();
    lib.reflection = Some(r);
    Ok(lib)
}
