//! Textures of `InstEmitAmpereMem.cs`: TEX, TLD, TLD4, TMML, TXD, TXQ.

use super::operand::*;
use crate::accessor::SetBinding;
use crate::emit::mem::tex::{DEFAULT_CBUF_SLOT, FORMAT_UNKNOWN, convert_sampler_type_dim, is_1d};
use crate::ir::Opd;
use crate::ir::inst::Inst;
use crate::ir::ops::{regtype, texflags};
use crate::maxwell::ampere::AmpOp;
use crate::maxwell::enums::TexDim;
use crate::translation::props::{TextureFlags, dimensions};
use crate::types::SamplerType;

fn c(ctx: &mut Ctx, v: i32) -> Opd {
    ctx.ir.const_(v)
}

fn cf(ctx: &mut Ctx, v: f32) -> Opd {
    ctx.ir.const_f(v)
}

/// `ATexDim`: texture dimension (61..64) to Maxwell's `TexDim`.
fn tex_dim(ctx: &Ctx, dim: i32) -> Result<TexDim, String> {
    Ok(match dim {
        0 => TexDim::_1d,
        4 => TexDim::Array1d,
        1 => TexDim::_2d,
        5 => TexDim::Array2d,
        2 => TexDim::_3d,
        3 => TexDim::Cube,
        7 => TexDim::ArrayCube,
        _ => return Err(fail(ctx, &format!("texture dimension {dim}"))),
    })
}

/// `ATexHandle`: the constant buffer word that holds the texture handle, and its bank.
pub(super) fn tex_handle(ctx: &Ctx, x: &AmpOp) -> (i32, i32) {
    let (bank, offset) = map_c0(ctx, x.bits(54, 59), x.bits(40, 54) * 4);
    (
        offset >> 2,
        if bank == 2 { DEFAULT_CBUF_SLOT } else { bank },
    )
}

/// `ATexBlocks`.
fn tex_blocks(ctx: &mut Ctx, x: &AmpOp, n0: i32, n1: i32) -> Result<(Vec<Opd>, Vec<Opd>), String> {
    let mut seq: Vec<Opd> = Vec::new();
    let mut take = |ctx: &mut Ctx, reg: i32, n: i32| {
        for k in 0..n {
            let v = if reg + k >= RZ {
                ctx.ir.const_(0)
            } else {
                let r = ctx.ir.register(reg + k, regtype::GPR);
                ctx.copy(r)
            };
            seq.push(v);
        }
    };
    if x.bit(60) {
        let total = n0 + n1;
        if total > 4 {
            return Err(fail(ctx, "texture .SCR with more than four sources"));
        }
        let k0 = if total >= 3 { 2 } else { 1 };
        take(ctx, x.ra(), k0.min(total));
        take(ctx, x.rb(), total - k0.min(total));
    } else {
        take(ctx, x.ra(), n0);
        take(ctx, x.rb(), n1);
    }
    let b = seq.split_off(n0 as usize);
    Ok((seq, b))
}

/// `ATexResults`.
fn tex_results(ctx: &mut Ctx, x: &AmpOp, results: &[Opd], f16: bool) {
    let d0 = x.rd();
    let d1 = x.rc();
    let split = d1 != RZ;
    let write = |ctx: &mut Ctx, reg: i32, value: Opd| {
        if reg < RZ {
            let d = gpr(ctx, reg);
            ctx.copy_to(d, value);
        }
    };
    if d0 == RZ {
        return;
    }
    if f16 {
        let mut pair = 0;
        while pair * 2 < results.len() {
            let lo = results[pair * 2];
            let hi = if pair * 2 + 1 < results.len() {
                results[pair * 2 + 1]
            } else {
                cf(ctx, 0.0)
            };
            let v = ctx.pack_half2x16(lo, hi);
            let reg = if pair == 0 {
                d0
            } else if split {
                d1
            } else {
                d0 + 1
            };
            write(ctx, reg, v);
            pair += 1;
        }
        return;
    }
    for (i, r) in results.iter().enumerate() {
        let i = i as i32;
        let reg = if !split || i < 2 { d0 + i } else { d1 + i - 2 };
        write(ctx, reg, *r);
    }
}

fn locals(ctx: &mut Ctx, n: u32) -> Vec<Opd> {
    (0..n).map(|_| ctx.ir.local()).collect()
}

