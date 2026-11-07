//! `build_from`: the canonical layout, the placement of the requests, the moved code, the output relocations and the
//! three files.

use std::collections::{BTreeMap, BTreeSet};

use super::a64::{Kind, adrp_add, branch_target, seqs, set_branch, words};
use super::{Image, Keys, PAGE, Record, Region, align, hn, hx};

/// Why an attempt failed.
pub(crate) enum Fail {
    /// `Unplaced`: code of `session` refers to the code memory offset `off`, which none of its records holds.
    Unplaced { session: u64, off: i128 },
    /// Any other error (the tool's `[RED]` exits and assertion failures).
    Other(String),
}

impl From<String> for Fail {
    fn from(m: String) -> Fail {
        Fail::Other(m)
    }
}

/// The text of an `Unplaced`.
pub(crate) fn unplaced(session: u64, off: i128) -> String {
    format!(
        "code offset {} of session {session:#x} lies in no captured request",
        hx(off)
    )
}

/// `region_of`: the first region that holds `v` (its patch area included) and the offset from its base.
fn region_of(regs: &[Region], v: i128) -> Option<(&str, i128)> {
    regs.iter()
        .find(|r| {
            let base = i128::from(r.base);
            base - i128::from(r.pre) <= v && v < base + i128::from(r.size)
        })
        .map(|r| (r.name.as_str(), v - i128::from(r.base)))
}

type Spans = BTreeMap<u64, Vec<(i128, i128, i128)>>;

/// `map_rx`: the image offset of the code memory offset `off` of `session`.
fn map_rx(spans: &Spans, session: u64, off: i128) -> Result<i128, Fail> {
    spans
        .get(&session)
        .into_iter()
        .flatten()
        .find(|&&(c, n, _)| (c <= off && off < c + n) || (n == 0 && off == c))
        .map(|&(c, _, i)| i + off - c)
        .ok_or(Fail::Unplaced { session, off })
}

type Mods = Vec<(String, (i128, u64, u64))>;

/// `canon`: the canonical offset of the host address `v` of record `r`, `None` for a constant.
fn canon(spans: &Spans, mods: &Mods, key: &str, r: &Record, v: u64) -> Result<Option<i128>, Fail> {
    let Some((name, off)) = region_of(&r.regs, i128::from(v)) else {
        return Ok(None);
    };
    if name == "jit_rx" {
        return map_rx(spans, r.session, off).map(Some);
    }
    if let Some((_, (m, _, _))) = mods.iter().find(|(n, _)| n == name) {
        return Ok(Some(m + off));
    }
    Err(Fail::Other(format!(
        "{key}: an address into {name} (+{}), which the canonical layout does not hold",
        hx(off)
    )))
}

fn bytes_of(w: &[u32]) -> Vec<u8> {
    w.iter().flat_map(|x| x.to_le_bytes()).collect()
}

/// The code of one request with its address sequences made PC-relative at `pos0` and its branches out of the request
/// moved. `skip_seqs`: the branch pass leaves the words of the sequences alone (the first record of a request).
fn relocate(
    spans: &Spans,
    mods: &Mods,
    key: &str,
    r: &Record,
    pos0: i128,
    skip_seqs: bool,
    stats: &mut Stats,
) -> Result<Vec<u32>, Fail> {
    let cap0 = i128::from(r.o0.0);
    let len = r.code.len() as i128;
    let mut w = words(&r.code);
    let mut inseq = BTreeSet::new();
    for (p, rd, v) in seqs(&w) {
        let t = canon(spans, mods, key, r, v)?;
        inseq.extend(p / 4..p / 4 + 4);
        let Some(t) = t else {
            stats.constants += 1;
            continue;
        };
        let at = adrp_add(rd, pos0 + p as i128, t).map_err(|e| format!("{key}: {e}"))?;
        w[p / 4..p / 4 + 4].copy_from_slice(&at);
        stats.addresses += 1;
    }
    for (i, slot) in w.iter_mut().enumerate() {
        if skip_seqs && inseq.contains(&i) {
            continue;
        }
        let word = *slot;
        let Some((kind, tgt)) = branch_target(word, cap0 + 4 * i as i128) else {
            continue;
        };
        let Some(tgt) = tgt.filter(|_| kind != Kind::Lit) else {
            if skip_seqs {
                return Err(Fail::Other(format!(
                    "{key}: a literal / adr form at +{:#x}, which the image does not move",
                    4 * i
                )));
            }
            continue;
        };
        if cap0 <= tgt && tgt < cap0 + len {
            continue;
        }
        let to = map_rx(spans, r.session, tgt)?;
        *slot = set_branch(word, kind, to - (pos0 + 4 * i as i128))
            .map_err(|e| format!("{key}: {e}"))?;
        stats.branches += 1;
    }
    Ok(w)
}

