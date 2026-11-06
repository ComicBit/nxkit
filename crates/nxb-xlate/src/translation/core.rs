//! The translation core: `Translator.CreateContext(...).Translate()` for the Maxwell path, from machine code to SPIR-V,
//! and the typed program interface (descriptors) the driver turns into `res.json`.

use super::context::TranslatorContext;
use super::pipeline::{options_of, run_to_xform};
use super::props as tp;
use super::resources::ResourceManager;
use crate::accessor::Accessor;
use crate::attr::{Stage, feature};
use crate::codegen::generator::generate;
use crate::codegen::props::{self as cp, Props};
use crate::driver::{BufferInfo, Core, Program, TexInfo};
use crate::ir::Ir;
use crate::job::Job;
use crate::structured::StructResources;
use crate::structured::program::{FunctionIn, make_structured_program};
use crate::structured::types::AggType;
use crate::types::AttributeType;
use AggType as Agg;

/// The resource manager seen from the structured program (`GetConstantBufferBinding` queries the accessor).
pub(super) struct StructRes<'a, 'b> {
    pub(super) rm: &'a mut ResourceManager,
    pub(super) acc: &'a mut Accessor<'b>,
}

impl StructResources for StructRes<'_, '_> {
    fn constant_buffer_binding(&mut self, slot: i32) -> Result<i32, String> {
        self.rm.get_constant_buffer_binding(self.acc, slot)
    }

    fn set_used_constant_buffer_binding(&mut self, binding: i32) {
        self.rm.set_used_constant_buffer_binding(binding);
    }
}

fn agg(a: tp::AggregateType) -> Agg {
    AggType(a.0)
}

fn attr_num(t: AttributeType) -> u32 {
    match t {
        AttributeType::Float => 0,
        AttributeType::Sint => 1,
        AttributeType::Uint => 2,
    }
}

pub(super) fn definitions_of(tc: &TranslatorContext) -> cp::Definitions {
    definitions_for(&tc.definitions)
}

pub(super) fn definitions_for(d: &super::definitions::Definitions) -> cp::Definitions {
    let elem = |t: AttributeType| match t {
        AttributeType::Float => Agg::FP32,
        AttributeType::Sint => Agg::S32,
        AttributeType::Uint => Agg::U32,
    };
    cp::Definitions {
        stage: Some(d.stage),
        compute_local_size: d.compute_local_size,
        gp_passthrough: d.gp_passthrough,
        last_in_vertex_pipeline: d.last_in_vertex_pipeline,
        threads_per_input_primitive: d.threads_per_input_primitive,
        input_topology: d.gs.topology,
        output_topology: d.output_topology,
        max_output_vertices: d.max_output_vertices,
        dual_source_blend: false,
        early_z_force: d.gs.early_z,
        y_negate_enabled: false,
        origin_upper_left: true,
        tess_cw: false,
        tess_patch_type: 1,
        tess_spacing: 0,
        ia_indexing: d.ia_indexing,
        oa_indexing: d.oa_indexing,
        transform_feedback_enabled: false,
        supports_scaled_vertex_formats: d.supports_scaled_vertex_formats,
        imap_types: d
            .imap_types
            .iter()
            .map(|i| [i.x as u8, i.y as u8, i.z as u8, i.w as u8])
            .collect(),
        attribute_types: d.gs.attr.iter().map(|t| attr_num(*t)).collect(),
        fragment_output_types: d
            .gs
            .fout
            .iter()
            .map(|t| AggType(AggType::VECTOR4.0 | elem(*t).0))
            .collect(),
    }
}

fn buffer_def(b: &tp::BufferDefinition) -> cp::BufferDef {
    cp::BufferDef {
        layout: match b.layout {
            tp::BufferLayout::Std140 => 0,
            tp::BufferLayout::Std430 => 1,
        },
        set: b.set,
        binding: b.binding,
        name: b.name.clone(),
        fields: b
            .ty
            .fields
            .iter()
            .map(|f| cp::Field {
                ty: agg(f.ty),
                array_length: f.array_length,
                name: f.name.clone(),
            })
            .collect(),
    }
}

fn tex_def(t: &tp::TextureDefinition) -> cp::TextureDef {
    cp::TextureDef {
        set: t.set,
        binding: t.binding,
        array_length: t.array_length,
        separate: t.separate,
        name: t.name.clone(),
        ty: t.ty,
        format: t.format,
        flags: t.flags.0 as i32,
    }
}

