//! The entry of the MSL back end into the translation: a job and its machine code to the structured program, with the
//! retry loop of the driver (guessed texture and image types, 64 bit float lowering).

use super::context::TranslatorContext;
use super::core::structure_context;
use super::pipeline::options_of;
use crate::codegen::props::Props;
use crate::driver::{Program, retry};
use crate::job::Job;
use crate::msl::walk::uses_float64;
use crate::structured::StructInfo;
use std::collections::BTreeSet;

/// The result of the front end for the MSL emitter.
pub struct Structured {
    /// The job that produced it (`softfloat64` set after the float lowering retry).
    pub job: Job,
    /// The structured program.
    pub info: StructInfo,
    /// The definitions, host capabilities and resource properties.
    pub props: Props,
    /// The resource lists.
    pub program: Program,
    /// The accessor's notes.
    pub notes: Vec<String>,
    /// The texture slots read by a 1D texel fetch that became a wrapped 2D read.
    pub fetch1d: BTreeSet<i32>,
}

/// Run the front end of one job to the structured program.
pub fn structure_job(job: &Job, code: &[u8]) -> Result<Structured, String> {
    if job.geometry_topology.is_some() {
        return Err("unported: geometry translation".into());
    }
    let r = retry(job, &|j: &Job, acc| {
        let tc = TranslatorContext::create(code, acc, options_of(j))?;
        let debug = tc.options.debug_mode;
        let (info, props, program) = structure_context(&tc, acc, debug)?;
        let fp64 = uses_float64(&info);
        Ok(((info, props), program, fp64))
    })?;
    let (info, props) = r.value;
    Ok(Structured {
        job: r.job,
        info,
        props,
        program: r.program,
        notes: r.notes,
        fetch1d: r.fetch1d,
    })
}
