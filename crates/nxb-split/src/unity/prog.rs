//! The Shader readers of the reference (`shader_programs`, `subprogram`, `unity_program`, `unity_stages`,
//! `unity_code_blobs`, `unity_pass_pairs`, `unity_compute`, `unity_text_glslc`, `unity_shaders`).

use std::collections::{BTreeSet, HashMap, HashSet};

use super::bundle::{bundle_files, is_serialized};
use super::ser::{Trees, Val, is_no_tree, objects, text_assets};
use crate::SplitError;
use crate::glslc::glslc_shaders;
use crate::rd::{red, sl, u32le};
use crate::shaders::{CTL_MAGIC, Found, Profile, check, memfind, skey};

type R<T> = Result<T, SplitError>;
/// (stage, control section, code area)
type Stage = (&'static str, Vec<u8>, Vec<u8>);
type Programs = HashMap<(i128, i128), (&'static str, Vec<u8>, Vec<u8>)>;

const GLSLC: [u8; 4] = [0x91, 0x68, 0x86, 0x19];
const NV_STAGES: [&str; 6] = ["vs", "ps", "gs", "hs", "ds", "cs"];

fn le32(d: &[u8], o: usize) -> R<i32> {
    Ok(u32le(d, o)? as u32 as i32)
}

fn lz4(raw: &[u8], size: usize) -> R<Vec<u8>> {
    crate::lz4::block_upto(raw, size)
}

fn ints(v: &Val) -> Vec<i128> {
    v.list().iter().filter_map(Val::int).collect()
}

fn at(v: Option<&Val>, i: usize) -> R<&Val> {
    v.and_then(|x| x.list().get(i))
        .ok_or_else(|| SplitError::Red("IndexError: list index out of range".into()))
}

/// `shader_programs`: (platform, subprogram index, bytes) of a Shader object (`only`: those indices).
fn shader_programs(sh: &Val, only: Option<&HashSet<i128>>) -> R<Vec<(i128, usize, Vec<u8>)>> {
    let blob = sh.get("compressedBlob").map_or(&[][..], Val::bytes);
    let mut out = Vec::new();
    for (i, plat) in ints(sh.get("platforms").unwrap_or(&Val::List(vec![])))
        .into_iter()
        .enumerate()
    {
        let (offs, clen, dlen) = (
            at(sh.get("offsets"), i)?,
            at(sh.get("compressedLengths"), i)?,
            at(sh.get("decompressedLengths"), i)?,
        );
        let want = |k: usize| only.is_none_or(|o| o.contains(&(k as i128)));
        if let Val::Int(o) = offs {
            let (o, c, d) = (
                *o as usize,
                clen.int().unwrap_or(0) as usize,
                dlen.int().unwrap_or(0) as usize,
            );
            let s0 = lz4(sl(blob, o, o + c), d)?;
            for k in 0..u32le(&s0, 0)? {
                let (off, ln) = (u32le(&s0, 4 + 8 * k)?, u32le(&s0, 8 + 8 * k)?);
                if want(k) {
                    out.push((plat, k, sl(&s0, off, off + ln).to_vec()));
                }
            }
            continue;
        }
        let (o, c, d) = (ints(offs), ints(clen), ints(dlen));
        let mut segs = Vec::new();
        for ((o, c), u) in o.iter().zip(&c).zip(&d) {
            segs.push(lz4(sl(blob, *o as usize, (*o + *c) as usize), *u as usize)?);
        }
        let Some(s0) = segs.first() else { continue };
        for k in 0..u32le(s0, 0)? {
            let (off, ln, seg) = (
                u32le(s0, 4 + 12 * k)?,
                u32le(s0, 8 + 12 * k)?,
                u32le(s0, 12 + 12 * k)?,
            );
            if want(k) {
                let s = segs
                    .get(seg)
                    .ok_or_else(|| SplitError::Red("IndexError: list index out of range".into()))?;
                out.push((plat, k, sl(s, off, off + ln).to_vec()));
            }
        }
    }
    Ok(out)
}

/// `subprogram`: the program code of a subprogram's bytes.
fn subprogram(b: &[u8]) -> R<Vec<u8>> {
    let ver = le32(b, 0)?;
    le32(b, 4)?;
    let mut p = 8 + 12 + if ver >= 201_608_170 { 4 } else { 0 };
    let skip = |p: &mut usize| -> R<()> {
        let n = le32(b, *p)?;
        *p += 4;
        for _ in 0..n {
            let ln = le32(b, *p)?.max(0) as usize;
            *p += 4 + ln;
            *p = (*p + 3) & !3;
        }
        Ok(())
    };
    skip(&mut p)?;
    if (201_806_140..202_012_090).contains(&ver) {
        skip(&mut p)?;
    }
    let ln = le32(b, p)?.max(0) as usize;
    p += 4;
    Ok(sl(b, p, p + ln).to_vec())
}

/// `unity_program`: [(stage, control, code)] of a Switch program code.
fn unity_program(code: &[u8], where_: &str, stage: Option<&'static str>) -> R<Vec<Stage>> {
    if code.len() >= 0x14 && sl(code, 0x10, 0x14) == CTL_MAGIC {
        let Some(st) = stage else {
            return red(format!(
                "{where_}: pre-2019.3 subprogram whose stage is not in m_ParsedForm"
            ));
        };
        let (nctl, ncode) = (le32(code, 8)?, le32(code, 12)?);
        if nctl < 0 || ncode < 0 {
            return red(format!("{where_}: negative section lengths"));
        }
        let (a, end) = (0x10usize, 0x10 + nctl as usize + ncode as usize);
        let (ctl, cd) = (
            sl(code, a, a + nctl as usize),
            sl(code, a + nctl as usize, end),
        );
        let why = if end <= code.len() {
            check(st, ctl, cd).map(str::to_string)
        } else {
            Some("block out of bounds".into())
        };
        if let Some(w) = why {
            return red(format!("{where_} {st}: {w}"));
        }
        if end != code.len() {
            return red(format!(
                "{where_}: program code is {:#x} bytes, but control section + code area end at {end:#x}",
                code.len()
            ));
        }
        return Ok(vec![(st, ctl.to_vec(), cd.to_vec())]);
    }
    let h: Vec<i32> = (0..24).map(|i| le32(code, 4 * i)).collect::<R<_>>()?;
    let (mut out, mut end) = (Vec::new(), 0x60usize);
    for (i, st) in NV_STAGES.iter().enumerate() {
        let (off, nctl, ncode) = (h[6 + i], h[12 + i], h[18 + i]);
        if off == -1 {
            continue;
        }
        if off < 0 || nctl < 0 || ncode < 0 {
            return red(format!("{where_} {st}: negative stage block"));
        }
        let (a, nc, nk) = (0x60 + off as usize, nctl as usize, ncode as usize);
        let (ctl, cd) = (sl(code, a, a + nc), sl(code, a + nc, a + nc + nk));
        let why = if a + nc + nk <= code.len() {
            check(st, ctl, cd).map(str::to_string)
        } else {
            Some("block out of bounds".into())
        };
        if let Some(w) = why {
            return red(format!("{where_} {st}: {w}"));
        }
        out.push((*st, ctl.to_vec(), cd.to_vec()));
        end = end.max(a + nc + nk);
    }
    if end != code.len() {
        return red(format!(
            "{where_}: program code is {:#x} bytes, but the stage blocks end at {end:#x}",
            code.len()
        ));
    }
    Ok(out)
}

const PROG_FIELDS: [(&str, &str); 5] = [
    ("progVertex", "vs"),
    ("progFragment", "ps"),
    ("progGeometry", "gs"),
    ("progHull", "hs"),
    ("progDomain", "ds"),
];

fn passes(o: &Val) -> Vec<&Val> {
    o.get("m_ParsedForm")
        .and_then(|f| f.get("m_SubShaders"))
        .map(|s| {
            s.list()
                .iter()
                .flat_map(|sub| sub.get("m_Passes").map_or(&[][..], Val::list).iter())
                .collect()
        })
        .unwrap_or_default()
}

/// `unity_stages`: subprogram index to stage.
fn unity_stages(o: &Val) -> HashMap<i128, &'static str> {
    let mut m = HashMap::new();
    for p in passes(o) {
        for (field, st) in PROG_FIELDS {
            for sp in p
                .get(field)
                .and_then(|f| f.get("m_SubPrograms"))
                .map_or(&[][..], Val::list)
            {
                if let Some(i) = sp.get("m_BlobIndex").and_then(Val::int) {
                    m.entry(i).or_insert(st);
                }
            }
        }
    }
    m
}

/// `unity_code_blobs`: the subprogram indices that hold program code, `None` when every entry does.
fn unity_code_blobs(o: &Val) -> Option<HashSet<i128>> {
    let progs: Vec<&Val> = passes(o)
        .into_iter()
        .flat_map(|p| PROG_FIELDS.iter().filter_map(move |(f, _)| p.get(f)))
        .collect();
    if !progs
        .iter()
        .any(|p| p.get("m_ParameterBlobIndices").is_some())
    {
        return None;
    }
    Some(
        progs
            .iter()
            .flat_map(|p| {
                p.get("m_PlayerSubPrograms")
                    .map_or(&[][..], Val::list)
                    .iter()
                    .flat_map(|t| {
                        t.list()
                            .iter()
                            .filter_map(|sp| sp.get("m_BlobIndex").and_then(Val::int))
                    })
            })
            .collect(),
    )
}

/// `ps_inputs`: the generic attributes a pixel shader reads.
fn ps_inputs(code: &[u8]) -> R<BTreeSet<u32>> {
    let mut s = BTreeSet::new();
    for i in 0..32u32 {
        if (u32le(code, 0x30 + 4 * (6 + (i as usize >> 2)))? >> ((i & 3) * 8)) & 0xff != 0 {
            s.insert(i);
        }
    }
    Ok(s)
}

/// `pixel_input_cover`: a greedy cover of the generic inputs, ties to the smaller key.
fn pixel_input_cover(shaders: &HashMap<String, Vec<u8>>) -> R<Vec<String>> {
    let ins: Vec<(&String, BTreeSet<u32>)> = shaders
        .iter()
        .map(|(k, c)| Ok((k, ps_inputs(c)?)))
        .collect::<R<_>>()?;
    let mut left: BTreeSet<u32> = ins.iter().flat_map(|x| x.1.iter().copied()).collect();
    let mut cover = Vec::new();
    while !left.is_empty() {
        let best = ins
            .iter()
            .min_by_key(|(k, s)| {
                (
                    std::cmp::Reverse(s.intersection(&left).count()),
                    (*k).clone(),
                )
            })
            .map(|(k, s)| ((*k).clone(), s.clone()));
        let Some((k, s)) = best else { break };
        left = left.difference(&s).copied().collect();
        cover.push(k);
    }
    Ok(cover)
}

/// `unity_pass_pairs`: vertex shaders paired with the cover of the pixel inputs of each pass and platform.
fn unity_pass_pairs(p: &Profile, o: &Val, programs: &Programs) -> R<BTreeSet<(String, String)>> {
    let mut pairs = BTreeSet::new();
    let plats: BTreeSet<i128> = programs.keys().map(|k| k.0).collect();
    for pass in passes(o) {
        for plat in &plats {
            let mut stages: HashMap<&str, HashMap<String, Vec<u8>>> = HashMap::new();
            for (field, stage) in [("progVertex", "vs"), ("progFragment", "ps")] {
                let m = stages.entry(stage).or_default();
                for sp in pass
                    .get(field)
                    .and_then(|f| f.get("m_SubPrograms"))
                    .map_or(&[][..], Val::list)
                {
                    let Some(bi) = sp.get("m_BlobIndex").and_then(Val::int) else {
                        continue;
                    };
                    if let Some((st, ctl, code)) = programs.get(&(*plat, bi))
                        && *st == stage
                    {
                        m.insert(skey(p, ctl, code), code.clone());
                    }
                }
            }
            let cover = pixel_input_cover(&stages["ps"])?;
            for v in stages["vs"].keys() {
                for f in &cover {
                    pairs.insert((v.clone(), f.clone()));
                }
            }
        }
    }
    Ok(pairs)
}

/// What a Unity asset gives: shaders and the legacy pass pairs.
pub struct Found2 {
    /// Shaders in the reference's order.
    pub found: Vec<Found>,
    /// Vertex and pixel-cover pairs of the legacy programs.
    pub pairs: BTreeSet<(String, String)>,
}

/// `unity_compute`: ComputeShader objects of a serialized file.
fn unity_compute(
    x: &[u8],
    base: &str,
    common: &HashMap<u32, String>,
    trees: &mut Trees,
    second: bool,
    p: &Profile,
    notes: &mut Vec<String>,
) -> R<Vec<Found>> {
    let mut out = Vec::new();
    match objects(x, common, &[72], Some(trees)) {
        Ok(objs) => {
            for (_c, _p, o) in objs {
                let name = o
                    .get("m_Name")
                    .and_then(|v| {
                        if let Val::Str(s) = v {
                            Some(s.as_str())
                        } else {
                            None
                        }
                    })
                    .unwrap_or("");
                for v in o.get("variants").map_or(&[][..], Val::list) {
                    for k in v.get("kernels").map_or(&[][..], Val::list) {
                        let variants: Vec<&Val> = if let Some(u) = k.get("uniqueVariants") {
                            u.list().iter().collect()
                        } else if let Some(m) = k.get("variantMap") {
                            m.list().iter().filter_map(|x| x.get("second")).collect()
                        } else {
                            vec![k]
                        };
                        for (i, u) in variants.into_iter().enumerate() {
                            let code = u.get("code").map_or(&[][..], Val::bytes);
                            if code.is_empty() {
                                continue;
                            }
                            let kn = k
                                .get("name")
                                .and_then(|v| {
                                    if let Val::Str(s) = v {
                                        Some(s.as_str())
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or("");
                            let where_ = format!("{base}#{name}#{kn}.{i}");
                            for (st, ctl, cd) in unity_program(code, &where_, Some("cs"))? {
                                out.push(Found {
                                    place: where_.clone(),
                                    stage: st,
                                    ctl,
                                    code: cd,
                                });
                            }
                        }
                    }
                }
            }
        }
        Err(e) if is_no_tree(&e) => {
            if !second {
                return Err(e);
            }
            notes.push(format!("[RED] {base}: ComputeShader objects that no bundle lends a type tree to and unityfs.py has no layout for, their kernels are left out ({e})"));
            return Ok(Vec::new());
        }
        Err(e) => return Err(e),
    }
    for f in &out {
        notes.push(format!(
            "unity compute source: [\"{}\", \"{{label}}{}\"]",
            skey(p, &f.ctl, &f.code),
            f.place
        ));
    }
    Ok(out)
}

/// `unity_text_glslc`: whole GLSLC outputs in TextAssets.
fn unity_text_glslc(x: &[u8], base: &str, common: &HashMap<u32, String>) -> R<Vec<Found>> {
    if memfind(x, &GLSLC).is_none() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for (name, b) in text_assets(x, common)? {
        if memfind(&b, &GLSLC).is_some() {
            out.extend(glslc_shaders(&b, &format!("{base}#{name}"), false, &[])?);
        }
    }
    Ok(out)
}

/// `unity_shaders`: shaders and compute shaders in a UnityFS bundle or serialized file. A `NoTree` error means the
/// file is scanned again once the trees of every bundle are known.
pub fn unity_shaders(
    d: &[u8],
    label: &str,
    common: Option<&HashMap<u32, String>>,
    trees: &mut Trees,
    second: bool,
    p: &Profile,
    notes: &mut Vec<String>,
) -> R<Found2> {
    let Some(common) = common else {
        return red(format!(
            "{label}: a Unity asset, but main has no Unity common string table"
        ));
    };
    let (mut found, mut pairs) = (Vec::new(), BTreeSet::new());
    let sers = if sl(d, 0, 8) == b"UnityFS\0" {
        bundle_files(d)?
    } else {
        vec![(String::new(), d.to_vec())]
    };
    for (node, x) in sers {
        if !is_serialized(&x) {
            continue;
        }
        let base = if node.is_empty() {
            label.to_string()
        } else {
            format!("{label}:{node}")
        };
        for (_c, _p, o) in objects(&x, common, &[48], Some(trees))? {
            let name = match o
                .get("m_ParsedForm")
                .and_then(|f| f.get("m_Name"))
                .or_else(|| o.get("m_Name"))
            {
                Some(Val::Str(s)) => s.clone(),
                _ => "?".into(),
            };
            let stages = unity_stages(&o);
            let mut legacy: Programs = HashMap::new();
            let blobs = unity_code_blobs(&o);
            for (plat, k, sp) in shader_programs(&o, blobs.as_ref())? {
                let where_ = format!("{base}#{name}#var{plat}.{k}");
                let code = subprogram(&sp)?;
                let programs = unity_program(&code, &where_, stages.get(&(k as i128)).copied())?;
                if code.len() >= 0x14 && sl(&code, 0x10, 0x14) == CTL_MAGIC {
                    legacy.insert((plat, k as i128), programs[0].clone());
                }
                for (st, ctl, cd) in programs {
                    found.push(Found {
                        place: where_.clone(),
                        stage: st,
                        ctl,
                        code: cd,
                    });
                }
            }
            pairs.extend(unity_pass_pairs(p, &o, &legacy)?);
        }
        found.extend(unity_compute(&x, &base, common, trees, second, p, notes)?);
        found.extend(unity_text_glslc(&x, &base, common)?);
    }
    Ok(Found2 { found, pairs })
}
