//! Image loads, stores and atomics.

use super::args::SET_NAME;
use super::emit::{Em, R, Val};
use super::inst_tex::{dimensions, result_type, swizzle, vec_of};
use super::types::{literal, msl_type};
use crate::ir::Inst;
use crate::structured::ast::{AstTex, OpData};
use crate::structured::types::{AggType, format_component_type};
use crate::types::SamplerType;

const ATOMIC_MASK: u32 = 15 << 16;
const BOUNDS_MASK: u32 = 3 << 20;
const PREDICATE_RESULT: u32 = 1 << 24;
const FORMAT_R32_UINT: usize = 11;
const FORMAT_R32_SINT: usize = 12;

impl Em<'_> {
    /// The image object of an operation, reading its index source.
    fn image_ref(&mut self, o: &OpData, i: &mut usize, t: &AstTex) -> R<String> {
        let tx = self
            .res
            .imgs
            .get(&(t.set, t.binding))
            .ok_or("image is not in the argument buffer")?;
        let name = tx.name.clone();
        if tx.def.array_length != 1 {
            let idx = self.tsrc(o, i, AggType::S32)?;
            Ok(format!("{SET_NAME}.{name}[{}]", idx.s))
        } else {
            Ok(format!("{SET_NAME}.{name}"))
        }
    }

    /// The coordinate arguments of an image access: the coordinate and the array layer.
    fn image_coords(&mut self, o: &OpData, i: &mut usize, t: &AstTex) -> R<Vec<String>> {
        if t.ty.0 & SamplerType::MASK == 5 {
            return Err("unported: cube image access".into());
        }
        let dims = dimensions(t.ty)?;
        let mut c = Vec::new();
        for _ in 0..dims {
            c.push(self.tsrc(o, i, AggType::S32)?.s);
        }
        let mut out = vec![vec_of("uint", &c)];
        if t.ty.0 & SamplerType::ARRAY != 0 {
            out.push(format!("uint({})", self.tsrc(o, i, AggType::S32)?.s));
        }
        Ok(out)
    }

    /// Image instructions.
    pub(super) fn image(&mut self, inst: Inst, o: &OpData) -> R<Option<Val>> {
        let t = o.tex.clone().ok_or("not a texture operation")?;
        let comp = format_component_type(t.format);
        let mut i = 0;
        match inst {
            Inst::ImageLoad => {
                let img = self.image_ref(o, &mut i, &t)?;
                let c = self.image_coords(o, &mut i, &t)?;
                let s = format!("{img}.read({})", c.join(", "));
                let ty = result_type(o.index, comp);
                Ok(Some(Val::new(format!("{s}{}", swizzle(o.index)), ty)))
            }
            Inst::ImageStore => {
                let img = self.image_ref(o, &mut i, &t)?;
                let c = self.image_coords(o, &mut i, &t)?;
                let mut texel = Vec::new();
                for _ in 0..4 {
                    if i < o.sources.len() {
                        texel.push(self.tsrc(o, &mut i, comp)?.s);
                    } else {
                        texel.push(literal(comp, 0));
                    }
                }
                let ty = msl_type(comp | AggType::VECTOR4)?;
                self.line(&format!(
                    "{img}.write({ty}({}), {});",
                    texel.join(", "),
                    c.join(", ")
                ));
                Ok(None)
            }
            _ => self.image_atomic(o, &t).map(Some),
        }
    }

    fn image_atomic(&mut self, o: &OpData, t: &AstTex) -> R<Val> {
        if !self.props.host.metal_image_atomics {
            return Err("unported: image atomics without Metal image atomics".into());
        }
        if t.flags & (BOUNDS_MASK | PREDICATE_RESULT) != 0 {
            return Err("unported: image atomic with trap semantics".into());
        }
        let ty = t.ty;
        let ok_type = ty == SamplerType::TEXTURE_BUFFER
            || ty == SamplerType::TEXTURE_2D
            || ty.0 == (SamplerType::TEXTURE_2D.0 | SamplerType::ARRAY)
            || ty == SamplerType::TEXTURE_3D;
        let array = self
            .res
            .imgs
            .get(&(t.set, t.binding))
            .is_some_and(|x| x.def.array_length != 1);
        if array || !ok_type || (t.format != FORMAT_R32_UINT && t.format != FORMAT_R32_SINT) {
            return Err("unported: image atomic on this image".into());
        }
        let comp = format_component_type(t.format);
        let cty = msl_type(comp)?;
        let mut i = 0;
        let img = self.image_ref(o, &mut i, t)?;
        let c = self.image_coords(o, &mut i, t)?.join(", ");
        let value = self.tsrc(o, &mut i, comp)?.s;
        let f = match t.flags & ATOMIC_MASK {
            x if x == 1 << 16 => "min",
            x if x == 2 << 16 => "max",
            x if x == 3 << 16 || x == 0 => "add",
            x if x == 4 << 16 => "sub",
            x if x == 5 << 16 => "and",
            x if x == 6 << 16 => "or",
            x if x == 7 << 16 => "xor",
            x if x == 8 << 16 => "exchange",
            _ => "compare",
        };
        let v4 = format!("{cty}4");
        if f == "compare" {
            let cmp = self.tsrc(o, &mut i, comp)?.s;
            let n = self.tmp;
            self.tmp += 1;
            self.pre.push(format!("{v4} t{n} = {v4}({cmp});"));
            self.pre.push(format!(
                "{img}.atomic_compare_exchange_weak({c}, &t{n}, {v4}({value}));"
            ));
            return Ok(Val::new(format!("t{n}.x"), comp));
        }
        let one = matches!(t.flags & ATOMIC_MASK, x if x == 3 << 16 || x == 4 << 16);
        let arg = if one { literal(comp, 1) } else { value };
        let call = format!("{img}.atomic_fetch_{f}({c}, {v4}({arg})).x");
        let v = self.effect(call, comp)?;
        Ok(v)
    }
}
