//! `Declarations`: the module level declarations (buffers, memories, samplers, images, inputs and outputs) and the
//! function level ones (parameters and locals).

use super::context::*;
use super::props::*;
use super::spirv_enums::{access_qualifier, decoration, dim, image_format, storage_class};
use crate::attr::Stage;
use crate::ir::{IoVariable, StorageKind};
use crate::spv::{Inst, Lit, Operand};
use crate::structured::types::AggType;
use crate::structured::{IoDef, StructFunction};
use crate::types::SamplerType;
use std::collections::HashSet;

type R<T> = Result<T, String>;

fn lit(v: i32) -> Operand {
    Operand::Lit(Lit::I32(v))
}

pub fn declare_parameters(ctx: &mut CodeGenContext, f: &StructFunction) -> R<()> {
    let mut idx = 0;
    for list in [&f.in_arguments, &f.out_arguments] {
        for &t in list {
            let ty = ctx.ty(t)?;
            let ptr = ctx.type_pointer(storage_class::Function, ty, false);
            let arg = ctx.function_parameter(ptr);
            ctx.declare_argument(idx, arg);
            idx += 1;
        }
    }
    Ok(())
}

pub fn declare_locals(ctx: &mut CodeGenContext, f: &StructFunction) -> R<()> {
    for &local in &f.locals {
        let vt = match ctx.ast.kind(local) {
            crate::structured::ast::Kind::Operand { var_type, .. } => *var_type,
            _ => return Err("local is not an operand".into()),
        };
        let t = ctx.ty(vt)?;
        let ptr = ctx.type_pointer(storage_class::Function, t, false);
        let var = ctx.variable(ptr, storage_class::Function, None);
        ctx.add_local_variable(var);
        ctx.declare_local(local, var);
    }
    Ok(())
}

pub fn declare_all(ctx: &mut CodeGenContext) -> R<()> {
    let p = ctx.properties.clone();
    declare_buffers(ctx, &p.constant_buffers, false)?;
    declare_buffers(ctx, &p.storage_buffers, true)?;
    declare_memories(ctx, &p.local_memories, storage_class::Private, false)?;
    declare_memories(ctx, &p.shared_memories, storage_class::Workgroup, true)?;
    declare_samplers(ctx, &p.textures)?;
    declare_images(ctx, &p.images)?;
    declare_inputs_and_outputs(ctx)
}

fn declare_memories(
    ctx: &mut CodeGenContext,
    mems: &[MemoryDef],
    storage: u32,
    shared: bool,
) -> R<()> {
    for m in mems {
        let t = ctx.get_type(m.ty, m.array_length)?;
        let ptr = ctx.type_pointer(storage, t, false);
        let var = ctx.variable(ptr, storage, None);
        ctx.add_global_variable(var);
        if shared {
            ctx.shared_memories.insert(m.id, var);
        } else {
            ctx.local_memories.insert(m.id, var);
        }
    }
    Ok(())
}

