//! Replay reflection (ports of `dmsh_build.reflect`, `decode_tex`, `const_sampler`, `decode_ubo_tex`, `c2_array`,
//! `drv_reads`, `unfillable`, the pixel part of `raster_facts`, and the `entry` / `vertex_inputs` fields).

use crate::msl::border_member;
use nxb_pyjson::Value;
use std::collections::BTreeSet;

mod handles;
mod raster_vs;
use handles::{c2_array, const_sampler, decode_tex, decode_ubo_img, decode_ubo_tex, is_image};
mod screen;
pub use raster_vs::raster_vs;
pub use screen::screen_varyings;

const C2_ROW: i64 = 8;
const C2_TEX: i64 = 32;
const C2_IMG: i64 = 288;
const C2_IMG_SLOTS: i64 = 8;
const C2_SEPTEX: i64 = 360;
const SEPTEX_SLOT0: i64 = 128;
const C2_SAMP: i64 = 1384;
const C2_SEP_SLOTS: i64 = 128;
const C0_ROW: i64 = 16;
const C0_SSBO_SLOTS: i64 = 16;
const C0_CS_UBO: i64 = 528;
const C0_CS_UBO_SLOTS: i64 = 16;
const GMEM_MAP_CB_SLOT: i64 = 255;
const GMEM_LOG_CB_OFFSET: i64 = 1;
const C0_SAMPLE_TABLE: i64 = 1568;
const C0_SAMPLE_BYTES: i64 = 16;
const C0_SAMPLE_INFO: i64 = 1584;

/// `SSBO[stage]`: the storage buffer address table's base in c0.
fn ssbo(stage: &str) -> Option<i64> {
    Some(match stage {
        "vs" => 272,
        "cs" => 784,
        "gs" => 1040,
        "ps" => 1296,
        "ts" => 1552,
        "ms" => 1808,
        _ => return None,
    })
}

/// `dmsh_geometry.HELPERS`.
fn geometry_helper(nm: &str) -> Option<i64> {
    Some(match nm {
        "vertex_output" => 0,
        "geometry_vb_output" | "vb_input" => 1,
        "geometry_ib_output" => 2,
        "vb_info" => 3,
        "trb_data" => 4,
        _ => return None,
    })
}

pub(crate) fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `re.fullmatch(r'[a-z]+_<letter>(\d+)', nm)`.
fn bank_name(nm: &str, letter: char) -> Option<i64> {
    let low = nm.bytes().take_while(u8::is_ascii_lowercase).count();
    let r = nm[low..].strip_prefix('_')?.strip_prefix(letter)?;
    (low > 0 && !r.is_empty() && r.bytes().all(|c| c.is_ascii_digit())).then(|| r.parse().ok())?
}

fn word_followed(src: &str, nm: &str, tail: &str) -> bool {
    let pat = format!("{nm}{tail}");
    src.match_indices(&pat)
        .any(|(i, _)| !src[..i].chars().next_back().is_some_and(is_word))
}

fn int(v: i64) -> Value {
    Value::Int(v)
}

