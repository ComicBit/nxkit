//! Format and sampler type name tables of the properties.

use crate::types::SamplerType;

/// `TextureFormatExtensions.ToGlslFormat` by format index.
pub fn to_glsl_format(format: usize) -> &'static str {
    const NAMES: [&str; 40] = [
        "",
        "r8",
        "r8_snorm",
        "r8ui",
        "r8i",
        "r16f",
        "r16",
        "r16_snorm",
        "r16ui",
        "r16i",
        "r32f",
        "r32ui",
        "r32i",
        "rg8",
        "rg8_snorm",
        "rg8ui",
        "rg8i",
        "rg16f",
        "rg16",
        "rg16_snorm",
        "rg16ui",
        "rg16i",
        "rg32f",
        "rg32ui",
        "rg32i",
        "rgba8",
        "rgba8_snorm",
        "rgba8ui",
        "rgba8i",
        "rgba16f",
        "rgba16",
        "rgba16_snorm",
        "rgba16ui",
        "rgba16i",
        "rgba32f",
        "rgba32ui",
        "rgba32i",
        "rgb10_a2",
        "rgb10_a2ui",
        "r11f_g11f_b10f",
    ];
    NAMES.get(format).copied().unwrap_or("")
}

/// `SamplerType.Dimensions`: `Err` where the C# throws.
pub fn dimensions(ty: SamplerType) -> Result<i32, String> {
    match ty.0 & SamplerType::MASK {
        1 | 2 => Ok(1),
        3 => Ok(2),
        4 | 5 => Ok(3),
        _ => Err(format!("Invalid sampler type \"{}\".", ty.name())),
    }
}

/// `SamplerType.ShortTypeName`: `Err` where the C# throws.
pub fn short_type_name(ty: SamplerType) -> Result<String, String> {
    let mut s = match ty.0 & SamplerType::MASK {
        1 => "1d",
        2 => "b",
        3 => "2d",
        4 => "3d",
        5 => "cube",
        _ => return Err(format!("Invalid sampler type \"{}\".", ty.name())),
    }
    .to_string();
    if ty.0 & SamplerType::MULTISAMPLE != 0 {
        s.push_str("ms");
    }
    if ty.0 & SamplerType::ARRAY != 0 {
        s.push('a');
    }
    if ty.0 & SamplerType::SHADOW != 0 {
        s.push('s');
    }
    Ok(s)
}
