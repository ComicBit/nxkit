//! The pen layer of `make_ext_font.py` (fontTools' `TTGlyphPen`, `ReverseContourPen`, `BoundsPen` and the script's
//! shape helpers), in the same float arithmetic.
#![allow(clippy::unwrap_used, unreachable_pub, missing_docs)]

use super::ttf::Glyph;
use super::ttf_read::{Contour, Kind, P, Seg};

pub const CY: f64 = 350.0;
pub const R: f64 = 450.0;

pub type Pt = (f64, f64, bool);

pub fn area(c: &[P]) -> f64 {
    let n = c.len();
    (0..n)
        .map(|i| c[i].0 * c[(i + n - 1) % n].1 - c[(i + n - 1) % n].0 * c[i].1)
        .sum::<f64>()
        / 2.0
}
pub fn pyround(v: f64) -> f64 {
    v.round_ties_even()
}
pub fn ot_round(v: f64) -> i32 {
    (v + 0.5).floor() as i32
}

/// `TTGlyphPen` (without components, without implied-closing line).
#[derive(Default)]
pub struct Pen {
    pts: Vec<P>,
    on: Vec<bool>,
    ends: Vec<usize>,
}

impl Pen {
    fn closed(&self) -> bool {
        self.pts.is_empty() || self.ends.last() == Some(&(self.pts.len() - 1))
    }
    pub fn replay(&mut self, c: &[Seg]) {
        for s in c {
            match s.kind {
                Kind::Move | Kind::Line => {
                    self.pts.push(s.pts[0].unwrap());
                    self.on.push(true);
                }
                Kind::Q => {
                    let (last, offs) = s.pts.split_last().unwrap();
                    for p in offs {
                        self.pts.push(p.unwrap());
                        self.on.push(false);
                    }
                    if let Some(p) = last {
                        self.pts.push(*p);
                        self.on.push(true);
                    }
                }
            }
        }
        self.close();
    }
    fn close(&mut self) {
        let end = self.pts.len() - 1;
        if end == 0 || self.ends.last().is_some_and(|e| end == e + 1) {
            self.pts.pop();
            self.on.pop();
            return;
        }
        let start = self.ends.last().map_or(0, |e| e + 1);
        let mut end = end;
        if self.pts[start] == self.pts[end] {
            self.pts.pop();
            self.on.pop();
            end -= 1;
        }
        self.ends.push(end);
    }
    pub fn glyph(self) -> Glyph {
        debug_assert!(self.closed());
        Glyph {
            pts: self
                .pts
                .iter()
                .zip(&self.on)
                .map(|(p, &o)| (ot_round(p.0), ot_round(p.1), o))
                .collect(),
            ends: self.ends.iter().map(|&e| e as u16).collect(),
        }
    }
}

/// `contour(pen, pts, hole)` of the script.
pub fn contour(pen: &mut Pen, pts: &[Pt], hole: bool) {
    let mut pts = pts.to_vec();
    let pp: Vec<P> = pts.iter().map(|p| (p.0, p.1)).collect();
    if (area(&pp) > 0.0) != hole {
        pts.reverse();
    }
    let k = pts.iter().position(|p| p.2).unwrap();
    pts.rotate_left(k);
    let p = |q: &Pt| Some((q.0, q.1));
    let mut c: Contour = vec![Seg {
        kind: Kind::Move,
        pts: vec![p(&pts[0])],
    }];
    let mut offs: Vec<Option<P>> = vec![];
    for q in &pts[1..] {
        if !q.2 {
            offs.push(p(q));
            continue;
        }
        if offs.is_empty() {
            c.push(Seg {
                kind: Kind::Line,
                pts: vec![p(q)],
            });
        } else {
            let mut v = std::mem::take(&mut offs);
            v.push(p(q));
            c.push(Seg {
                kind: Kind::Q,
                pts: v,
            });
        }
    }
    if !offs.is_empty() {
        offs.push(p(&pts[0]));
        c.push(Seg {
            kind: Kind::Q,
            pts: offs,
        });
    }
    pen.replay(&c);
}