/// `reflect(key, stage, msl, res)` → the bindings list.
pub fn reflect(key: &str, stage: &str, msl: &str, res: &Value) -> Result<Value, String> {
    let group = crate::msl::descriptor_group(msl);
    let body = group.map_or("", |(a, b)| &msl[a..b]);
    let members: Vec<(String, String, u64)> = body.split('\n').filter_map(border_member).collect();
    if group.is_some() && members.len() != body.matches("[[id(").count() {
        return Err(format!(
            "{key}: an argument buffer member the reflection cannot read"
        ));
    }
    let empty = Vec::new();
    let fetch1d: Vec<i64> = res
        .get("fetch1d")
        .and_then(Value::as_list)
        .unwrap_or(&empty)
        .iter()
        .filter_map(Value::as_i64)
        .collect();
    let mut binds: Vec<Value> = Vec::new();
    for (ty, orig, aid) in members {
        let mut e = Value::obj([
            ("msl_name", Value::str(orig.clone())),
            ("msl_type", Value::str(ty.clone())),
            ("arg_id", int(aid as i64)),
        ]);
        if ty == format!("constant spvPool_{orig}*") {
            return Err(format!("{key}: pool arrays are not ported yet"));
        }
        let mut nm = orig.clone();
        if ty == "sampler" && nm.ends_with("Smplr_w") {
            nm.truncate(nm.len() - 2);
            let n = binds
                .iter()
                .filter(|b| b.get("border_bit").is_some())
                .count();
            e.set("border_bit", int(n as i64));
        }
        let base_nm = nm.strip_suffix("Smplr").unwrap_or(&nm).to_string();
        if nm == "dm_border" {
            e.set("kind", Value::str("border"));
        } else if let Some(r) = geometry_helper(&nm) {
            e.set("kind", Value::str("geometry"));
            e.set("reg", int(r));
        } else if nm == "support_buffer" {
            e.set("kind", Value::str("support"));
        } else if let Some(n) = bank_name(&nm, 'c') {
            if (3..=16).contains(&n) {
                e.set("kind", Value::str("cbuf"));
                e.set("reg", int(n - 3));
            } else {
                e.set("kind", Value::str(if n == 1 { "c1" } else { "drvcb" }));
                e.set("bank", int(n));
            }
        } else if let Some(n) = bank_name(&nm, 's') {
            e.set("kind", Value::str("sbuf"));
            e.set("index", int(n));
        } else if let Some((ut, us)) = decode_ubo_tex(&base_nm) {
            // handle type 1: the word of the other half, which the host ORs in
            let other = us.is_ok_and(|u| u != ut).then_some(us);
            if nm.ends_with("Smplr") {
                match us {
                    Err(id) => {
                        e.set("kind", Value::str("samp_id"));
                        e.set("reg", int(id));
                    }
                    Ok((r, o)) => {
                        e.set("kind", Value::str("samp_ubo"));
                        e.set("reg", int(r));
                        e.set("off", int(o));
                        if other.is_some() {
                            e.set("soff", int(ut.1));
                            e.set("sreg", int(ut.0));
                        }
                    }
                }
            } else if ty.contains("texture_buffer") {
                e.set("kind", Value::str("tbuf_ubo"));
                e.set("reg", int(ut.0));
                e.set("off", int(ut.1));
            } else {
                e.set("kind", Value::str("tex_ubo"));
                e.set("reg", int(ut.0));
                e.set("off", int(ut.1));
                if let Some(Ok((r, o))) = other {
                    e.set("soff", int(o));
                    e.set("sreg", int(r));
                }
            }
        } else if let Some((reg, off)) = decode_ubo_img(&nm) {
            e.set("kind", Value::str("img_ubo"));
            e.set("reg", int(reg));
            e.set("off", int(off));
            if word_followed(msl, &nm, ".atomic_") {
                e.set("atomic", Value::Bool(true));
            }
        } else if let Some((off, soff)) = c2_array(&nm, &ty) {
            e.set(
                "kind",
                Value::str(if nm.ends_with("Smplr") {
                    "samp_c2a"
                } else {
                    "tex_c2a"
                }),
            );
            e.set("off", int(off));
            if let Some(s) = soff {
                e.set("soff", int(s));
            }
        } else if nm.ends_with("Smplr") {
            let Some((t, s, hty)) = decode_tex(&base_nm) else {
                return Err(format!("{key}: sampler {nm} has no decodable handle"));
            };
            let opt = |v: Option<i64>| v.map_or(Value::Null, int);
            if hty == 3 {
                e.set("kind", Value::str("samp_id"));
                e.set("reg", opt(const_sampler(&base_nm)));
            } else if hty != 0 {
                e.set("kind", Value::str("samp"));
                e.set("reg", opt(s));
            } else {
                e.set("kind", Value::str("samp_tex"));
                e.set("reg", opt(t));
            }
        } else {
            match decode_tex(&nm) {
                None => e.set("kind", Value::str("?")),
                Some((t, _, _)) => {
                    let kind = if is_image(&nm) {
                        "img"
                    } else if ty.contains("texture_buffer") {
                        "tbuf"
                    } else {
                        "tex"
                    };
                    e.set("kind", Value::str(kind));
                    e.set("reg", t.map_or(Value::Null, int));
                    if kind == "img" && word_followed(msl, &nm, ".atomic_") {
                        e.set("atomic", Value::Bool(true));
                    }
                    if kind == "tex" && t.is_some_and(|r| fetch1d.contains(&r)) {
                        e.set("fetch1d", Value::Bool(true));
                        if word_followed(msl, &nm, ".get_width(") {
                            e.set("fetch1d_size", Value::Bool(true));
                        }
                    }
                }
            }
        }
        binds.push(e);
    }
    storage_buffers(stage, res, &mut binds);
    Ok(Value::List(binds))
}