fn declare_buffers(ctx: &mut CodeGenContext, buffers: &[BufferDef], is_buffer: bool) -> R<()> {
    let mut decorated: HashSet<u32> = HashSet::new();
    for b in buffers {
        let set_index = if ctx.target_api == 1 { b.set } else { 0 };
        let alignment = if b.layout == 0 { 16 } else { 4 };
        let mask = alignment - 1;
        let mut offset = 0;
        let mut field_types: Vec<Inst> = Vec::new();
        let mut field_offsets: Vec<i32> = Vec::new();
        for (fi, f) in b.fields.iter().enumerate() {
            let field_size = (f.ty.size_in_bytes() + mask) & !mask;
            let ft = ctx.get_type(f.ty, f.array_length)?;
            field_types.push(ft);
            field_offsets.push(offset);
            if f.ty.has(AggType::ARRAY) {
                if decorated.insert(ctx.m.id(ft)) {
                    ctx.decorate(ft, decoration::ArrayStride, &[lit(field_size)]);
                }
                if !(f.array_length > 0 || fi == b.fields.len() - 1) {
                    return Err("runtime array must be the last field".into());
                }
                offset += field_size * f.array_length;
            } else {
                offset += field_size;
            }
        }
        let struct_type = ctx.type_struct(false, &field_types);
        if decorated.insert(ctx.m.id(struct_type)) {
            ctx.decorate(
                struct_type,
                if is_buffer {
                    decoration::BufferBlock
                } else {
                    decoration::Block
                },
                &[],
            );
            for (fi, o) in field_offsets.iter().enumerate() {
                ctx.member_decorate(
                    struct_type,
                    Lit::I32(fi as i32),
                    decoration::Offset,
                    &[lit(*o)],
                );
            }
        }
        let ptr = ctx.type_pointer(storage_class::Uniform, struct_type, false);
        let var = ctx.variable(ptr, storage_class::Uniform, None);
        ctx.name(var, &b.name);
        ctx.decorate(var, decoration::DescriptorSet, &[lit(set_index)]);
        ctx.decorate(var, decoration::Binding, &[lit(b.binding)]);
        ctx.add_global_variable(var);
        if is_buffer {
            ctx.storage_buffers.insert(b.binding, var);
        } else {
            ctx.constant_buffers.insert(b.binding, var);
        }
    }
    Ok(())
}

fn dim_of(t: SamplerType) -> R<u32> {
    Ok(match t.0 & SamplerType::MASK {
        1 => dim::Dim1D,
        3 => dim::Dim2D,
        4 => dim::Dim3D,
        5 => dim::Cube,
        2 => dim::Buffer,
        m => return Err(format!("Invalid sampler type \"{m}\".")),
    })
}

fn flag_lit(t: SamplerType, f: u32) -> Lit {
    Lit::I32((t.0 & f != 0) as i32)
}

fn declare_samplers(ctx: &mut CodeGenContext, samplers: &[TextureDef]) -> R<()> {
    for s in samplers {
        let set_index = if ctx.target_api == 1 { s.set } else { 0 };
        let (image_type, sampled_image_type);
        if s.ty != SamplerType::NONE {
            let d = dim_of(s.ty)?;
            let fp = ctx.type_fp32();
            image_type = ctx.type_image(
                fp,
                d,
                flag_lit(s.ty, SamplerType::SHADOW),
                flag_lit(s.ty, SamplerType::ARRAY),
                flag_lit(s.ty, SamplerType::MULTISAMPLE),
                Lit::I32(1),
                image_format::Unknown,
                None,
                false,
            );
            sampled_image_type = ctx.type_sampled_image(image_type, false);
        } else {
            let t = ctx.type_sampler(false);
            image_type = t;
            sampled_image_type = t;
        }
        let soi = if s.separate {
            image_type
        } else {
            sampled_image_type
        };
        let sampled_image_pointer_type =
            ctx.type_pointer(storage_class::UniformConstant, soi, false);
        let mut array_ptr = sampled_image_pointer_type;
        if s.array_length == 0 {
            let at = ctx.type_runtime_array(soi, false);
            array_ptr = ctx.type_pointer(storage_class::UniformConstant, at, false);
        } else if s.array_length != 1 {
            let u = ctx.type_u32();
            let len = ctx.constant(u, Lit::I32(s.array_length));
            let at = ctx.type_array(soi, len, false);
            array_ptr = ctx.type_pointer(storage_class::UniformConstant, at, false);
        }
        let var = ctx.variable(array_ptr, storage_class::UniformConstant, None);
        ctx.samplers.insert(
            (s.set, s.binding),
            SamplerDecl {
                image_type,
                sampled_image_type,
                sampled_image_pointer_type,
                image: var,
                is_indexed: s.array_length != 1,
            },
        );
        ctx.samplers_types.insert((s.set, s.binding), s.ty);
        ctx.name(var, &s.name);
        ctx.decorate(var, decoration::DescriptorSet, &[lit(set_index)]);
        ctx.decorate(var, decoration::Binding, &[lit(s.binding)]);
        ctx.add_global_variable(var);
    }
    Ok(())
}

