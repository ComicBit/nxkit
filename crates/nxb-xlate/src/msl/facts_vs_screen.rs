//! The varyings of a vertex shader that hold the screen position of the fragment.
//!
//! A varying is a screen varying when it equals `(0.5 * Px + 0.5 * Pw, -0.5 * Py + 0.5 * Pw, Pw)` for the position
//! `(Px, Py, Pz, Pw)` the shader writes. A projective sample of such a varying reads the texel of the pixel itself.
//! The check follows the values as linear forms over the three components, with `fma`, additions, subtractions and
//! the products and quotients with a constant.

use super::facts_vs_sym::{Node, Sym, Trace};
use super::io::Plan;
use std::collections::BTreeMap;

/// A linear form: the coefficient of `x`, `y`, `w` and of the constant `1`.
type Lin = BTreeMap<char, f64>;

fn add(a: &Lin, b: &Lin, s: f64) -> Lin {
    let mut r = a.clone();
    for (k, v) in b {
        *r.entry(*k).or_insert(0.0) += s * v;
    }
    r.into_iter().filter(|(_, v)| *v != 0.0).collect()
}

fn scale(a: &Lin, c: f64) -> Lin {
    a.iter()
        .filter(|(_, v)| **v * c != 0.0)
        .map(|(k, v)| (*k, v * c))
        .collect()
}

fn constant(a: &Lin) -> Option<f64> {
    a.keys()
        .all(|k| *k == '1')
        .then(|| a.get(&'1').copied().unwrap_or(0.0))
}

fn mul(a: &Lin, b: &Lin) -> Option<Lin> {
    match (constant(a), constant(b)) {
        (Some(c), _) => Some(scale(b, c)),
        (_, Some(c)) => Some(scale(a, c)),
        _ => None,
    }
}

/// The linear form of `s`. A value equal to one of the `atoms` is that component.
fn lin(s: &Sym, atoms: &[(char, &Sym)]) -> Option<Lin> {
    if let Some((c, _)) = atoms.iter().find(|(_, a)| **a == *s) {
        return Some([(*c, 1.0)].into_iter().collect());
    }
    let l = |x: &Sym| lin(x, atoms);
    match &**s {
        Node::Konst(b) => {
            let v = f64::from(f32::from_bits(*b));
            Some(if v == 0.0 {
                Lin::new()
            } else {
                [('1', v)].into_iter().collect()
            })
        }
        Node::Add(a, b) => Some(add(&l(a)?, &l(b)?, 1.0)),
        Node::Sub(a, b) => Some(add(&l(a)?, &l(b)?, -1.0)),
        Node::Neg(a) => Some(scale(&l(a)?, -1.0)),
        Node::Mul(a, b) => mul(&l(a)?, &l(b)?),
        Node::Div(a, b) => {
            let d = constant(&l(b)?).filter(|d| *d != 0.0)?;
            Some(scale(&l(a)?, 1.0 / d))
        }
        Node::Fma(a, b, c) => Some(add(&mul(&l(a)?, &l(b)?)?, &l(c)?, 1.0)),
        _ => None,
    }
}

/// The wanted form with a constant `w` folded into the constant term.
fn fold(f: &[(char, f64)], w: Option<f64>) -> Lin {
    let mut r = Lin::new();
    for (k, v) in f {
        let (k, v) = match (k, w) {
            ('w', Some(c)) => ('1', v * c),
            _ => (*k, *v),
        };
        *r.entry(k).or_insert(0.0) += v;
    }
    r.into_iter().filter(|(_, v)| *v != 0.0).collect()
}

/// The user output locations that are screen varyings.
pub fn locations(t: &Trace, plan: &Plan) -> Vec<u32> {
    let (Some(px), Some(py), Some(pw)) = (t.pos(0), t.pos(1), t.pos(3)) else {
        return Vec::new();
    };
    // A constant w is not an atom, so that its uses in the varying are constants as well.
    let w = match **pw {
        Node::Konst(b) => Some(f64::from(f32::from_bits(b))),
        _ => None,
    };
    let mut atoms = vec![('x', px), ('y', py)];
    if w.is_none() {
        atoms.push(('w', pw));
    }
    let want = [
        (0, fold(&[('x', 0.5), ('w', 0.5)], w)),
        (1, fold(&[('y', -0.5), ('w', 0.5)], w)),
        (3, fold(&[('w', 1.0)], w)),
    ];
    let mut locs = plan.user_locs.clone();
    locs.sort_unstable();
    locs.dedup();
    locs.retain(|l| {
        want.iter()
            .all(|(c, f)| t.user(*l, *c).and_then(|s| lin(s, &atoms)).as_ref() == Some(f))
    });
    locs
}
