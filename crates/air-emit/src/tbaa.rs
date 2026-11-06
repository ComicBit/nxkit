//! Struct-path TBAA nodes as clang writes them for MSL ("Simple C++ TBAA").

use msl_sema::types::{Scalar, Type};

use crate::fx::HashMap;
use crate::meta::{s, Md};
use crate::ModCx;

/// Node numbers already built. Metadata nodes are only ever appended and deduplicated, so a
/// node built once keeps its number for the whole module.
#[derive(Default)]
pub struct Cache {
    char: Option<usize>,
    named: HashMap<String, usize>,
    structs: HashMap<usize, usize>,
    wrappers: HashMap<String, usize>,
    tags: HashMap<(usize, usize, u32), usize>,
}

fn root(cx: &mut ModCx) -> usize {
    cx.meta.node(&[s("Simple C++ TBAA")])
}

/// `omnipotent char`
pub fn char(cx: &mut ModCx) -> usize {
    if let Some(n) = cx.tbaa.char {
        return n;
    }
    let r = root(cx);
    let n = cx
        .meta
        .node(&[s("omnipotent char"), Md::Node(r), Md::Raw("i64 0".into())]);
    cx.tbaa.char = Some(n);
    n
}

/// A named scalar node under `omnipotent char`.
pub fn named(cx: &mut ModCx, name: &str) -> usize {
    if let Some(n) = cx.tbaa.named.get(name) {
        return *n;
    }
    let c = char(cx);
    let n = cx
        .meta
        .node(&[s(name), Md::Node(c), Md::Raw("i64 0".into())]);
    cx.tbaa.named.insert(name.to_string(), n);
    n
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
    if let Some(n) = cx.tbaa.structs.get(&sid) {
        return *n;
    }
    let st = cx.types.structs[sid].clone();
    let mut ops = vec![s(&format!("_ZTS{}{}", st.name.len(), st.name))];
    for f in &st.fields {
        let n = match &f.ty {
            Type::Array(e, len) if f.metal_array => metal_array(cx, e, *len),
            t => member(cx, t),
        };
        ops.push(Md::Node(n));
        ops.push(Md::Raw(format!("i64 {}", f.offset)));
    }
    let n = cx.meta.node(&ops);
    cx.tbaa.structs.insert(sid, n);
    n
}

/// The node of a `metal::array<T, n>` member: `_ZTSN5metal5arrayI<T>Lm<n>EEE` with T mangled
/// inside the `metal` namespace.
fn metal_array(cx: &mut ModCx, e: &Type, len: u32) -> usize {
    let el = wrapper_name(e);
    let inner = el.strip_prefix("_ZTSN5metal").unwrap_or(&el);
    let name = format!("_ZTSN5metal5arrayINS_{inner}Lm{len}EEE");
    if let Some(n) = cx.tbaa.wrappers.get(&name) {
        return *n;
    }
    let c = char(cx);
    let n = cx
        .meta
        .node(&[s(&name), Md::Node(c), Md::Raw("i64 0".into())]);
    cx.tbaa.wrappers.insert(name, n);
    n
}

/// An access tag (base type, access type, offset).
pub fn tag(cx: &mut ModCx, base: usize, access: usize, off: u32) -> usize {
    if let Some(n) = cx.tbaa.tags.get(&(base, access, off)) {
        return *n;
    }
    let n = cx.meta.node(&[
        Md::Node(base),
        Md::Node(access),
        Md::Raw(format!("i64 {off}")),
    ]);
    cx.tbaa.tags.insert((base, access, off), n);
    n
}

fn handle_name(t: &Type) -> String {
    match t {
        Type::Texture(tx) => format!("__metal_{}_t", tx.kind.air_name()),
        _ => "__metal_sampler_t".to_string(),
    }
}

/// The mangled type-info name of a `metal::texture*` or `metal::sampler` wrapper struct.
fn wrapper_name(t: &Type) -> String {
    match t {
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
    }
}

/// The node of a `metal::texture*` or `metal::sampler` wrapper struct.
fn wrapper(cx: &mut ModCx, t: &Type) -> usize {
    let name = wrapper_name(t);
    if let Some(n) = cx.tbaa.wrappers.get(&name) {
        return *n;
    }
    let h = named(cx, &handle_name(t));
    let n = cx
        .meta
        .node(&[s(&name), Md::Node(h), Md::Raw("i64 0".into())]);
    cx.tbaa.wrappers.insert(name, n);
    n
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