#[derive(Default)]
struct Stats {
    addresses: u64,
    constants: u64,
    branches: u64,
    relocations: u64,
}

/// The output relocations of a request: `(offset, region, offset in the region)`.
fn relocations(
    spans: &Spans,
    key: &str,
    rl: &[Record],
    notes: &mut Vec<String>,
) -> Result<Vec<(usize, String, i128)>, Fail> {
    let r = &rl[0];
    let out = &r.out;
    let word = |b: &[u8], i: usize| {
        b.get(i..i + 8)
            .and_then(|s| <[u8; 8]>::try_from(s).ok())
            .map(u64::from_le_bytes)
    };
    let mut rel = Vec::new();
    let Some(r2) = rl.get(1) else {
        for i in (0..out.len().saturating_sub(7)).step_by(8) {
            let v = word(out, i).unwrap_or(0);
            if v >> 32 != 0 && region_of(&r.regs, i128::from(v)).is_some_and(|(n, _)| !n.is_empty())
            {
                return Err(Fail::Other(format!(
                    "{key}: output word +{i:#x} ({v:#x}) may be a host address and the request was captured once: capture it again in another run"
                )));
            }
        }
        return Ok(rel);
    };
    for i in (0..out.len().saturating_sub(7)).step_by(8) {
        let a = word(out, i).unwrap_or(0);
        let Some(b) = word(&r2.out, i) else {
            return Err(Fail::Other(format!(
                "{key}: the output of another capture is shorter"
            )));
        };
        if a == b {
            continue;
        }
        let one = region_of(&r.regs, i128::from(a));
        let two = region_of(&r2.regs, i128::from(b));
        match (one, two) {
            (Some((n1, o1)), Some((n2, o2))) if n1 == n2 && (n1 == "jit_rx" || o1 == o2) => {
                if n1 == "jit_rx" {
                    rel.push((i, "jit_rx".to_string(), map_rx(spans, r.session, o1)?));
                } else {
                    rel.push((i, n1.to_string(), o1));
                }
            }
            _ => {
                return Err(Fail::Other(format!(
                    "{key}: output word +{i:#x} differs between captures ({a:#x}, {b:#x}) and is no host address in both"
                )));
            }
        }
    }
    for g in &r.regs {
        let Some(g2) = r2.reg(&g.name) else {
            return Err(Fail::Other(format!(
                "{key}: another capture has no region {}",
                g.name
            )));
        };
        if g.base == g2.base && !g.name.starts_with("jit_t") {
            notes.push(format!(
                "[RED] {key}: {} was at the same base in both captures; its addresses in the output cannot be told apart",
                g.name
            ));
        }
    }
    Ok(rel)
}

