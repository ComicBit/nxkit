//! Where the coordinate of a texture sample comes from, read from the structured program.
//!
//! The `_bf` library can replace a sample by a read of the framebuffer when the host knows what the coordinate
//! means. Three forms are known: a varying (`in.loc.xy`), a perspective divided varying (`in.loc.xy / in.loc.w`) and
//! a pixel position scaled by two uniforms (`gl_FragCoord.xy [/ render scale] * uniform`). Every variable on the way
//! must have exactly one definition.

use crate::ir::{Inst, IoVariable, OperandType, StorageKind};
use crate::msl::args::Res;
use crate::msl::io::Plan;
use crate::props::Props;
use crate::structured::StructInfo;
use crate::structured::ast::{A, Kind, OpData};
use std::collections::BTreeMap;

/// The bit pattern of 1.0.
const ONE: i32 = 0x3f80_0000;
/// The field of `support_buffer` that holds the render scales.
const RENDER_SCALE_FIELD: i32 = 5;
/// The first driver bank of the pixel stage's uniforms.
const FIRST_BANK: i64 = 3;
const FP32: u32 = Inst::FP32;

/// What a coordinate pair means.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum How {
    /// A varying, with or without the perspective divide.
    Varying {
        /// The user location.
        locn: i32,
        /// The coordinates are divided by `w`.
        proj: bool,
    },
    /// The pixel position times two uniforms: `[slot, vec4, component]` for x and y.
    FragPos([[i64; 3]; 2]),
}

enum N<'a> {
    Const(i32),
    Op(&'a OpData),
}

/// A view of the program for the questions of the fetch variant.
pub struct Tracer<'a> {
    /// The structured program.
    pub info: &'a StructInfo,
    /// The properties of the program.
    pub props: &'a Props,
    /// The resources.
    pub res: &'a Res,
    /// The IO plan.
    pub plan: &'a Plan,
    /// The values assigned to each variable.
    pub defs: &'a BTreeMap<A, Vec<A>>,
}

