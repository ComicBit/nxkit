//! LLVM spelling of types and constants (typed pointers, LLVM 14 syntax).

use std::fmt::Write;

use msl_sema::types::{is_float, is_signed, Scalar, Space, Type, Types};

/// AIR address space number.
pub fn space_num(sp: Space) -> u32 {
    match sp {
        Space::Thread => 0,
        Space::Device => 1,
        Space::Constant => 2,
        Space::Threadgroup => 3,
    }
}

/// `addrspace(n)` suffix for a pointer (empty for 0).
pub fn space_suffix(n: u32) -> String {
    if n == 0 {
        String::new()
    } else {
        format!(" addrspace({n})")
    }
}

/// LLVM scalar type. `mem` selects the in-memory form (`bool` is `i8` in memory, `i1` as a value).
pub fn scalar_ty(s: Scalar, mem: bool) -> &'static str {
    match s {
        Scalar::Bool => {
            if mem {
                "i8"
            } else {
                "i1"
            }
        }
        Scalar::Char | Scalar::Uchar => "i8",
        Scalar::Short | Scalar::Ushort => "i16",
        Scalar::Int | Scalar::Uint => "i32",
        Scalar::Long | Scalar::Ulong | Scalar::SizeT => "i64",
        Scalar::Half => "half",
        Scalar::Float => "float",
    }
}

/// Bit width of a scalar's value form.
pub fn scalar_bits(s: Scalar) -> u32 {
    if s == Scalar::Bool {
        1
    } else {
        s.size() * 8
    }
}

/// LLVM type of a value or memory form. Aggregates always use the memory form inside.
pub fn llty(types: &Types, t: &Type, mem: bool) -> String {
    match t {
        Type::Void => "void".into(),
        Type::Scalar(s) => scalar_ty(*s, mem).into(),
        Type::Vector(s, n) => format!("<{} x {}>", n, scalar_ty(*s, mem)),
        Type::Packed(s, n) => {
            if mem {
                format!("[{} x {}]", n, scalar_ty(*s, true))
            } else {
                format!("<{} x {}>", n, scalar_ty(*s, false))
            }
        }
        Type::Matrix(s, c, r) => format!("[{} x <{} x {}>]", c, r, scalar_ty(*s, true)),
        Type::Struct(id) => format!("%struct.{}", types.structs[*id].name),
        Type::Texture(tx) if mem => format!("%\"struct.metal::{}\"", tx.kind.name()),
        Type::Sampler if mem => "%\"struct.metal::sampler\"".into(),
        Type::Array(e, n) => format!("[{} x {}]", n, llty(types, e, true)),
        Type::Ptr(sp, e) => {
            let inner = match &**e {
                Type::Void => "i8".to_string(),
                e => llty(types, e, true),
            };
            format!("{}{}*", inner, space_suffix(space_num(*sp)))
        }
        Type::Texture(tx) => format!("%struct._{}_t addrspace(1)*", tx.kind.air_name()),
        Type::Sampler => "%struct._sampler_t addrspace(2)*".into(),
    }
}

/// Converts an f32 to IEEE half bits, rounding to nearest even.
pub fn f32_to_f16(v: f32) -> u16 {
    let x = v.to_bits();
    let sign = ((x >> 16) & 0x8000) as u16;
    let exp = ((x >> 23) & 0xff) as i32;
    let man = x & 0x7f_ffff;
    if exp == 0xff {
        return sign
            | 0x7c00
            | if man != 0 {
                0x200 | (man >> 13) as u16
            } else {
                0
            };
    }
    let e = exp - 127 + 15;
    if e >= 0x1f {
        return sign | 0x7c00;
    }
    if e <= 0 {
        if e < -10 {
            return sign;
        }
        let m = man | 0x80_0000;
        let shift = (14 - e) as u32;
        let half = 1u32 << (shift - 1);
        let rest = m & ((1 << shift) - 1);
        let mut r = m >> shift;
        if rest > half || (rest == half && r & 1 == 1) {
            r += 1;
        }
        return sign | r as u16;
    }
    let mut r = ((e as u32) << 10) | (man >> 13);
    let rest = man & 0x1fff;
    if rest > 0x1000 || (rest == 0x1000 && r & 1 == 1) {
        r += 1;
    }
    sign | r as u16
}

/// A float constant in LLVM's exact hexadecimal form.
pub fn float_const(v: f64, s: Scalar) -> String {
    match s {
        Scalar::Half => format!("0xH{:04X}", f32_to_f16(v as f32)),
        _ => format!("0x{:016X}", ((v as f32) as f64).to_bits()),
    }
}

/// An integer constant of scalar type `s` (value form.`bool` prints `true`/`false`).
pub fn int_const(v: i128, s: Scalar, mem: bool) -> String {
    if s == Scalar::Bool {
        let b = v != 0;
        return if mem {
            (b as u8).to_string()
        } else {
            b.to_string()
        };
    }
    let bits = s.size() * 8;
    let mask: i128 = (1i128 << bits) - 1;
    let mut x = v & mask;
    if x >> (bits - 1) & 1 == 1 {
        x -= 1i128 << bits;
    }
    x.to_string()
}