const IMAGE_FORMATS: [u32; 40] = [
    image_format::Unknown,
    image_format::R8,
    image_format::R8Snorm,
    image_format::R8ui,
    image_format::R8i,
    image_format::R16f,
    image_format::R16,
    image_format::R16Snorm,
    image_format::R16ui,
    image_format::R16i,
    image_format::R32f,
    image_format::R32ui,
    image_format::R32i,
    image_format::Rg8,
    image_format::Rg8Snorm,
    image_format::Rg8ui,
    image_format::Rg8i,
    image_format::Rg16f,
    image_format::Rg16,
    image_format::Rg16Snorm,
    image_format::Rg16ui,
    image_format::Rg16i,
    image_format::Rg32f,
    image_format::Rg32ui,
    image_format::Rg32i,
    image_format::Rgba8,
    image_format::Rgba8Snorm,
    image_format::Rgba8ui,
    image_format::Rgba8i,
    image_format::Rgba16f,
    image_format::Rgba16,
    image_format::Rgba16Snorm,
    image_format::Rgba16ui,
    image_format::Rgba16i,
    image_format::Rgba32f,
    image_format::Rgba32ui,
    image_format::Rgba32i,
    image_format::Rgb10A2,
    image_format::Rgb10a2ui,
    image_format::R11fG11fB10f,
];

fn declare_images(ctx: &mut CodeGenContext, images: &[TextureDef]) -> R<()> {
    for im in images {
        let set_index = if ctx.target_api == 1 { im.set } else { 0 };
        let d = dim_of(im.ty)?;
        let comp = crate::structured::types::format_component_type(im.format);
        let ct = ctx.ty(comp)?;
        let fmt = *IMAGE_FORMATS
            .get(im.format)
            .ok_or("invalid texture format")?;
        let image_type = ctx.type_image(
            ct,
            d,
            flag_lit(im.ty, SamplerType::SHADOW),
            flag_lit(im.ty, SamplerType::ARRAY),
            flag_lit(im.ty, SamplerType::MULTISAMPLE),
            Lit::I32(access_qualifier::ReadWrite as i32),
            fmt,
            None,
            false,
        );
        let image_pointer_type =
            ctx.type_pointer(storage_class::UniformConstant, image_type, false);
        let mut array_ptr = image_pointer_type;
        if im.array_length == 0 {
            let at = ctx.type_runtime_array(image_type, false);
            array_ptr = ctx.type_pointer(storage_class::UniformConstant, at, false);
        } else if im.array_length != 1 {
            let u = ctx.type_u32();
            let len = ctx.constant(u, Lit::I32(im.array_length));
            let at = ctx.type_array(image_type, len, false);
            array_ptr = ctx.type_pointer(storage_class::UniformConstant, at, false);
        }
        let var = ctx.variable(array_ptr, storage_class::UniformConstant, None);
        ctx.images.insert(
            (im.set, im.binding),
            ImageDecl {
                image_type,
                image_pointer_type,
                image: var,
                is_indexed: im.array_length != 1,
            },
        );
        ctx.name(var, &im.name);
        ctx.decorate(var, decoration::DescriptorSet, &[lit(set_index)]);
        ctx.decorate(var, decoration::Binding, &[lit(im.binding)]);
        if im.flags & 8 != 0 {
            ctx.decorate(var, decoration::Coherent, &[]);
        }
        ctx.add_global_variable(var);
    }
    Ok(())
}

fn spirv_built_in_array_length(v: IoVariable) -> i32 {
    match v {
        IoVariable::ClipDistance => 8,
        IoVariable::TessellationLevelInner => 2,
        IoVariable::TessellationLevelOuter => 4,
        IoVariable::ViewportMask => 1,
        IoVariable::UserDefined => 16,
        _ => 1,
    }
}

