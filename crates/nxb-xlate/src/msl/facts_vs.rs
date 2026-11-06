//! The raster facts of a vertex shader: `pos_attr`, `pos_w`, `uv_from_attr`, `uv_affine` and `screen`.
//!
//! The host uses them to prove that a draw of a full-screen pass needs no vertex shader at all. Every fact is
//! derived from the trace of the stores (`facts_vs_sym`). A shader the trace cannot follow has no position fact,
//! which the host reads as unprovable.

use super::args::Res;
use super::facts_vs_screen;
use super::facts_vs_sym::{Node, Sym, Trace, trace};
use super::io::Plan;
use super::refl::{Raster, UvAffine};
use crate::codegen::props::Props;
use crate::structured::StructInfo;

/// The first attribute location the host cannot read a draw's vertices from.
const ATTR_END: u32 = 16;

/// The attribute component a value copies.
fn attr(s: &Sym) -> Option<(u32, u32)> {
    match **s {
        Node::Attr(l, c) if l < ATTR_END => Some((l, c)),
        _ => None,
    }
}

/// An attribute component, a scale uniform and an offset uniform.
type Affine = ((u32, u32), [u32; 3], [u32; 3]);

/// The parts of `fma(attr.c, scale, offset)`.
fn affine(s: &Sym) -> Option<Affine> {
    let Node::Fma(a, b, c) = &**s else {
        return None;
    };
    match (&**b, &**c) {
        (Node::Uni(b), Node::Uni(c)) => Some((attr(a)?, *b, *c)),
        _ => None,
    }
}

/// The attribute the position is copied from and whether its w is 1.
fn position(t: &Trace) -> Option<(u32, bool)> {
    let (a, c) = attr(t.pos(0)?)?;
    if c != 0 {
        return None;
    }
    for comp in [1, 2] {
        if attr(t.pos(comp)?) != Some((a, comp as u32)) {
            return None;
        }
    }
    let w = t.pos(3)?;
    if **w == Node::Konst(1.0f32.to_bits()) {
        Some((a, true))
    } else if attr(w) == Some((a, 3)) {
        Some((a, false))
    } else {
        None
    }
}

/// The facts of the vertex shader.
pub fn raster(info: &StructInfo, props: &Props, plan: &Plan, res: &Res) -> Raster {
    let Some(t) = trace(info, props, plan, res) else {
        return Raster::default();
    };
    let screen = facts_vs_screen::locations(&t, plan);
    let Some((pos_attr, one)) = position(&t) else {
        return Raster {
            screen,
            ..Raster::default()
        };
    };
    let mut r = Raster {
        pos_attr: Some(pos_attr),
        pos_w_one: one,
        screen,
        ..Raster::default()
    };
    let mut locs = plan.user_locs.clone();
    locs.sort_unstable();
    locs.dedup();
    for l in locs {
        let (Some(x), Some(y)) = (t.user(l, 0), t.user(l, 1)) else {
            continue;
        };
        if let (Some((ax, 0)), Some((ay, 1))) = (attr(x), attr(y))
            && ax == ay
        {
            r.uv_from_attr.push((l, ax));
        } else if let (Some((ax, sx, ox)), Some((ay, sy, oy))) = (affine(x), affine(y))
            && ax.1 == 0
            && ay.1 == 1
            && ax.0 == ay.0
        {
            r.uv_affine.push((
                l,
                UvAffine {
                    attr: ax.0,
                    sx: (sx[0], sx[1], sx[2]),
                    ox: (ox[0], ox[1], ox[2]),
                    sy: (sy[0], sy[1], sy[2]),
                    oy: (oy[0], oy[1], oy[2]),
                },
            ));
        }
    }
    r
}
