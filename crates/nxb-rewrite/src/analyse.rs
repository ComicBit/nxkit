//! The per-function analysis: exclusive windows, retry blocks and the targets of every `br x18`.

use std::collections::{HashMap, HashSet};

use nxb_a64::{Insn, Op, Reg, Sh, decode};

use crate::error::fail;
use crate::jumps::An;
use crate::pat::{gn, is_direct_branch, preg};
use crate::rw::{R, Rw, num};
use crate::scan::{BIG_FN, Kind, Scan, pieces};
use crate::windows::{find_windows, retry_block};

/// The result of the function analysis.
pub(crate) struct Analysis {
    pub(crate) windows: Vec<Vec<Insn>>,
    pub(crate) retries: Vec<Vec<(Vec<Insn>, i64)>>,
    pub(crate) jumps: Vec<(i64, (i64, usize))>,
    pub(crate) tabjumps: Vec<(i64, Vec<i64>)>,
    pub(crate) tailcalls: HashSet<i64>,
    pub(crate) done: HashSet<i64>,
}

/// The function holding `a`: from the FDE start at or before it to the next one.
fn func_of(scan: &Scan, text_end: i64, a: i64) -> (i64, i64) {
    let starts = &scan.starts;
    let i = starts.partition_point(|s| *s <= a) as i64 - 1;
    if i >= 0 {
        let i = i as usize;
        return (starts[i], starts.get(i + 1).copied().unwrap_or(text_end));
    }
    if starts.is_empty() {
        return (a, a + 4);
    }
    (
        scan.data_blocks
            .iter()
            .filter(|(_, e)| *e <= a)
            .map(|(_, e)| *e)
            .max()
            .unwrap_or(0),
        starts[0],
    )
}

/// A function's instructions, a word that does not decode is skipped.
fn disasm_fn(rw: &Rw, fa: i64, fb: i64) -> Vec<Insn> {
    let mut out = Vec::with_capacity(((fb - fa) / 4) as usize);
    let mut a = fa;
    while a < fb {
        if let Some(i) = decode(rw.w32(a), a as u64) {
            out.push(i);
        }
        a += 4;
    }
    out
}

fn is_br18(i: &Insn) -> bool {
    i.mn == "br" && preg(i, 0) == Some(Reg::X(18)) && i.ops.len() == 1
}

fn idx_of(r: Reg) -> Option<u32> {
    match r {
        Reg::X(29) => Some(29),
        Reg::X(30) => Some(30),
        r => num(&r),
    }
}