fn is_per_vertex_built_in(v: IoVariable) -> bool {
    matches!(
        v,
        IoVariable::Position | IoVariable::PointSize | IoVariable::ClipDistance
    )
}

fn declare_inputs_and_outputs(ctx: &mut CodeGenContext) -> R<()> {
    let stage = ctx.definitions.stage()?;
    let ios: Vec<IoDef> = ctx.io_definitions.clone();
    let mut first_location = i32::MAX;
    if stage == Stage::Fragment && ctx.definitions.dual_source_blend {
        for io in &ios {
            if io.io_variable == IoVariable::FragmentOutputColor && io.location < first_location {
                first_location = io.location;
            }
        }
    }
    for io in &ios {
        // PixelImap: 0 Unused, 1 Constant, 2 Perspective, 3 ScreenLinear
        let mut iq = 0u8;
        if stage == Stage::Fragment {
            if io.io_variable == IoVariable::UserDefined {
                let imap = ctx
                    .definitions
                    .imap_types
                    .get(io.location as usize)
                    .ok_or("imap type missing")?;
                iq = imap.iter().copied().find(|x| *x != 0).unwrap_or(imap[3]);
            } else {
                let (_, vt) = spirv_built_in(io.io_variable);
                let elem = vt.0 & AggType::ELEMENT_TYPE_MASK;
                if elem == AggType::S32.0 || elem == AggType::U32.0 {
                    iq = 1;
                }
            }
        } else if is_per_vertex_built_in(io.io_variable) {
            continue;
        }
        let is_out = is_output(io.storage_kind);
        let per_patch = is_per_patch(io.storage_kind);
        declare_input_or_output(ctx, io, is_out, per_patch, iq, first_location)?;
    }
    declare_per_vertex_block(ctx)
}

fn declare_per_vertex_block(ctx: &mut CodeGenContext) -> R<()> {
    let stage = ctx.definitions.stage()?;
    if !stage.is_vtg() {
        return Ok(());
    }
    if stage != Stage::Vertex {
        return Err("unported: per-vertex block of tessellation and geometry stages".into());
    }
    let out_struct = create_per_vertex_struct_type(ctx)?;
    let ptr = ctx.type_pointer(storage_class::Output, out_struct, false);
    let var = ctx.variable(ptr, storage_class::Output, None);
    ctx.add_global_variable(var);
    ctx.outputs.insert(
        IoDef {
            storage_kind: StorageKind::Output,
            io_variable: IoVariable::Position,
            location: 0,
            component: 0,
        },
        var,
    );
    Ok(())
}

fn create_per_vertex_struct_type(ctx: &mut CodeGenContext) -> R<Inst> {
    use super::spirv_enums::built_in;
    let f = ctx.type_fp32();
    let vec4 = ctx.type_vector(f, Lit::I32(4), false);
    let float_type = ctx.type_fp32();
    let f = ctx.type_fp32();
    let u = ctx.type_u32();
    let c8 = ctx.constant(u, Lit::I32(8));
    let array8 = ctx.type_array(f, c8, false);
    let f = ctx.type_fp32();
    let u = ctx.type_u32();
    let c1 = ctx.constant(u, Lit::I32(1));
    let array1 = ctx.type_array(f, c1, false);
    let s = ctx.type_struct(true, &[vec4, float_type, array8, array1]);
    ctx.name(s, "gl_PerVertex");
    ctx.member_name(s, Lit::I32(0), "gl_Position");
    ctx.member_name(s, Lit::I32(1), "gl_PointSize");
    ctx.member_name(s, Lit::I32(2), "gl_ClipDistance");
    ctx.member_name(s, Lit::I32(3), "gl_CullDistance");
    ctx.decorate(s, decoration::Block, &[]);
    if ctx.host.reduced_precision {
        ctx.member_decorate(s, Lit::I32(0), decoration::Invariant, &[]);
    }
    ctx.member_decorate(
        s,
        Lit::I32(0),
        decoration::BuiltIn,
        &[lit(built_in::Position as i32)],
    );
    ctx.member_decorate(
        s,
        Lit::I32(1),
        decoration::BuiltIn,
        &[lit(built_in::PointSize as i32)],
    );
    ctx.member_decorate(
        s,
        Lit::I32(2),
        decoration::BuiltIn,
        &[lit(built_in::ClipDistance as i32)],
    );
    ctx.member_decorate(
        s,
        Lit::I32(3),
        decoration::BuiltIn,
        &[lit(built_in::CullDistance as i32)],
    );
    Ok(s)
}