fn mem_defs(v: &[tp::MemoryDefinition]) -> Vec<cp::MemoryDef> {
    v.iter()
        .enumerate()
        .map(|(i, m)| cp::MemoryDef {
            id: i as i32,
            name: m.name.clone(),
            ty: agg(m.ty),
            array_length: m.array_length,
        })
        .collect()
}

pub(super) fn properties_of(rm: &ResourceManager) -> cp::Properties {
    let p = &rm.properties;
    cp::Properties {
        constant_buffers: p.constant_buffers.values().map(buffer_def).collect(),
        storage_buffers: p.storage_buffers.values().map(buffer_def).collect(),
        textures: p.textures.values().map(tex_def).collect(),
        images: p.images.values().map(tex_def).collect(),
        local_memories: mem_defs(&p.local_memories),
        shared_memories: mem_defs(&p.shared_memories),
        uses_physical_pointers: p.uses_physical_pointers,
    }
}

pub(super) fn host_caps(acc: &Accessor) -> cp::HostCaps {
    use super::host as h;
    cp::HostCaps {
        reduced_precision: acc.query_host_reduced_precision(),
        supports_fragment_shader_interlock: h::SUPPORTS_FRAGMENT_SHADER_INTERLOCK,
        supports_fragment_shader_ordering_intel: h::SUPPORTS_FRAGMENT_SHADER_ORDERING_INTEL,
        supports_geometry_shader_passthrough: acc.query_host_supports_geometry_shader_passthrough(),
        supports_shader_ballot: h::SUPPORTS_SHADER_BALLOT,
        supports_shader_barrier_divergence: h::SUPPORTS_SHADER_BARRIER_DIVERGENCE,
        supports_shader_float64: acc.query_host_supports_shader_float64(),
        supports_shader_non_uniform_indexing: h::SUPPORTS_SHADER_NON_UNIFORM_INDEXING,
        supports_texture_shadow_lod: acc.query_host_supports_texture_shadow_lod(),
        supports_viewport_mask: h::SUPPORTS_VIEWPORT_MASK,
        metal_image_atomics: true,
    }
}

pub(super) fn buf_info(rm: &ResourceManager, sb: bool) -> Vec<BufferInfo> {
    let l = if sb {
        rm.get_storage_buffer_descriptors()
    } else {
        rm.get_constant_buffer_descriptors()
    };
    l.iter()
        .map(|b| BufferInfo {
            binding: b.binding as i64,
            slot: b.slot as i64,
            cb_slot: b.sb_cb_slot as i64,
            cb_offset: b.sb_cb_offset as i64,
            flags: b.flags.0 as i64,
        })
        .collect()
}

pub(super) fn tex_info(l: Vec<tp::TextureDescriptor>) -> Vec<TexInfo> {
    l.iter()
        .map(|t| TexInfo {
            binding: t.binding as i64,
            ty: t.ty.0,
            format: t.format,
            cbuf_slot: t.cbuf_slot as i64,
            handle: t.handle_index as i64,
            array_length: t.array_length as i64,
            separate: t.separate,
            flags: t.flags.0 as i64,
        })
        .collect()
}

/// `TranslatorContext.Translate()` of a context (no geometry-as-compute).
pub fn translate_context(
    tc: &TranslatorContext,
    acc: &mut Accessor,
    debug_mode: bool,
) -> Result<Program, String> {
    let mut t = run_to_xform(tc, acc)?;
    let mut user: Vec<crate::translation::pipeline::TransformedFunction> =
        std::mem::take(&mut t.funcs);
    let mut helpers: Vec<crate::xform::helper::Function> = t
        .hfm
        .functions
        .iter_mut()
        .skip(user.len())
        .filter_map(Option::take)
        .collect();
    let mut ins = Vec::new();
    for (i, f) in user.iter_mut().enumerate() {
        ins.push(FunctionIn {
            name: format!("fun{i}"),
            blocks: &mut f.blocks,
            in_arguments_count: f.in_arguments,
            out_arguments_count: f.out_arguments,
            returns_value: false,
        });
    }
    for h in helpers.iter_mut() {
        ins.push(FunctionIn {
            name: h.name.clone(),
            blocks: &mut h.blocks,
            in_arguments_count: h.in_arguments as usize,
            out_arguments_count: h.out_arguments as usize,
            returns_value: h.returns_value,
        });
    }
    let defs = definitions_of(tc);
    let info = {
        let mut res = StructRes { rm: &mut t.rm, acc };
        make_structured_program(&mut t.ir, &mut ins, &defs, &mut res, 0, debug_mode)?
    };
    let features = t.used_features as i32 | tc.program.used_features;
    let program = Program {
        cbufs: buf_info(&t.rm, false),
        sbufs: buf_info(&t.rm, true),
        textures: tex_info(t.rm.get_texture_descriptors(true)),
        images: tex_info(t.rm.get_image_descriptors(true)),
        uses_frag_coord: features & feature::FRAG_COORD_XY != 0,
        frag_out_map: tc.definitions.omap_targets as i64,
        geometry_stride: 0,
        spirv: Vec::new(),
    };
    let props = Props {
        definitions: defs,
        host: host_caps(acc),
        properties: properties_of(&t.rm),
        passthrough_attributes: 0,
        target_api: 1,
        cb_slot_to_binding: Vec::new(),
        helper_functions_mask: 0,
        io_definitions: Vec::new(),
    };
    let spirv = generate(info, props)?;
    Ok(Program { spirv, ..program })
}