fn binding(
    ctx: &mut Ctx,
    inst: Inst,
    ty: SamplerType,
    flags: u32,
    cbuf_slot: i32,
    handle: i32,
) -> Result<SetBinding, String> {
    ctx.rm.get_texture_or_image_binding(
        ctx.acc,
        inst,
        ty,
        FORMAT_UNKNOWN,
        TextureFlags(flags),
        cbuf_slot,
        handle,
        1,
        false,
    )
}

pub(super) fn tex(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let mut opcode = x.opcode();
    let handle_in_b = opcode == 0x361;
    let bindless = handle_in_b || opcode == 0xf60;
    let mut bound_handle: Option<Opd> = None;
    if opcode == 0xf60 {
        let ur = x.bits(40, 46);
        bound_handle = Some(if ur == URZ {
            c(ctx, 0)
        } else {
            let r = ctx.ir.register(ur, regtype::UNIFORM);
            ctx.copy(r)
        });
    }
    if bindless {
        opcode = 0xb60;
    }
    let dim = tex_dim(ctx, x.bits(61, 64))?;
    let mut ty = convert_sampler_type_dim(dim)?;
    let is_array = ty.0 & SamplerType::ARRAY != 0;
    let coords = dimensions(ty)?;
    let mask = x.bits(72, 76);
    let f16 = x.bit(79) && opcode != 0xb6c;
    if x.bits(81, 84) != PT && opcode != 0xb69 {
        return Err(fail(ctx, "texture residency predicate"));
    }
    let mut flags = texflags::NONE;
    let mut lod = 0;
    let (mut offset, mut depth, mut ms, mut ptp) = (false, false, false, false);
    let mut n1 = 0;
    match opcode {
        0xb60 => {
            lod = x.bits(87, 90);
            offset = x.bit(76);
            depth = x.bit(78);
            if lod > 3 {
                return Err(fail(ctx, &format!("TEX lod mode {lod}")));
            }
            n1 = i32::from(lod >= 2) + i32::from(offset) + i32::from(depth);
        }
        0xb66 => {
            lod = x.bits(87, 90);
            offset = x.bit(76);
            ms = x.bit(78);
            if lod != 1 && lod != 3 {
                return Err(fail(ctx, &format!("TLD lod mode {lod}")));
            }
            flags |= texflags::INT_COORDS;
            n1 = i32::from(lod == 3) + i32::from(offset) + i32::from(ms);
        }
        0xb63 => {
            let off_mode = x.bits(76, 78);
            depth = x.bit(78);
            if off_mode == 3 {
                return Err(fail(ctx, "TLD4 offset mode 3"));
            }
            offset = off_mode != 0;
            ptp = off_mode == 2;
            flags |= texflags::GATHER;
            n1 = (if off_mode == 2 { 2 } else { off_mode }) + i32::from(depth);
        }
        0xb6c => {
            offset = x.bit(76);
            flags |= texflags::DERIVATIVES;
        }
        _ => {}
    }
    let (handle, cbuf_slot) = if bindless {
        (0, DEFAULT_CBUF_SLOT)
    } else {
        tex_handle(ctx, &x)
    };
    let mut sources: Vec<Opd> = Vec::new();
    let mut array_index: Option<Opd> = None;
    let mut is_1d_to_2d = false;
    if bindless {
        flags |= texflags::BINDLESS;
    }
    if opcode == 0xb6c {
        let n0 = coords + i32::from(is_array || offset);
        let (a, b) = tex_blocks(ctx, &x, n0, 2 * coords)?;
        sources.extend_from_slice(&a[..coords as usize]);
        if is_1d(ty) {
            sources.push(cf(ctx, 0.0));
            ty = SamplerType::TEXTURE_2D.with(ty.0 & SamplerType::ARRAY);
            is_1d_to_2d = true;
        }
        if is_array {
            let m = c(ctx, 0xffff);
            let ai = ctx.bitwise_and(a[coords as usize], m);
            sources.push(ai);
        }
        sources.extend_from_slice(&b);
        if is_1d_to_2d {
            sources.push(cf(ctx, 0.0));
            sources.push(cf(ctx, 0.0));
        }
        if offset {
            for i in 0..coords {
                let o = c(ctx, 16 + i * 4);
                let w = c(ctx, 4);
                let v = ctx.bitfield_extract_s32(a[coords as usize], o, w);
                sources.push(v);
            }
            if is_1d_to_2d {
                sources.push(c(ctx, 0));
            }
            flags |= texflags::OFFSET;
        }
    } else {
        let n0 = coords + i32::from(is_array);
        let (a, b) = tex_blocks(ctx, &x, n0, n1 + i32::from(handle_in_b))?;
        let (mut ia, mut ib) = (0usize, 0usize);
        if handle_in_b {
            bound_handle = Some(b[ib]);
            ib += 1;
        }
        if bindless {
            sources.push(bound_handle.ok_or("TEX: bindless without a handle")?);
        }
        if is_array {
            array_index = Some(a[ia]);
            ia += 1;
        }
        for _ in 0..coords {
            sources.push(a[ia]);
            ia += 1;
        }
        if is_1d(ty) {
            let plain_fetch = opcode == 0xb66 && !is_array && lod == 1 && !offset && !ms;
            if !plain_fetch || ctx.acc.query_sampler_type(handle, -1) != SamplerType::TEXTURE_BUFFER
            {
                if plain_fetch {
                    let fetch_x = *sources.last().ok_or("TEX: no coordinate")?;
                    let m = c(ctx, 0x3FFF);
                    let lo = ctx.bitwise_and(fetch_x, m);
                    if let Some(l) = sources.last_mut() {
                        *l = lo;
                    }
                    let k = c(ctx, 14);
                    let hi = ctx.shift_right_u32(fetch_x, k);
                    sources.push(hi);
                    ctx.acc.record_texel_fetch_1d(handle);
                } else {
                    let z = if opcode == 0xb66 {
                        c(ctx, 0)
                    } else {
                        cf(ctx, 0.0)
                    };
                    sources.push(z);
                }
                ty = SamplerType::TEXTURE_2D.with(ty.0 & SamplerType::ARRAY);
                is_1d_to_2d = true;
            }
        }
        if is_array && let Some(ai) = array_index {
            sources.push(ai);
        }
        let mut lod_value: Option<Opd> = None;
        if (opcode == 0xb60 && lod >= 2) || (opcode == 0xb66 && lod == 3) {
            lod_value = Some(b[ib]);
            ib += 1;
        }
        let off0 = if offset {
            ib += 1;
            Some(b[ib - 1])
        } else {
            None
        };
        let off1 = if ptp {
            ib += 1;
            Some(b[ib - 1])
        } else {
            None
        };
        if depth {
            sources.push(b[ib]);
            ib += 1;
            ty = ty.with(SamplerType::SHADOW);
        }
        if (opcode == 0xb60 || opcode == 0xb66) && !ms && (lod == 1 || lod == 3) {
            let v = match lod_value {
                Some(v) => v,
                None => {
                    if opcode == 0xb66 {
                        c(ctx, 0)
                    } else {
                        cf(ctx, 0.0)
                    }
                }
            };
            sources.push(v);
            flags |= texflags::LOD_LEVEL;
        }
        if offset && opcode != 0xb63 {
            let o0 = off0.ok_or("TEX: offset")?;
            for i in 0..coords {
                let o = c(ctx, i * 4);
                let w = c(ctx, 4);
                let v = ctx.bitfield_extract_s32(o0, o, w);
                sources.push(v);
            }
            if is_1d_to_2d {
                sources.push(c(ctx, 0));
            }
            flags |= texflags::OFFSET;
        } else if offset {
            let texels = if ptp { 4 } else { 1 };
            for i in 0..coords * texels {
                let packed = if (i >> 2) & 1 == 0 { off0 } else { off1 }.ok_or("TEX: offset")?;
                let o = c(ctx, (i & 3) * 8);
                let w = c(ctx, 6);
                let v = ctx.bitfield_extract_s32(packed, o, w);
                sources.push(v);
            }
            flags |= if ptp {
                texflags::OFFSETS
            } else {
                texflags::OFFSET
            };
        }
        if opcode == 0xb60 && lod == 2 {
            sources.push(lod_value.ok_or("TEX: lod")?);
            flags |= texflags::LOD_BIAS;
        }
        if ms {
            sources.push(b[ib]);
            ty = ty.with(SamplerType::MULTISAMPLE);
        }
        if opcode == 0xb63 && !depth {
            sources.push(c(ctx, x.bits(87, 89)));
        }
        if opcode == 0xb66
            && ty == SamplerType::TEXTURE_1D
            && lod == 1
            && !offset
            && ctx.acc.query_sampler_type(handle, -1) == SamplerType::TEXTURE_BUFFER
        {
            ty = SamplerType::TEXTURE_BUFFER;
            sources.pop();
            flags &= !texflags::LOD_LEVEL;
        }
    }
    let count = (mask as u32).count_ones();
    let results = locals(ctx, count);
    if opcode == 0xb69 {
        tmml(ctx, ty, flags, handle, cbuf_slot, mask, &results, &sources)?;
    } else {
        let sb = if bindless {
            SetBinding { set: 0, binding: 0 }
        } else {
            binding(ctx, Inst::TextureSample, ty, flags, cbuf_slot, handle)?
        };
        let dests: Vec<Option<Opd>> = results.iter().map(|r| Some(*r)).collect();
        ctx.texture_sample(ty, flags, sb, mask, &dests, &sources);
    }
    tex_results(ctx, &x, &results, f16);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn tmml(
    ctx: &mut Ctx,
    ty: SamplerType,
    flags: u32,
    handle: i32,
    cbuf_slot: i32,
    mask: i32,
    results: &[Opd],
    sources: &[Opd],
) -> R {
    let sb = binding(ctx, Inst::Lod, ty, flags, cbuf_slot, handle)?;
    let mut r = 0;
    for comp in 0..4 {
        if mask & (1 << comp) == 0 {
            continue;
        }
        if comp >= 2 {
            let z = c(ctx, 0);
            ctx.copy_to(results[r], z);
            r += 1;
            continue;
        }
        let res = ctx.lod(ty, flags, sb, comp ^ 1, sources);
        let k = cf(ctx, 256.0);
        let m = ctx.fp_multiply(res, k);
        let v = ctx.fp32_convert_to_s32(m);
        ctx.copy_to(results[r], v);
        r += 1;
    }
    Ok(())
}

pub(super) fn txq(ctx: &mut Ctx) -> R {
    let x = x(ctx);
    let query = x.bits(62, 64);
    let mask = x.bits(72, 76);
    let bindless = x.opcode() == 0x370;
    let (handle, cbuf_slot) = if bindless {
        (0, DEFAULT_CBUF_SLOT)
    } else {
        tex_handle(ctx, &x)
    };
    let ty = if bindless {
        SamplerType::TEXTURE_2D
    } else {
        ctx.acc.query_sampler_type(handle, -1)
    };
    let flags = if bindless {
        texflags::BINDLESS
    } else {
        texflags::NONE
    };
    let count = (mask as u32).count_ones();
    let results = locals(ctx, count);
    let src = |ctx: &mut Ctx, reg: i32| -> Opd {
        if reg >= RZ {
            ctx.ir.const_(0)
        } else {
            let r = ctx.ir.register(reg, regtype::GPR);
            ctx.copy(r)
        }
    };
    let sources: Vec<Opd> = if bindless {
        let a = src(ctx, x.ra());
        let b = src(ctx, x.ra() + 1);
        vec![a, b]
    } else {
        vec![src(ctx, x.ra())]
    };
    if query == 0 {
        let sb = if bindless {
            SetBinding { set: 0, binding: 0 }
        } else {
            binding(
                ctx,
                Inst::TextureQuerySize,
                ty,
                texflags::NONE,
                cbuf_slot,
                handle,
            )?
        };
        let mut r = 0;
        for comp in 0..4 {
            if mask & (1 << comp) != 0 {
                let v = ctx.texture_query_size(ty, flags, sb, comp, &sources);
                ctx.copy_to(results[r], v);
                r += 1;
            }
        }
    } else if query == 1 {
        let sb = if bindless {
            SetBinding { set: 0, binding: 0 }
        } else {
            binding(
                ctx,
                Inst::TextureQuerySamples,
                ty,
                texflags::NONE,
                cbuf_slot,
                handle,
            )?
        };
        let mut r = 0;
        for comp in 0..4 {
            if mask & (1 << comp) != 0 {
                let v = if comp == 2 {
                    ctx.texture_query_samples(ty, flags, sb, &sources)
                } else {
                    c(ctx, 0)
                };
                ctx.copy_to(results[r], v);
                r += 1;
            }
        }
    } else {
        return Err(fail(ctx, &format!("TXQ query {query}")));
    }
    tex_results(ctx, &x, &results, false);
    Ok(())
}