pub fn circle(cx: f64, cy: f64, r: f64) -> Vec<Pt> {
    let n = 8.0;
    let mut v = vec![];
    for i in 0..8 {
        let a = 2.0 * std::f64::consts::PI * i as f64 / n;
        let b = a + std::f64::consts::PI / n;
        let rr = r / (std::f64::consts::PI / n).cos();
        v.push((pyround(cx + r * a.cos()), pyround(cy + r * a.sin()), true));
        v.push((
            pyround(cx + rr * b.cos()),
            pyround(cy + rr * b.sin()),
            false,
        ));
    }
    v
}
pub fn rrect(x0: f64, y0: f64, x1: f64, y1: f64, rad: f64) -> Vec<Pt> {
    let rad = rad.min((x1 - x0) / 2.0).min((y1 - y0) / 2.0);
    vec![
        (x0 + rad, y1, true),
        (x1 - rad, y1, true),
        (x1, y1, false),
        (x1, y1 - rad, true),
        (x1, y0 + rad, true),
        (x1, y0, false),
        (x1 - rad, y0, true),
        (x0 + rad, y0, true),
        (x0, y0, false),
        (x0, y0 + rad, true),
        (x0, y1 - rad, true),
        (x0, y1, false),
    ]
}
pub fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Pt> {
    vec![
        (x0, y1, true),
        (x1, y1, true),
        (x1, y0, true),
        (x0, y0, true),
    ]
}
pub fn rot(pts: &[Pt], cx: f64, cy: f64, deg: f64) -> Vec<Pt> {
    let a = deg.to_radians();
    let (c, s) = (a.cos(), a.sin());
    pts.iter()
        .map(|&(x, y, on)| {
            (
                pyround(cx + (x - cx) * c - (y - cy) * s),
                pyround(cy + (x - cx) * s + (y - cy) * c),
                on,
            )
        })
        .collect()
}
pub fn cross(cx: f64, cy: f64, half: f64, arm: f64) -> Vec<Pt> {
    let (a, h) = (arm / 2.0, half);
    [
        (-a, h),
        (a, h),
        (a, a),
        (h, a),
        (h, -a),
        (a, -a),
        (a, -h),
        (-a, -h),
        (-a, -a),
        (-h, -a),
        (-h, a),
        (-a, a),
    ]
    .iter()
    .map(|&(x, y)| (cx + x, cy + y, true))
    .collect()
}

pub fn transform(c: &[Seg], m: (f64, f64, f64, f64, f64, f64)) -> Contour {
    let (xx, xy, yx, yy, dx, dy) = m;
    c.iter()
        .map(|s| Seg {
            kind: s.kind,
            pts: s
                .pts
                .iter()
                .map(|p| p.map(|(x, y)| (xx * x + yx * y + dx, xy * x + yy * y + dy)))
                .collect(),
        })
        .collect()
}

/// `reversedContour` for a closed contour.
pub fn reversed(mut c: Contour) -> Result<Contour, String> {
    let mut first = c.remove(0);
    let first_on = *first.pts.last().unwrap();
    if first.kind == Kind::Q {
        if first_on.is_some() || !c.is_empty() {
            return Err("off-curve contour".into());
        }
        let n = first.pts.len();
        let mut v = vec![first.pts[0]];
        v.extend(first.pts[1..n - 1].iter().rev().copied());
        v.push(None);
        first.pts = v;
    }
    if c.is_empty() {
        if first.kind == Kind::Move {
            return Err("single point contour".into());
        }
        return Ok(vec![first]);
    }
    let last = c.last().unwrap().clone();
    let last_on = *last.pts.last().unwrap();
    let mut out = vec![first.clone()];
    if first_on != last_on {
        out.push(Seg {
            kind: Kind::Line,
            pts: vec![last_on],
        });
        let n = c.len() - 1;
        let mut p = last.pts[..last.pts.len() - 1].to_vec();
        p.push(first_on);
        c[n].pts = p;
    }
    let second = if c.len() > 1 {
        c[0].clone()
    } else {
        last.clone()
    };
    if second.kind == Kind::Line && first.pts != second.pts {
        c.remove(0);
        if !c.is_empty() {
            let n = c.len() - 1;
            let mut p = last.pts[..last.pts.len() - 1].to_vec();
            p.extend(second.pts.iter().copied());
            c[n].pts = p;
        }
    }
    let n = c.len();
    for i in (0..n).rev() {
        let cur = &c[i];
        let next = if i == 0 { &c[n - 1] } else { &c[i - 1] };
        let mut p: Vec<Option<P>> = cur.pts[..cur.pts.len() - 1].iter().rev().copied().collect();
        p.push(*next.pts.last().unwrap());
        out.push(Seg {
            kind: cur.kind,
            pts: p,
        });
    }
    Ok(out)
}