/// The storage-buffer pass of `reflect`.
fn storage_buffers(stage: &str, res: &Value, binds: &mut [Value]) {
    let empty = Vec::new();
    let mut sb: Vec<&Value> = res
        .get("sbufs")
        .and_then(Value::as_list)
        .unwrap_or(&empty)
        .iter()
        .collect();
    sb.sort_by_key(|b| b.get("binding").and_then(Value::as_i64).unwrap_or(0));
    let notes: Vec<&str> = res
        .get("notes")
        .and_then(Value::as_list)
        .unwrap_or(&empty)
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let base = ssbo(stage);
    for e in binds.iter_mut() {
        if e.get("kind").and_then(Value::as_str) != Some("sbuf") {
            continue;
        }
        let index = e.get("index").and_then(Value::as_i64).unwrap_or(0);
        let b = usize::try_from(index).ok().and_then(|i| sb.get(i)).copied();
        let g = |k: &str| {
            b.and_then(|b| b.get(k))
                .and_then(Value::as_i64)
                .unwrap_or(0)
        };
        let off = b.filter(|_| g("cb_slot") == 0).map(|_| g("cb_offset") * 4);
        if b.is_some() && g("cb_slot") == GMEM_MAP_CB_SLOT {
            if g("cb_offset") == GMEM_LOG_CB_OFFSET {
                e.set("kind", Value::str("gmem_log"));
                e.set("rw", Value::Bool(true));
                continue;
            }
            let rw = notes.iter().any(|n| {
                n.match_indices("operation \"").any(|(i, m)| {
                    let r = &n[i + m.len()..];
                    let op = r.split('"').next().unwrap_or("");
                    let ok = op == "Store"
                        || op
                            .strip_prefix("Atomic")
                            .is_some_and(|x| x.chars().all(is_word));
                    ok && r[op.len()..].starts_with("\" traced to no storage buffer")
                })
            });
            e.set("kind", Value::str("gmem_map"));
            e.set("rw", Value::Bool(rw));
            continue;
        }
        if let Some(o) = off
            && stage == "cs"
            && (C0_CS_UBO..C0_CS_UBO + C0_ROW * C0_CS_UBO_SLOTS).contains(&o)
            && (o - C0_CS_UBO) % C0_ROW == 0
        {
            e.set("kind", Value::str("ubo_ptr"));
            e.set("reg", int((o - C0_CS_UBO) / C0_ROW));
        } else if b.is_some() && g("cb_slot") >= 3 {
            e.set("kind", Value::str("sbuf_ubo"));
            e.set("reg", int(g("cb_slot") - 3));
            e.set("off", int(g("cb_offset") * 4));
        } else {
            let reg = match (off, base) {
                (Some(o), Some(bs))
                    if (0..C0_ROW * C0_SSBO_SLOTS).contains(&(o - bs))
                        && (o - bs) % C0_ROW == 0 =>
                {
                    int((o - bs) / C0_ROW)
                }
                _ => Value::Null,
            };
            e.set("reg", reg);
        }
        e.set("rw", Value::Bool(b.is_some() && g("flags") & 1 != 0));
    }
}

/// `drv_reads(src)`.
pub fn drv_reads(src: &str) -> Value {
    let mut out = Vec::new();
    for bank in [0, 2] {
        let pat = format!("fp_c{bank})._m0[");
        let (mut offs, mut dynamic) = (BTreeSet::new(), false);
        for (i, _) in src.match_indices(&pat) {
            let r = &src[i + pat.len()..];
            let Some(k) = r.find(']').filter(|k| *k > 0) else {
                continue;
            };
            let idx = r[..k].trim_end_matches('u');
            if !idx.is_empty() && idx.bytes().all(|c| c.is_ascii_digit()) {
                if let Ok(v) = idx.parse::<i64>() {
                    offs.insert(v * 16);
                }
            } else {
                dynamic = true;
            }
        }
        if !offs.is_empty() || dynamic {
            out.push((
                bank.to_string(),
                Value::obj([
                    (
                        "vec4_offsets",
                        Value::List(offs.into_iter().map(int).collect()),
                    ),
                    ("dynamic", Value::Bool(dynamic)),
                ]),
            ));
        }
    }
    Value::Object(out)
}

