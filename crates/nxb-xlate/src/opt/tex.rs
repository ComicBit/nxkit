//! `TextureOperation` helpers and the instruction predicates of `InstructionExtensions`.
use crate::accessor::SetBinding;
use crate::ir::{Inst, Ir, NodeId, TexData};

pub const BINDLESS: u32 = 1;
pub const INT_COORDS: u32 = 1 << 3;
pub const LOD_LEVEL: u32 = 1 << 5;
pub const ATOMIC_METAL: u32 = 1 << 23;

pub fn masked(inst: Inst) -> Inst {
    Inst(inst.0 & Inst::MASK)
}

pub fn is_image(inst: Inst) -> bool {
    matches!(
        masked(inst),
        Inst::ImageAtomic | Inst::ImageLoad | Inst::ImageStore
    )
}

pub fn is_image_store(inst: Inst) -> bool {
    matches!(masked(inst), Inst::ImageAtomic | Inst::ImageStore)
}

pub fn is_texture_query(inst: Inst) -> bool {
    matches!(
        masked(inst),
        Inst::Lod | Inst::TextureQuerySamples | Inst::TextureQuerySize
    )
}

/// `HandleType` numbers of `TextureHandleType`.
pub const SEPARATE_SAMPLER_HANDLE: i32 = 1;
pub const SEPARATE_SAMPLER_ID: i32 = 2;
pub const SEPARATE_CONSTANT_SAMPLER_HANDLE: i32 = 3;
pub const DIRECT: i32 = 4;

pub fn pack_offsets(o0: i32, o1: i32, ty: i32) -> i32 {
    o0 | (o1 << 14) | (ty << 28)
}

pub fn pack_slots(s0: i32, s1: i32) -> i32 {
    s0 | ((s1 + 1) << 16)
}

/// The texture fields of a `TextureOperation`, `None` for other operations.
pub fn tex(ir: &Ir, n: NodeId) -> Option<&TexData> {
    if ir.is_op(n) {
        ir.op(n).tex.as_ref()
    } else {
        None
    }
}

pub fn tex_mut(ir: &mut Ir, n: NodeId) -> Result<&mut TexData, String> {
    ir.op_mut(n)
        .tex
        .as_mut()
        .ok_or_else(|| "not a texture operation".to_string())
}

/// `TextureOperation.TurnIntoArray(setAndBinding)`.
pub fn turn_into_array(ir: &mut Ir, n: NodeId, sb: SetBinding) -> Result<(), String> {
    let t = tex_mut(ir, n)?;
    t.flags &= !BINDLESS;
    t.set = sb.set;
    t.binding = sb.binding;
    Ok(())
}

/// `TextureOperation.SetBinding`.
pub fn set_binding(ir: &mut Ir, n: NodeId, sb: SetBinding) -> Result<(), String> {
    if tex_mut(ir, n)?.flags & BINDLESS != 0 {
        tex_mut(ir, n)?.flags &= !BINDLESS;
        ir.remove_source(n, 0);
    }
    let t = tex_mut(ir, n)?;
    t.set = sb.set;
    t.binding = sb.binding;
    Ok(())
}
