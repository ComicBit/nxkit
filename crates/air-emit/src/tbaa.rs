//! Struct-path TBAA nodes as clang writes them for MSL ("Simple C++ TBAA").

use msl_sema::types::{Scalar, Type};

use crate::meta::{s, Md};
use crate::ModCx;

fn root(cx: &mut ModCx) -> usize {
    cx.meta.node(&[s("Simple C++ TBAA")])
}

/// `omnipotent char`
pub fn char(cx: &mut ModCx) -> usize {
    let r = root(cx);
    cx.meta
        .node(&[s("omnipotent char"), Md::Node(r), Md::Raw("i64 0".into())])
}

/// A named scalar node under `omnipotent char`.
pub fn named(cx: &mut ModCx, name: &str) -> usize {
    let c = char(cx);
    cx.meta
        .node(&[s(name), Md::Node(c), Md::Raw("i64 0".into())])
}

/// The node of a scalar type (signedness does not count).
pub fn scalar(cx: &mut ModCx, sc: Scalar) -> usize {
    match sc {
        Scalar::Char | Scalar::Uchar => char(cx),
        Scalar::Bool => named(cx, "bool"),
        Scalar::Short | Scalar::Ushort => named(cx, "short"),
        Scalar::Int | Scalar::Uint => named(cx, "int"),
        Scalar::Long | Scalar::Ulong | Scalar::SizeT => named(cx, "long"),
        Scalar::Half => named(cx, "half"),
        Scalar::Float => named(cx, "float"),
    }
}

/// The type node of a struct member type.
fn member(cx: &mut ModCx, t: &Type) -> usize {
    match t {
        Type::Vector(..) | Type::Packed(..) if cx.vec_tbaa_char => char(cx),
        Type::Scalar(sc) | Type::Vector(sc, _) | Type::Packed(sc, _) => scalar(cx, *sc),
        Type::Struct(id) => structure(cx, *id),
        Type::Ptr(..) => named(cx, "any pointer"),
        Type::Texture(_) | Type::Sampler => wrapper(cx, t),
        _ => char(cx),
    }
}

/// The struct type node `_ZTS<len><name>` with its members and offsets.
pub fn structure(cx: &mut ModCx, sid: usize) -> usize {
    let st = cx.types.structs[sid].clone();
    let mut ops = vec![s(&format!("_ZTS{}{}", st.name.len(), st.name))];
    for f in &st.fields {
        let n = member(cx, &f.ty);
        ops.push(Md::Node(n));
        ops.push(Md::Raw(format!("i64 {}", f.offset)));
    }
    cx.meta.node(&ops)
}

/// An access tag (base type, access type, offset).
pub fn tag(cx: &mut ModCx, base: usize, access: usize, off: u32) -> usize {
    cx.meta.node(&[
        Md::Node(base),
        Md::Node(access),
        Md::Raw(format!("i64 {off}")),
    ])
}

fn handle_name(t: &Type) -> String {
    match t {
        Type::Texture(tx) => format!("__metal_{}_t", tx.kind.air_name()),
        _ => "__metal_sampler_t".to_string(),
    }
}

/// The node of a `metal::texture*` or `metal::sampler` wrapper struct.
fn wrapper(cx: &mut ModCx, t: &Type) -> usize {
    let name = match t {
        Type::Texture(tx) => {
            let el = match tx.elem {
                Scalar::Float => "f",
                Scalar::Half => "Dh",
                Scalar::Int => "i",
                _ => "j",
            };
            let acc = match tx.access {
                msl_sema::types::Access::Sample => 0,
                msl_sema::types::Access::Read => 1,
                msl_sema::types::Access::Write => 2,
                msl_sema::types::Access::ReadWrite => 3,
            };
            let k = tx.kind.name();
            format!(
                "_ZTSN5metal{}{k}I{el}LNS_6accessE{acc}ELNS_16memory_coherenceE0EvEE",
                k.len()
            )
        }
        _ => "_ZTSN5metal7samplerE".to_string(),
    };
    let h = named(cx, &handle_name(t));
    cx.meta
        .node(&[s(&name), Md::Node(h), Md::Raw("i64 0".into())])
}

/// Scalar tag on the handle type itself (what a by-value copy's `!tbaa.struct` names).
pub fn handle_scalar_tag(cx: &mut ModCx, t: &Type) -> usize {
    let h = named(cx, &handle_name(t));
    tag(cx, h, h, 0)
}

/// Access tag of the handle inside a wrapper (a method's `this` access).
pub fn handle_tag(cx: &mut ModCx, t: &Type) -> usize {
    let w = wrapper(cx, t);
    let h = named(cx, &handle_name(t));
    tag(cx, w, h, 0)
}