pub(crate) fn analyse(rw: &Rw, scan: &Scan) -> R<Analysis> {
    let mut a = Analysis {
        windows: Vec::new(),
        retries: Vec::new(),
        jumps: Vec::new(),
        tabjumps: Vec::new(),
        tailcalls: HashSet::new(),
        done: HashSet::new(),
    };
    let mut jm: HashMap<i64, (i64, usize)> = HashMap::new();
    let mut tm: HashMap<i64, Vec<i64>> = HashMap::new();
    let sites18: Vec<i64> = scan
        .kinds
        .iter()
        .filter(|(_, (k, _))| *k == Kind::X18)
        .map(|(a, _)| *a)
        .collect();
    let mut funcs: Vec<(i64, i64)> = sites18
        .iter()
        .map(|x| func_of(scan, rw.text_end, *x))
        .collect();
    funcs.sort_unstable();
    funcs.dedup();
    let mut far_into: Vec<(i64, i64)> = Vec::new();
    let mut rmid_all: HashSet<i64> = HashSet::new();
    if funcs.iter().any(|(fa, fb)| fb - fa > BIG_FN) {
        let mut split = Vec::new();
        for (fa, fb) in &funcs {
            if fb - fa <= BIG_FN {
                split.push((*fa, *fb));
                continue;
            }
            let (ps, far) = pieces(rw, *fa, *fb);
            far_into.extend(far);
            for (p, q) in &ps {
                if sites18.partition_point(|s| s < p) < sites18.partition_point(|s| s < q) {
                    split.push((*p, *q));
                }
            }
        }
        funcs = split;
    }
    for (fa, fb) in funcs {
        let insl = disasm_fn(rw, fa, fb);
        let ws = find_windows(&insl);
        for w in &ws {
            for x in w {
                a.done.insert(x.addr as i64);
            }
        }
        a.windows.extend(ws.iter().cloned());
        for j in 0..insl.len() {
            let ins = &insl[j];
            if !is_br18(ins) {
                continue;
            }
            let (mut base, mut width): (Option<i64>, Option<usize>) = (None, None);
            let lo0 = j.saturating_sub(16);
            let ctx = &insl[lo0..j];
            for p in ctx.iter().rev() {
                if p.mn == "adr" && preg(p, 0) == Some(Reg::X(18)) {
                    base = p.imm(1);
                    break;
                }
            }
            for p in ctx.iter().rev() {
                let is_add = p.mn == "add"
                    && preg(p, 0) == Some(Reg::X(18))
                    && preg(p, 1) == Some(Reg::X(18))
                    && matches!(
                        p.ops.get(2),
                        Some(Op::Reg(
                            _,
                            Sh {
                                kind: "lsl",
                                amt: 2,
                                ..
                            }
                        ))
                    );
                if !is_add {
                    continue;
                }
                let idx = match p.ops.get(2) {
                    Some(Op::Reg(r, _)) => idx_of(*r),
                    _ => None,
                };
                for (qi, q) in ctx.iter().enumerate().rev() {
                    let dest = q.reg(0).and_then(|r| {
                        if matches!(r, Reg::W(_) | Reg::X(_)) {
                            gn(r).map(u32::from)
                        } else {
                            None
                        }
                    });
                    let same = dest == idx && dest.is_some();
                    if matches!(q.mn, "ldrb" | "ldrsb") && same {
                        width = Some(256);
                    }
                    if matches!(q.mn, "ldrh" | "ldrsh") && same {
                        let an = An {
                            img: &rw.img,
                            jumps: &jm,
                            tabjumps: &tm,
                        };
                        width = Some(match base {
                            Some(b) if b != 0 => {
                                an.half_count(&insl, lo0 + qi, b)?.unwrap_or(65536)
                            }
                            _ => 65536,
                        });
                    }
                    if width.is_some() {
                        break;
                    }
                }
                break;
            }
            let have = base.is_some_and(|b| b != 0) && width.is_some();
            if !have {
                let an = An {
                    img: &rw.img,
                    jumps: &jm,
                    tabjumps: &tm,
                };
                let mut tgt = an.jump_offsets(&insl, j, false, false, false)?;
                if tgt.is_none() {
                    tgt = an.jump_adr_offsets(&insl, j)?;
                }
                if tgt.is_none() && An::tail_call(&insl, j) {
                    a.tailcalls.insert(ins.addr as i64);
                    continue;
                }
                if tgt.is_none()
                    && let Some(far) = an.jump_bytes_far(&insl, j)?
                {
                    jm.insert(ins.addr as i64, far);
                    a.jumps.push((ins.addr as i64, far));
                    continue;
                }
                if tgt.is_none() {
                    tgt = an.jump_offsets(&insl, j, true, true, false)?;
                }
                if tgt.is_none() {
                    tgt = an.jump_masked_pred(&insl, j)?;
                }
                if tgt.is_none() {
                    tgt = an.jump_masked_not(&insl, j)?;
                }
                if tgt.is_none() {
                    tgt = an.jump_offsets(&insl, j, true, true, true)?;
                }
                if tgt.is_none() {
                    tgt = an.jump_zero_extended(&insl, j)?;
                }
                let Some(t) = tgt else {
                    return fail(format!(
                        "jump table at {:#x} in function {} ({fa:#x}..{fb:#x}): no bounds check guards the dispatch",
                        ins.addr,
                        rw.function_name(fa)
                    ));
                };
                tm.insert(ins.addr as i64, t.clone());
                a.tabjumps.push((ins.addr as i64, t));
                continue;
            }
            let v = (base.unwrap_or(0), width.unwrap_or(0));
            jm.insert(ins.addr as i64, v);
            a.jumps.push((ins.addr as i64, v));
        }
        let inwin: HashSet<u64> = ws.iter().flat_map(|w| w.iter().map(|x| x.addr)).collect();
        let heads: HashSet<u64> = ws.iter().map(|w| w[0].addr).collect();
        let mut rmid: HashSet<u64> = HashSet::new();
        for (k, x) in insl.iter().enumerate() {
            if !is_direct_branch(x.mn) {
                continue;
            }
            let Some(t) = x.target() else { continue };
            if inwin.contains(&t) && !heads.contains(&t) {
                let Some(wseg) = ws.iter().find(|w| w.iter().any(|y| y.addr == t)) else {
                    continue;
                };
                let rb = if !matches!(x.mn, "b" | "bl") {
                    retry_block(&insl, k, wseg)
                } else {
                    None
                };
                let Some(rb) = rb else {
                    return fail(format!("assertion failed: branch into window {:x}", x.addr));
                };
                if rb.0.iter().any(|y| a.done.contains(&(y.addr as i64))) {
                    return fail(format!(
                        "assertion failed: retry block overlaps a window {:x}",
                        rb.0[0].addr
                    ));
                }
                let wend = wseg[wseg.len() - 1].addr as i64 + 4;
                for y in &rb.0 {
                    a.done.insert(y.addr as i64);
                }
                rmid.extend(rb.0[1..].iter().map(|y| y.addr));
                a.retries
                    .push(vec![(rb.0, x.addr as i64 + 4), (rb.1, wend)]);
            }
        }
        for x in &insl {
            if is_direct_branch(x.mn) && x.target().is_some_and(|t| rmid.contains(&t)) {
                return fail(format!(
                    "assertion failed: branch into retry block {:x}",
                    x.addr
                ));
            }
        }
        for (a_, tgt) in &a.tabjumps {
            if fa <= *a_
                && *a_ < fb
                && tgt
                    .iter()
                    .any(|t| inwin.contains(&(*t as u64)) && !heads.contains(&(*t as u64)))
            {
                return fail(format!(
                    "assertion failed: jump table target into window {a_:x}"
                ));
            }
        }
        rmid_all.extend(rmid.iter().map(|x| *x as i64));
    }
    if !far_into.is_empty() {
        let win_mid: HashSet<i64> = a
            .windows
            .iter()
            .flat_map(|w| w[1..].iter().map(|x| x.addr as i64))
            .collect();
        for (at, to) in &far_into {
            if win_mid.contains(to) || rmid_all.contains(to) {
                return fail(format!(
                    "assertion failed: far branch into a window or retry block {at:x}"
                ));
            }
        }
        for (at, tgt) in &a.tabjumps {
            if tgt
                .iter()
                .any(|t| win_mid.contains(t) || rmid_all.contains(t))
            {
                return fail(format!(
                    "assertion failed: jump table target into a window or retry block {at:x}"
                ));
            }
        }
    }
    Ok(a)
}
