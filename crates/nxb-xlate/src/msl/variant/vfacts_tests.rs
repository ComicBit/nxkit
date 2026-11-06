//! Tests of the raster facts of vertex shaders.

use super::vtest::{F32, ONE, V};
use crate::ir::{Inst, IoVariable, StorageKind};
use crate::msl::refl::Raster;
use crate::structured::ast::{A, BlockType};

const POS: IoVariable = IoVariable::Position;
const USER: IoVariable = IoVariable::UserDefined;

fn raster(v: &V) -> Raster {
    let t = v.translate();
    t.libraries[0]
        .reflection
        .as_ref()
        .expect("reflection")
        .raster
        .clone()
}

/// `position = attr0` with the given w and a store of `(attr.x, attr.y)` to the varying 1.
fn copy(w_one: bool) -> V {
    copy_with(w_one, |_| {})
}

fn copy_with(w_one: bool, extra: impl FnOnce(&mut V)) -> V {
    let mut v = V::new(&[(0, 0)]);
    v.out(1);
    let c: Vec<_> = (0..4).map(|i| v.attr(0, i, F32)).collect();
    for i in 0..3 {
        v.store(POS, None, i, c[i as usize]);
    }
    let w = if w_one { v.k(ONE) } else { c[3] };
    v.store(POS, None, 3, w);
    v.store(USER, Some(1), 0, c[0]);
    v.store(USER, Some(1), 1, c[1]);
    extra(&mut v);
    v.ret();
    v
}

#[test]
fn a_copied_position_names_its_attribute() {
    let r = raster(&copy(true));
    assert_eq!(r.pos_attr, Some(0));
    assert!(r.pos_w_one);
    assert_eq!(r.uv_from_attr, vec![(1, 0)]);
    assert!(r.uv_affine.is_empty());
}

#[test]
fn a_copied_w_is_not_one() {
    let r = raster(&copy(false));
    assert_eq!(r.pos_attr, Some(0));
    assert!(!r.pos_w_one);
}

#[test]
fn a_computed_position_has_no_position_fact() {
    let mut v = V::new(&[(0, 0)]);
    let x = v.attr(0, 0, F32);
    let one = v.k(ONE);
    let y = v.f(Inst::Add, vec![x, one]);
    v.store(POS, None, 0, y);
    v.ret();
    let r = raster(&v);
    assert_eq!(r.pos_attr, None);
}

#[test]
fn an_affine_varying_names_its_uniforms() {
    let mut v = V::new(&[(0, 0)]);
    v.out(2);
    v.cb(3, "vp_c3");
    let c: Vec<_> = (0..4).map(|i| v.attr(0, i, F32)).collect();
    for i in 0..4 {
        v.store(POS, None, i, c[i as usize]);
    }
    let (sx, ox) = (v.uni(3, 5, 0), v.uni(3, 5, 1));
    let (sy, oy) = (v.uni(3, 6, 2), v.uni(3, 6, 3));
    let x = v.f(Inst::FusedMultiplyAdd, vec![c[0], sx, ox]);
    let y = v.f(Inst::FusedMultiplyAdd, vec![c[1], sy, oy]);
    v.store(USER, Some(2), 0, x);
    v.store(USER, Some(2), 1, y);
    v.ret();
    let r = raster(&v);
    assert_eq!(r.pos_attr, Some(0));
    assert!(r.uv_from_attr.is_empty());
    let (l, a) = &r.uv_affine[0];
    assert_eq!(*l, 2);
    assert_eq!(a.attr, 0);
    assert_eq!(
        (a.sx, a.ox, a.sy, a.oy),
        ((0, 5, 0), (0, 5, 1), (0, 6, 2), (0, 6, 3))
    );
}

/// A position `(a.x, a.y, a.z, 1)` with the varying `loc` written by `f(px, py, pw)`.
fn screen(f: impl FnOnce(&mut V, [A; 3]) -> [A; 3]) -> Raster {
    let mut v = V::new(&[(0, 0)]);
    v.out(1);
    let c: Vec<_> = (0..3).map(|i| v.attr(0, i, F32)).collect();
    let one = v.k(ONE);
    for i in 0..3 {
        v.store(POS, None, i, c[i as usize]);
    }
    v.store(POS, None, 3, one);
    let [x, y, w] = f(&mut v, [c[0], c[1], one]);
    v.store(USER, Some(1), 0, x);
    v.store(USER, Some(1), 1, y);
    v.store(USER, Some(1), 3, w);
    v.ret();
    raster(&v)
}

#[test]
fn a_varying_of_the_screen_position_is_listed() {
    let r = screen(|v, [px, py, pw]| {
        let half = v.k(0x3f00_0000);
        let nhalf = v.k(0xbf00_0000u32 as i32);
        let x = v.f(Inst::FusedMultiplyAdd, vec![px, half, half]);
        let _ = pw;
        let y = v.f(Inst::FusedMultiplyAdd, vec![py, nhalf, half]);
        let one = v.k(ONE);
        [x, y, one]
    });
    assert_eq!(r.screen, vec![1]);
}

#[test]
fn a_varying_that_is_not_the_screen_position_is_not_listed() {
    let r = screen(|v, [px, py, pw]| {
        let half = v.k(0x3f00_0000);
        let x = v.f(Inst::FusedMultiplyAdd, vec![px, half, half]);
        let _ = pw;
        [x, py, half]
    });
    assert!(r.screen.is_empty());
}

#[test]
fn a_branch_makes_the_facts_unprovable() {
    let v = copy_with(true, |v| {
        let cond = v.k(1);
        let blk = v.s.info.ast.new_block(BlockType::If, Some(cond));
        v.put(blk);
    });
    assert_eq!(raster(&v), Raster::default());
}

#[test]
fn a_clip_distance_makes_the_facts_unprovable() {
    let v = copy_with(true, |v| {
        let z = v.k(0);
        let var = v.k(IoVariable::ClipDistance.0 as i32);
        let op = v.op(Inst::Store, StorageKind::Output, vec![var, z, z]);
        v.put(op);
    });
    assert_eq!(raster(&v), Raster::default());
}