/// A scalar constant of type `s` from an integer or float value.
pub fn scalar_const(v: &CVal, s: Scalar, mem: bool) -> String {
    match v {
        CVal::Int(i) => {
            if is_float(s) {
                float_const(*i as f64, s)
            } else {
                int_const(*i, s, mem)
            }
        }
        CVal::Float(f) => {
            if is_float(s) {
                float_const(*f, s)
            } else {
                int_const(*f as i128, s, mem)
            }
        }
        CVal::Vec(_) | CVal::Zero => "0".into(),
    }
}

/// A folded constant.
#[derive(Clone, Debug, PartialEq)]
pub enum CVal {
    /// Integer (or bool).
    Int(i128),
    /// Float.
    Float(f64),
    /// Vector components.
    Vec(Vec<CVal>),
    /// Zero of any type.
    Zero,
}

/// Spells a constant of type `t` (memory or value form).
pub fn const_text(types: &Types, v: &CVal, t: &Type, mem: bool) -> String {
    match (t, v) {
        (_, CVal::Zero) => zero_text(types, t, mem),
        (Type::Scalar(s), v) => scalar_const(v, *s, mem),
        (Type::Vector(s, n), CVal::Vec(items)) | (Type::Packed(s, n), CVal::Vec(items)) => {
            let et = scalar_ty(*s, mem);
            let open = if matches!(t, Type::Packed(..)) && mem {
                '['
            } else {
                '<'
            };
            let close = if open == '[' { ']' } else { '>' };
            let mut out = String::new();
            out.push(open);
            for i in 0..*n as usize {
                if i > 0 {
                    out.push_str(", ");
                }
                let c = items.get(i).cloned().unwrap_or(CVal::Zero);
                let _ = write!(out, "{} {}", et, scalar_const(&c, *s, mem));
            }
            out.push(close);
            out
        }
        (Type::Vector(s, n), v) | (Type::Packed(s, n), v) => const_text(
            types,
            &CVal::Vec(vec![v.clone(); *n as usize]),
            &Type::Vector(*s, *n),
            mem,
        ),
        _ => zero_text(types, t, mem),
    }
}

/// The zero constant of a type.
pub fn zero_text(_types: &Types, t: &Type, mem: bool) -> String {
    match t {
        Type::Scalar(s) => {
            if is_float(*s) {
                float_const(0.0, *s)
            } else {
                int_const(0, *s, mem)
            }
        }
        Type::Ptr(..) | Type::Texture(_) | Type::Sampler => "null".into(),
        _ => "zeroinitializer".into(),
    }
}

/// Intrinsic type suffix: `f32`, `v4f32`, `i32`, `v2i1`, `f16` ...
pub fn suffix(t: &Type) -> String {
    let (s, n) = match t {
        Type::Scalar(s) => (*s, 1),
        Type::Vector(s, n) | Type::Packed(s, n) => (*s, *n),
        _ => return "x".into(),
    };
    let base = match s {
        Scalar::Half => "f16".to_string(),
        Scalar::Float => "f32".to_string(),
        s => format!("i{}", scalar_bits(s)),
    };
    if n == 1 {
        base
    } else {
        format!("v{n}{base}")
    }
}

/// The `s`/`u`/`f` kind letter of a scalar for `air.convert`.
pub fn kind_letter(s: Scalar) -> &'static str {
    if is_float(s) {
        "f"
    } else if is_signed(s) {
        "s"
    } else {
        "u"
    }
}

/// Escapes a string for an LLVM metadata string or name.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b == b'"' || b == b'\\' || !(0x20..0x7f).contains(&b) {
            let _ = write!(out, "\\{b:02X}");
        } else {
            out.push(b as char);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_conversion() {
        assert_eq!(f32_to_f16(1.0), 0x3c00);
        assert_eq!(f32_to_f16(-2.0), 0xc000);
        assert_eq!(f32_to_f16(65504.0), 0x7bff);
        assert_eq!(f32_to_f16(1e6), 0x7c00);
        assert_eq!(f32_to_f16(5.96e-8), 0x0001);
        assert_eq!(f32_to_f16(0.0), 0);
    }

    #[test]
    fn constants() {
        assert_eq!(float_const(1.0, Scalar::Float), "0x3FF0000000000000");
        assert_eq!(int_const(4294966296, Scalar::Uint, false), "-1000");
        assert_eq!(int_const(1, Scalar::Bool, false), "true");
        assert_eq!(int_const(1, Scalar::Bool, true), "1");
        let t = Types::default();
        assert_eq!(llty(&t, &Type::Vector(Scalar::Bool, 4), true), "<4 x i8>");
        assert_eq!(
            llty(&t, &Type::Ptr(Space::Device, Type::UINT.into()), false),
            "i32 addrspace(1)*"
        );
        assert_eq!(suffix(&Type::Vector(Scalar::Float, 4)), "v4f32");
        assert_eq!(suffix(&Type::Vector(Scalar::Bool, 2)), "v2i1");
    }
}