/// The canonical layout: the code memory at 0, then every module of `any` in its order, page aligned, with its patch
/// area in front of its base.
fn layout(recs: &[(String, Vec<Record>)], rx_size: i128) -> Result<Mods, Fail> {
    let any = &recs[0].1[0];
    let mut mods: Mods = Vec::new();
    let mut cur = align(rx_size, PAGE as i128);
    for g in any.regs.iter().filter(|g| !g.name.starts_with("jit_")) {
        let off = align(cur + i128::from(g.pre), PAGE as i128);
        mods.push((g.name.clone(), (off, g.size, g.pre)));
        cur = off + i128::from(g.size);
    }
    for (k, rl) in recs {
        for r in rl {
            for g in r.regs.iter().filter(|g| !g.name.starts_with("jit_")) {
                let m = mods.iter().find(|(n, _)| *n == g.name).map(|(_, m)| *m);
                if m.is_none_or(|(_, s, p)| (s, p) != (g.size, g.pre)) {
                    let elsewhere =
                        m.map_or("None".to_string(), |(o, s, p)| format!("({o}, {s}, {p})"));
                    return Err(Fail::Other(format!(
                        "{k}: module {} is {:#x} + {:#x} here, {elsewhere} elsewhere",
                        g.name, g.size, g.pre
                    )));
                }
            }
        }
    }
    Ok(mods)
}

/// Placement: requests that every capture made at the same offset keep it, first come first served. The rest go
/// after the end, 64-byte aligned. Returns each request's image offset and the end.
fn place(recs: &[(String, Vec<Record>)]) -> (Vec<i128>, i128) {
    let mut order: Vec<usize> = (0..recs.len()).collect();
    order.sort_by(|&a, &b| {
        let key = |i: usize| (-(recs[i].1.len() as i64), recs[i].1[0].o0.0, &recs[i].0);
        key(a).cmp(&key(b))
    });
    let most = recs.iter().map(|(_, rl)| rl.len()).max().unwrap_or(0);
    let mut placed: Vec<(i128, i128)> = Vec::new();
    let mut at: Vec<Option<i128>> = vec![None; recs.len()];
    for &k in &order {
        let rl = &recs[k].1;
        let offs: BTreeSet<u64> = rl.iter().map(|r| r.o0.0).collect();
        let n = i128::from(rl[0].o0.1);
        let o = i128::from(rl[0].o0.0);
        let free = placed.iter().all(|&(p, m)| o + n <= p || p + m <= o);
        if offs.len() == 1 && free && rl.len() == most {
            at[k] = Some(o);
            placed.push((o, n));
        }
    }
    let mut end = align(placed.iter().map(|&(o, n)| o + n).max().unwrap_or(0), 64);
    for &k in &order {
        if at[k].is_some() {
            continue;
        }
        let n = i128::from(recs[k].1[0].o0.1);
        at[k] = Some(end);
        placed.push((end, n));
        end = align(end + n, 64);
    }
    (at.into_iter().map(|o| o.unwrap_or(0)).collect(), end)
}