impl<'a> Tracer<'a> {
    /// The node a value stands for, following variables that have one definition.
    fn node(&self, mut a: A) -> Option<N<'a>> {
        for _ in 0..32 {
            match self.info.ast.kind(a) {
                Kind::Operand {
                    ty: OperandType::LocalVariable,
                    ..
                } => match self.defs.get(&a)?.as_slice() {
                    [one] => a = *one,
                    _ => return None,
                },
                Kind::Operand {
                    ty: OperandType::Constant,
                    value,
                    ..
                } => return Some(N::Const(*value)),
                Kind::Operation(o) => return Some(N::Op(o)),
                _ => return None,
            }
        }
        None
    }

    fn op(&self, a: A, inst: Inst, fp: bool) -> Option<&'a OpData> {
        match self.node(a)? {
            N::Op(o) if Inst(o.inst.0 & Inst::MASK) == inst && (!fp || o.inst.0 & FP32 != 0) => {
                Some(o)
            }
            _ => None,
        }
    }

    fn konst(&self, o: &OpData, i: usize) -> Option<i32> {
        match self.node(*o.sources.get(i)?)? {
            N::Const(v) => Some(v),
            N::Op(_) => None,
        }
    }

    /// The location and component of a read of an input variable.
    fn input(&self, a: A, var: IoVariable) -> Option<(i32, i32)> {
        let o = self.op(a, Inst::Load, false)?;
        if o.storage_kind != StorageKind::Input || self.konst(o, 0)? != var.0 as i32 {
            return None;
        }
        if self
            .props
            .definitions
            .has_per_location_input_or_output(var, false)
        {
            (o.sources.len() == 3).then_some(())?;
            Some((self.konst(o, 1)?, self.konst(o, 2)?))
        } else {
            (o.sources.len() == 2).then_some(())?;
            Some((0, self.konst(o, 1)?))
        }
    }

    fn varying(&self, a: A, comp: i32) -> Option<i32> {
        let (loc, c) = self.input(a, IoVariable::UserDefined)?;
        let known = self.plan.varyings.iter().any(|v| v.location as i32 == loc);
        (c == comp && known).then_some(loc)
    }

    /// The buffer a constant buffer read names and its field.
    fn cbuf(&self, a: A) -> Option<(&'a str, &'a OpData)> {
        let o = self.op(a, Inst::Load, false)?;
        if o.storage_kind != StorageKind::ConstantBuffer || self.konst(o, 1)? < 0 {
            return None;
        }
        let b = self.res.cbs.get(&self.konst(o, 0)?)?;
        Some((b.name.as_str(), o))
    }

    /// `[slot, vec4, component]` of a pixel stage uniform read at constant indices.
    fn uniform(&self, a: A) -> Option<[i64; 3]> {
        let (name, o) = self.cbuf(a)?;
        let bank: i64 = name.strip_prefix("fp_c")?.parse().ok()?;
        if bank < FIRST_BANK || self.konst(o, 1)? != 0 || o.sources.len() != 4 {
            return None;
        }
        let (vec, comp) = (self.konst(o, 2)?, self.konst(o, 3)?);
        (vec >= 0 && (0..4).contains(&comp)).then_some([
            bank - FIRST_BANK,
            i64::from(vec),
            i64::from(comp),
        ])
    }

    fn render_scale(&self, a: A) -> bool {
        let Some((name, o)) = self.cbuf(a) else {
            return false;
        };
        name == "support_buffer"
            && self.konst(o, 1) == Some(RENDER_SCALE_FIELD)
            && o.sources.len() == 3
            && self.konst(o, 2) == Some(0)
    }

    /// The pixel position component, divided by the render scale or not.
    fn pixel(&self, a: A, comp: i32) -> bool {
        let frag = |a: A| self.input(a, IoVariable::FragmentCoord) == Some((0, comp));
        if frag(a) {
            return true;
        }
        match self.op(a, Inst::Divide, true) {
            Some(o) if o.sources.len() == 2 => {
                frag(o.sources[0]) && self.render_scale(o.sources[1])
            }
            _ => false,
        }
    }

    /// The reciprocal of the `w` of a location.
    fn recip_w(&self, a: A, loc: i32) -> bool {
        match self.op(a, Inst::Divide, true) {
            Some(o) if o.sources.len() == 2 => {
                self.konst(o, 0) == Some(ONE) && self.varying(o.sources[1], 3) == Some(loc)
            }
            _ => false,
        }
    }

    /// The location of a coordinate that is `x / w` or `x * (1 / w)` of one varying.
    fn projected(&self, a: A, comp: i32) -> Option<i32> {
        if let Some(o) = self
            .op(a, Inst::Divide, true)
            .filter(|o| o.sources.len() == 2)
        {
            let loc = self.varying(o.sources[0], comp)?;
            return (self.varying(o.sources[1], 3)? == loc).then_some(loc);
        }
        let o = self
            .op(a, Inst::Multiply, true)
            .filter(|o| o.sources.len() == 2)?;
        [(0, 1), (1, 0)].into_iter().find_map(|(x, r)| {
            let loc = self.varying(o.sources[x], comp)?;
            self.recip_w(o.sources[r], loc).then_some(loc)
        })
    }

    /// `[slot, vec4, component]` of the uniform a pixel position component is multiplied by.
    fn scaled(&self, a: A, comp: i32) -> Option<[i64; 3]> {
        let o = self
            .op(a, Inst::Multiply, true)
            .filter(|o| o.sources.len() == 2)?;
        [(0, 1), (1, 0)].into_iter().find_map(|(p, u)| {
            self.pixel(o.sources[p], comp)
                .then(|| self.uniform(o.sources[u]))
                .flatten()
        })
    }

    /// The meaning of the coordinate pair `(x, y)`.
    pub fn how(&self, x: A, y: A) -> Option<How> {
        let (lx, ly) = (self.varying(x, 0), self.varying(y, 1));
        if let (Some(a), Some(b)) = (lx, ly)
            && a == b
        {
            return Some(How::Varying {
                locn: a,
                proj: false,
            });
        }
        if let (Some(a), Some(b)) = (self.projected(x, 0), self.projected(y, 1))
            && a == b
        {
            return Some(How::Varying {
                locn: a,
                proj: true,
            });
        }
        let (fx, fy) = (self.scaled(x, 0)?, self.scaled(y, 1)?);
        Some(How::FragPos([fx, fy]))
    }
}