/// `unfillable(dr, stage)`.
pub fn unfillable(dr: &Value, stage: &str) -> Value {
    let mut bad = Vec::new();
    let Value::Object(banks) = dr else {
        return Value::List(bad);
    };
    for (bank, d) in banks {
        let mut fill: Vec<(i64, i64)> = if bank == "0" {
            [272, 784, 1040, 1296]
                .iter()
                .map(|b| (*b, b + C0_ROW * C0_SSBO_SLOTS))
                .collect()
        } else {
            vec![
                (C2_TEX, C2_SEPTEX),
                (C2_IMG, C2_IMG + C2_ROW * C2_IMG_SLOTS),
                (C2_SEPTEX, C2_SAMP),
                (C2_SAMP, C2_SAMP + C2_ROW * C2_SEP_SLOTS),
            ]
        };
        if bank == "0" && stage == "ps" {
            fill.push((C0_SAMPLE_TABLE, C0_SAMPLE_TABLE + C0_SAMPLE_BYTES));
            fill.push((C0_SAMPLE_INFO, C0_SAMPLE_INFO + C0_ROW));
        }
        if bank == "0"
            && (stage == "ts" || stage == "ms")
            && let Some(b) = ssbo(stage)
        {
            fill.push((b, b + C0_ROW * C0_SSBO_SLOTS));
        }
        let offs: Vec<i64> = d
            .get("vec4_offsets")
            .and_then(Value::as_list)
            .unwrap_or(&[])
            .iter()
            .filter_map(Value::as_i64)
            .collect();
        let dynamic = d.get("dynamic") == Some(&Value::Bool(true));
        if dynamic && bank == "0" && !(stage == "ps" && offs.contains(&C0_SAMPLE_INFO)) {
            bad.push(Value::str(format!("c{bank} dynamic index")));
        }
        for o in offs {
            if !fill.iter().any(|(a, b)| o < *b && *a < o + 16) {
                bad.push(Value::str(format!("c{bank}+{o:#x}")));
            }
        }
    }
    Value::List(bad)
}

/// `raster_facts('ps', src)`.
pub fn raster_ps(src: &str) -> Value {
    let out = crate::msl::out_group(src).map_or("", |(a, b)| &src[a..b]);
    let colors: BTreeSet<i64> = out
        .match_indices("[[color(")
        .filter_map(|(i, m)| {
            let r = &out[i + m.len()..];
            let d = r.bytes().take_while(u8::is_ascii_digit).count();
            (d > 0 && r[d..].starts_with(")]]"))
                .then(|| r[..d].parse().ok())
                .flatten()
        })
        .collect();
    Value::obj([
        ("discard", int(i64::from(src.contains("discard_fragment")))),
        ("sample_mask", int(i64::from(out.contains("sample_mask")))),
        (
            "primitive_id",
            int(i64::from(src.contains("[[primitive_id]]"))),
        ),
        ("colors", Value::List(colors.into_iter().map(int).collect())),
    ])
}

/// `re.search(r'(vertex|fragment|kernel)\s+\S+\s+(\w+)\s*\(', src)` → (kind, name).
pub fn entry(src: &str) -> Option<(String, String)> {
    let mut hits: Vec<(usize, &str)> = Vec::new();
    for k in ["vertex", "fragment", "kernel"] {
        hits.extend(src.match_indices(k).map(|(i, _)| (i, k)));
    }
    hits.sort();
    for (i, k) in hits {
        let r = &src[i + k.len()..];
        let a = r.trim_start();
        if a.len() == r.len() {
            continue;
        }
        let s = a.find(char::is_whitespace).unwrap_or(a.len());
        if s == 0 {
            continue;
        }
        let b = &a[s..];
        let c = b.trim_start();
        if c.len() == b.len() {
            continue;
        }
        let w = c.find(|ch: char| !is_word(ch)).unwrap_or(c.len());
        if w > 0 && c[w..].trim_start().starts_with('(') {
            return Some((k.to_string(), c[..w].to_string()));
        }
    }
    None
}

/// `vertex_inputs`: `(\w+)\s+\w+\s*\[\[attribute\((\d+)\)\]\]` matches → `{location, msl_type}`.
pub fn vertex_inputs(src: &str) -> Value {
    let mut out = Vec::new();
    for (i, m) in src.match_indices("[[attribute(") {
        let r = &src[i + m.len()..];
        let d = r.bytes().take_while(u8::is_ascii_digit).count();
        if d == 0 || !r[d..].starts_with(")]]") {
            continue;
        }
        let before = src[..i].trim_end();
        let name_start = before
            .char_indices()
            .rev()
            .take_while(|(_, c)| is_word(*c))
            .last()
            .map(|(k, _)| k);
        let Some(ns) = name_start else { continue };
        let gap = &before[..ns];
        let t = gap.trim_end();
        if t.len() == gap.len() {
            continue;
        }
        let Some(ts) = t
            .char_indices()
            .rev()
            .take_while(|(_, c)| is_word(*c))
            .last()
            .map(|(k, _)| k)
        else {
            continue;
        };
        if let Ok(loc) = r[..d].parse::<i64>() {
            out.push(Value::obj([
                ("location", int(loc)),
                ("msl_type", Value::str(&t[ts..])),
            ]));
        }
    }
    Value::List(out)
}