/// One attempt of the build over `recs` (key, records of the key by session, in first-seen order).
pub(crate) fn build_from(
    recs: &[(String, Vec<Record>)],
    keys: &Keys,
    notes: &mut Vec<String>,
) -> Result<Image, Fail> {
    let any = &recs[0].1[0];
    let size_of = |n: &str| {
        any.reg(n)
            .map(|g| i128::from(g.size))
            .ok_or_else(|| Fail::Other(format!("{}: the capture has no region {n}", recs[0].0)))
    };
    let rx_size = size_of("jit_rx")?;
    let ro_size = size_of("jit_ro")?;
    let mods = layout(recs, rx_size)?;
    let (at, end) = place(recs);
    if end > rx_size {
        return Err(Fail::Other(format!(
            "the image ({end:#x} bytes) does not fit the code memory ({rx_size:#x})"
        )));
    }
    let mut spans: Spans = BTreeMap::new();
    for ((_, rl), &w) in recs.iter().zip(&at) {
        for r in rl {
            spans
                .entry(r.session)
                .or_default()
                .push((i128::from(r.o0.0), i128::from(r.o0.1), w));
        }
    }
    let total = align(end, PAGE as i128);
    let mut img: Vec<u8> = Vec::new();
    let size =
        usize::try_from(total).map_err(|_| format!("the image is too large ({total:#x})"))?;
    img.try_reserve_exact(size)
        .map_err(|_| format!("the image is too large ({total:#x})"))?;
    img.resize(size, 0);
    let mut stats = Stats::default();
    let mut disagree = Vec::new();
    for ((k, rl), &pos0) in recs.iter().zip(&at) {
        let w = bytes_of(&relocate(&spans, &mods, k, &rl[0], pos0, true, &mut stats)?);
        let p = usize::try_from(pos0).unwrap_or(usize::MAX);
        let Some(dst) = p.checked_add(w.len()).and_then(|e| img.get_mut(p..e)) else {
            return Err(Fail::Other(format!("{k}: the code lies outside the image")));
        };
        dst.copy_from_slice(&w);
        for r2 in &rl[1..] {
            if r2.code.len() != rl[0].code.len() {
                disagree.push(k.clone());
                continue;
            }
            let mut quiet = Stats::default();
            let x = relocate(&spans, &mods, k, r2, pos0, false, &mut quiet)?;
            if bytes_of(&x) != w {
                disagree.push(k.clone());
            }
        }
    }
    let mut rows = Vec::new();
    let mut blob = Vec::new();
    for ((k, rl), &w) in recs.iter().zip(&at) {
        let r = &rl[0];
        let rel = relocations(&spans, k, rl, notes)?;
        stats.relocations += rel.len() as u64;
        let o = blob.len();
        blob.extend_from_slice(&r.ro);
        blob.extend_from_slice(&r.out);
        let rels: Vec<String> = rel
            .iter()
            .map(|(i, n, v)| format!("{i:x}={n}+{}", hn(*v)))
            .collect();
        let rels = if rels.is_empty() {
            "-".to_string()
        } else {
            rels.join(",")
        };
        rows.push(format!(
            "req\t{k}\t{}\t{}\t{:#x}\t{:#x}\t{:#x}\t{}\t{o:#x}\t{:#x}\t{rels}",
            r.cmd,
            hx(w),
            r.o0.1,
            r.o1.0,
            r.o1.1,
            r.ret,
            r.out.len()
        ));
    }
    let sessions: BTreeSet<u64> = recs
        .iter()
        .flat_map(|(_, rl)| rl)
        .map(|r| r.session)
        .collect();
    let mut by_cmd: Vec<(u32, usize)> = Vec::new();
    for (_, rl) in recs {
        match by_cmd.iter_mut().find(|(c, _)| *c == rl[0].cmd) {
            Some(e) => e.1 += 1,
            None => by_cmd.push((rl[0].cmd, 1)),
        }
    }
    let by_cmd: Vec<String> = by_cmd.iter().map(|(c, n)| format!("{c}: {n}")).collect();
    let summary = format!(
        "{} requests from {} captures: {{{}}} by command; image {:.1} MB; {} address sequences made PC-relative, {} constants left, {} branches moved, {} output relocations; {} requests seen in more than one capture, {} of them different after relocation",
        recs.len(),
        sessions.len(),
        by_cmd.join(", "),
        end as f64 / 1048576.0,
        stats.addresses,
        stats.constants,
        stats.branches,
        stats.relocations,
        recs.iter().filter(|(_, rl)| rl.len() > 1).count(),
        disagree.len()
    );
    if !disagree.is_empty() {
        notes.push(format!(
            "[RED] requests that differ between captures: {}",
            disagree.join(" ")
        ));
    }
    let mut tsv = format!(
        "# nn::jit image (dm/tools/jit_capture.py) from {} captures\nlayout\t{}\t{}\t{}\n",
        sessions.len(),
        hx(rx_size),
        hx(ro_size),
        hx(total)
    );
    for (c, m) in &keys.masks {
        tsv += &format!("keydata\t{c}\t{}\n", nxb_model::to_hex(m));
    }
    for (c, (e, n, z)) in &keys.out_codes {
        tsv += &format!("outcode\t{c}\t{}\t{}\t{}\n", hx(*e), hx(*n), hx(*z));
    }
    for (name, (off, size, pre)) in &mods {
        tsv += &format!("mod\t{name}\t{}\t{size:#x}\t{pre:#x}\n", hx(*off));
    }
    tsv += &rows.join("\n");
    tsv.push('\n');
    Ok(Image {
        img,
        out: blob,
        tsv,
        summary,
        disagree,
    })
}