/// The translator core behind [`Core`].
pub struct Translator;

impl Core for Translator {
    fn translate(
        &self,
        code: &[u8],
        acc: &mut Accessor,
        geometry: bool,
    ) -> Result<Program, String> {
        if geometry {
            return Err("unported: geometry translation".into());
        }
        let job: &Job = acc.job();
        let opts = options_of(job);
        let tc = TranslatorContext::create(code, acc, opts)?;
        let debug = tc.options.debug_mode;
        translate_context(&tc, acc, debug)
    }

    fn passthrough_reader(&self, _code: &[u8], _acc: &mut Accessor) -> Result<Vec<u8>, String> {
        Err("unported: geometry passthrough reader".into())
    }
}

#[allow(dead_code)]
fn _unused(_: Ir, _: Stage) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// The whole driver with the Rust core on every job file of the corpus, compared byte for byte with the C# outputs.
    #[test]
    fn corpus_end_to_end() {
        let (Some(corpus), Some(reff)) = (
            std::env::var_os("NXB_XLATE_CORPUS"),
            std::env::var_os("NXB_XLATE_REF"),
        ) else {
            return;
        };
        let jobs = PathBuf::from(corpus).join("jobs");
        let out = std::env::temp_dir().join(format!("nxb-e2e-{}", std::process::id()));
        let mut files: Vec<PathBuf> = std::fs::read_dir(&jobs)
            .map(|d| d.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        files.sort();
        let (mut spv_same, mut res_same, mut total) = (0, 0, 0);
        let mut why = std::collections::BTreeMap::<String, usize>::new();
        for f in files {
            let stem = f
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let od = out.join(&stem);
            let _ = main_jobs_quiet(&f, &od, &mut why);
            let rd = PathBuf::from(&reff).join(&stem);
            let Ok(list) = std::fs::read_dir(&rd) else {
                continue;
            };
            for e in list.flatten() {
                let p = e.path();
                let name = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                let is_spv = name.ends_with(".spv");
                let is_res = name.ends_with(".res.json");
                if !is_spv && !is_res {
                    continue;
                }
                if is_spv {
                    total += 1;
                }
                let want = std::fs::read(&p).unwrap_or_default();
                if std::fs::read(od.join(&name)).ok().as_deref() == Some(&want[..]) {
                    if is_spv {
                        spv_same += 1;
                    } else {
                        res_same += 1;
                    }
                }
            }
            let _ = std::fs::remove_dir_all(&od);
        }
        let _ = std::fs::remove_dir_all(&out);
        println!(
            "end to end: spv identical {spv_same} of {total}, res.json identical {res_same} of {total}"
        );
        let mut v: Vec<_> = why.into_iter().collect();
        v.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        for (m, n) in v.iter().take(15) {
            println!("  {n:5} {m}");
        }
    }

    fn main_jobs_quiet(
        f: &std::path::Path,
        od: &std::path::Path,
        why: &mut std::collections::BTreeMap<String, usize>,
    ) -> Result<(), String> {
        let text = std::fs::read_to_string(f).map_err(|e| e.to_string())?;
        let jobs = crate::job::parse_jobs(&text)?;
        std::fs::create_dir_all(od).map_err(|e| e.to_string())?;
        for job in &jobs {
            let code = std::fs::read(&job.code_path).map_err(|e| e.to_string())?;
            match crate::driver::run_job(&Translator, job, &code) {
                Ok(o) => {
                    let _ = std::fs::write(od.join(format!("{}.spv", job.key)), &o.spv);
                    let _ =
                        std::fs::write(od.join(format!("{}.res.json", job.key)), o.res.to_text());
                }
                Err(e) => *why.entry(e.chars().take(100).collect()).or_default() += 1,
            }
        }
        Ok(())
    }
}