fn declare_input_or_output(
    ctx: &mut CodeGenContext,
    io: &IoDef,
    is_out: bool,
    per_patch: bool,
    iq: u8,
    first_location: i32,
) -> R<()> {
    let stage = ctx.definitions.stage()?;
    if per_patch || !matches!(stage, Stage::Vertex | Stage::Fragment | Stage::Compute) {
        return Err("unported: per-patch or geometry/tessellation input or output".into());
    }
    let v = io.io_variable;
    let sc = if is_out {
        storage_class::Output
    } else {
        storage_class::Input
    };
    let (is_built_in, mut built_in, var_type);
    if v == IoVariable::UserDefined {
        var_type = ctx.definitions.user_defined_type(io.location, is_out)?;
        is_built_in = false;
        built_in = 0;
    } else if v == IoVariable::FragmentOutputColor {
        var_type = ctx.definitions.fragment_output_color_type(io.location)?;
        is_built_in = false;
        built_in = 0;
    } else {
        let (b, t) = spirv_built_in(v);
        built_in = b;
        var_type = t;
        is_built_in = true;
        if t == AggType::INVALID {
            return Err(format!("Unknown variable {}.", v.0));
        }
    }
    let _ = &mut built_in;
    let spv_type = ctx.get_type(var_type, spirv_built_in_array_length(v))?;
    let ptr = ctx.type_pointer(sc, spv_type, false);
    let var = ctx.variable(ptr, sc, None);
    if is_built_in {
        if ctx.host.reduced_precision && v == IoVariable::Position {
            ctx.decorate(var, decoration::Invariant, &[]);
        }
        ctx.decorate(var, decoration::BuiltIn, &[lit(built_in as i32)]);
    } else if v == IoVariable::UserDefined {
        ctx.decorate(var, decoration::Location, &[lit(io.location)]);
        if !is_out
            && !per_patch
            && ctx.attribute_passthrough & (1 << io.location) != 0
            && ctx.host.supports_geometry_shader_passthrough
        {
            ctx.decorate(var, decoration::PassthroughNV, &[]);
        }
    } else if v == IoVariable::FragmentOutputColor {
        let location = io.location;
        if stage == Stage::Fragment && ctx.definitions.dual_source_blend {
            let index = location - first_location;
            if (index as u32) < 2 {
                ctx.decorate(var, decoration::Location, &[lit(first_location)]);
                ctx.decorate(var, decoration::Index, &[lit(index)]);
            } else {
                ctx.decorate(var, decoration::Location, &[lit(location)]);
            }
        } else {
            ctx.decorate(var, decoration::Location, &[lit(location)]);
        }
    }
    if !is_out {
        match iq {
            1 => {
                ctx.decorate(var, decoration::Flat, &[]);
            }
            3 => {
                ctx.decorate(var, decoration::NoPerspective, &[]);
            }
            _ => {}
        }
    }
    ctx.add_global_variable(var);
    if is_out {
        ctx.outputs.insert(*io, var);
    } else {
        ctx.inputs.insert(*io, var);
    }
    Ok(())
}