/// `BoundsPen` over contours drawn with the pen protocol.
pub fn bounds_of(contours: &[Contour]) -> Option<(f64, f64, f64, f64)> {
    let mut b: Option<(f64, f64, f64, f64)> = None;
    let upd = |b: Option<(f64, f64, f64, f64)>, p: P| match b {
        None => Some((p.0, p.1, p.0, p.1)),
        Some(b) => Some((b.0.min(p.0), b.1.min(p.1), b.2.max(p.0), b.3.max(p.1))),
    };
    for c in contours {
        let mut cur: P = (0.0, 0.0);
        for s in c {
            let all: Vec<P> = s.pts.iter().flatten().copied().collect();
            match s.kind {
                Kind::Move => {
                    cur = all[0];
                    b = upd(b, cur);
                }
                Kind::Line => {
                    cur = all[0];
                    b = upd(b, cur);
                }
                Kind::Q => {
                    let mut pts = all.clone();
                    if s.pts.last().unwrap().is_none() {
                        let (l, f) = (pts[pts.len() - 1], pts[0]);
                        cur = (0.5 * (l.0 + f.0), 0.5 * (l.1 + f.1));
                        b = upd(b, cur);
                        pts.push(cur);
                    }
                    let n = pts.len() - 1;
                    let mut segs = vec![];
                    for i in 0..n.saturating_sub(1) {
                        let (p, q) = (pts[i], pts[i + 1]);
                        segs.push((p, (0.5 * (p.0 + q.0), 0.5 * (p.1 + q.1))));
                    }
                    segs.push((pts[n - 1], pts[n]));
                    for (bcp, pt) in segs {
                        b = upd(b, pt);
                        let r = b.unwrap();
                        if !(r.0 <= bcp.0 && bcp.0 <= r.2 && r.1 <= bcp.1 && bcp.1 <= r.3) {
                            let q = quad_bounds(cur, bcp, pt);
                            b = Some((r.0.min(q.0), r.1.min(q.1), r.2.max(q.2), r.3.max(q.3)));
                        }
                        cur = pt;
                    }
                }
            }
        }
    }
    b
}

pub fn quad_bounds(p1: P, p2: P, p3: P) -> (f64, f64, f64, f64) {
    let (bx, by) = ((p2.0 - p1.0) * 2.0, (p2.1 - p1.1) * 2.0);
    let (ax, ay) = (p3.0 - p1.0 - bx, p3.1 - p1.1 - by);
    let mut roots = vec![];
    if ax * 2.0 != 0.0 {
        roots.push(-bx / (ax * 2.0));
    }
    if ay * 2.0 != 0.0 {
        roots.push(-by / (ay * 2.0));
    }
    let mut pts: Vec<P> = roots
        .iter()
        .filter(|&&t| (0.0..1.0).contains(&t))
        .map(|&t| (ax * t * t + bx * t + p1.0, ay * t * t + by * t + p1.1))
        .collect();
    pts.push(p1);
    pts.push(p3);
    let f = |g: fn(f64, f64) -> f64, i: usize, init: f64| {
        pts.iter()
            .fold(init, |a, p| g(a, if i == 0 { p.0 } else { p.1 }))
    };
    (
        f(f64::min, 0, f64::INFINITY),
        f(f64::min, 1, f64::INFINITY),
        f(f64::max, 0, f64::NEG_INFINITY),
        f(f64::max, 1, f64::NEG_INFINITY),
    )
}
