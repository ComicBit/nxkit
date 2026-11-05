//! Exclusive windows: `ldxr` ... `stxr` sequences that use x18, and the retry blocks that re-enter them.

use std::collections::HashMap;

use nxb_a64::{Insn, encode};

use crate::error::fail;
use crate::pat::is_direct_branch;
use crate::rw::{K18, R, Rw, dis, pick, reach, regs_in, subst};

pub(crate) const EXCL_LD: [&str; 8] = [
    "ldxr", "ldaxr", "ldxp", "ldaxp", "ldxrb", "ldaxrb", "ldxrh", "ldaxrh",
];
pub(crate) const EXCL_ST: [&str; 8] = [
    "stxr", "stlxr", "stxp", "stlxp", "stxrb", "stlxrb", "stxrh", "stlxrh",
];

/// The exclusive windows of a function (each a copy of its instructions).
pub(crate) fn find_windows(insl: &[Insn]) -> Vec<Vec<Insn>> {
    let mut out = Vec::new();
    for (i, ins) in insl.iter().enumerate() {
        if !EXCL_LD.contains(&ins.mn) {
            continue;
        }
        let mut j = i;
        while j < insl.len() && !EXCL_ST.contains(&insl[j].mn) {
            j += 1;
        }
        if j >= insl.len() || j - i >= 16 {
            continue;
        }
        if insl[i + 1..j].iter().any(|x| EXCL_LD.contains(&x.mn)) {
            continue;
        }
        if j + 1 < insl.len() && matches!(insl[j + 1].mn, "cbz" | "cbnz") {
            j += 1;
        }
        let seg = &insl[i..=j];
        if seg.iter().any(Insn::has18) {
            out.push(seg.to_vec());
        }
    }
    out
}

fn window_adrp(x: &Insn, s: u32, pc: i64) -> R<u32> {
    let rd = x.reg(0).and_then(|r| r.gpr()).map_or(0, u32::from);
    reach(encode::adrp(
        if rd == 18 { s } else { rd },
        pc,
        x.imm(1).unwrap_or(0),
    ))
}

/// Re-encodes a copied conditional branch (`cbz`, `tbz`, `b.cond`) at `pc` to `newt`.
fn rebranch(x: &Insn, w: u32, pc: i64, newt: i64) -> R<u32> {
    let off = (newt - pc) >> 2;
    let w = if matches!(x.mn, "tbz" | "tbnz") {
        (w & 0xfff8_001f) | (((off as u32) & 0x3fff) << 5)
    } else {
        (w & 0xff00_001f) | (((off as u32) & 0x7_ffff) << 5)
    };
    let dd = dis(w, pc)?;
    if dd.target() != Some(newt as u64) || dd.mn != x.mn {
        return fail("assertion failed: copied branch");
    }
    Ok(w)
}

fn check_plain(x: &Insn) -> R<()> {
    let m = x.mn;
    if matches!(m, "adr" | "adrp" | "b" | "bl" | "br" | "blr" | "ret")
        || (m == "ldr" && !x.has_mem())
    {
        return fail(format!("assertion failed: {:#x}", x.addr));
    }
    Ok(())
}

impl Rw {
    fn window_prologue(&mut self, s: u32, t: u32) -> R<()> {
        self.emit(encode::stp(s, t, 31, -16));
        self.emit(encode::mrs_tpidrro(t));
        self.emit_slot_addr(t, s, K18)?;
        self.emit(encode::ldrx(s, t, 0));
        Ok(())
    }

    fn exit_stub(&mut self, s: u32, t: u32, to: i64) -> R<()> {
        self.emit(encode::strx(s, t, 0));
        self.emit(encode::ldp(s, t, 31, -16));
        self.emit_b(to)?;
        self.emit(encode::NOP);
        Ok(())
    }

    /// The rewrite block of a window, returns `(block start, window start, window end)`.
    pub(crate) fn site_window(&mut self, seg: &[Insn]) -> R<(i64, i64, i64)> {
        let mut used = Vec::new();
        for x in seg {
            used.extend(regs_in(x));
        }
        let st = pick(&used, 2)?;
        let (s, t) = (st[0], st[1]);
        let (lo, hi) = (seg[0].addr as i64, seg[seg.len() - 1].addr as i64 + 4);
        let start = self.here();
        self.window_prologue(s, t)?;
        let copy0 = self.here();
        let exits = copy0 + 4 * seg.len() as i64 + 16;
        let mut ex_targets: Vec<i64> = Vec::new();
        let mut words = Vec::new();
        for (k, x) in seg.iter().enumerate() {
            let mut w = x.word;
            if x.has18() {
                w = subst(x, s)?;
            }
            if is_direct_branch(x.mn) && !matches!(x.mn, "b" | "bl") {
                let tgt = x.target().unwrap_or(0) as i64;
                let newt = if lo <= tgt && tgt < hi {
                    copy0 + (tgt - lo)
                } else {
                    ex_targets.push(tgt);
                    exits + 16 * (ex_targets.len() as i64 - 1)
                };
                w = rebranch(x, w, copy0 + 4 * k as i64, newt)?;
            } else if x.mn == "adrp" {
                w = window_adrp(x, s, copy0 + 4 * k as i64)?;
            } else {
                check_plain(x)?;
            }
            words.push(w);
        }
        for w in words {
            self.emit(w);
        }
        let mut all = vec![hi];
        all.extend(ex_targets);
        for t2 in all {
            self.exit_stub(s, t, t2)?;
        }
        Ok((start, lo, hi))
    }

    /// The copy of a retry block and the window's tail under one prologue.
    pub(crate) fn site_retry_window(&mut self, parts: &[(Vec<Insn>, i64)]) -> R<i64> {
        let mut used = Vec::new();
        for (seg, _) in parts {
            for x in seg {
                used.extend(regs_in(x));
            }
        }
        let st = pick(&used, 2)?;
        let (s, t) = (st[0], st[1]);
        let start = self.here();
        self.window_prologue(s, t)?;
        let mut pos: HashMap<u64, i64> = HashMap::new();
        let mut at = self.here();
        for (seg, _) in parts {
            for x in seg {
                pos.insert(x.addr, at);
                at += 4;
            }
            at += 16;
        }
        let exits = at;
        let mut ex_targets: Vec<i64> = Vec::new();
        for (seg, ft) in parts {
            for x in seg {
                let mut w = x.word;
                if x.has18() {
                    w = subst(x, s)?;
                }
                let pc = self.here();
                if is_direct_branch(x.mn) && !matches!(x.mn, "b" | "bl") {
                    let tgt = x.target().unwrap_or(0);
                    if !pos.contains_key(&tgt) && !ex_targets.contains(&(tgt as i64)) {
                        ex_targets.push(tgt as i64);
                    }
                    let newt = match pos.get(&tgt) {
                        Some(p) => *p,
                        None => {
                            exits
                                + 16 * ex_targets
                                    .iter()
                                    .position(|e| *e == tgt as i64)
                                    .unwrap_or(0) as i64
                        }
                    };
                    w = rebranch(x, w, pc, newt)?;
                } else if x.mn == "adrp" {
                    w = window_adrp(x, s, pc)?;
                } else {
                    check_plain(x)?;
                }
                self.emit(w);
            }
            self.exit_stub(s, t, *ft)?;
        }
        for t2 in ex_targets {
            self.exit_stub(s, t, t2)?;
        }
        Ok(start)
    }
}

/// A branch at `insl[k]` that lands in the middle of window `wseg`: accepted only when it ends an out-of-line
/// retry block that re-enters the window at its store-exclusive. Returns `(the retry block, the window from the store on)`.
pub(crate) fn retry_block(
    insl: &[Insn],
    k: usize,
    wseg: &[Insn],
) -> Option<(Vec<Insn>, Vec<Insn>)> {
    let t = insl[k].target()?;
    let tail: Vec<Insn> = wseg.iter().filter(|y| y.addr >= t).cloned().collect();
    if tail.is_empty() || !EXCL_ST.contains(&tail[0].mn) {
        return None;
    }
    let mut p = k as i64 - 1;
    let stop = (k as i64 - 8).max(-1);
    while p > stop {
        let q = &insl[p as usize];
        if EXCL_LD.contains(&q.mn) {
            return Some((insl[p as usize..=k].to_vec(), tail));
        }
        if matches!(
            q.mn,
            "b" | "bl" | "br" | "blr" | "ret" | "cbz" | "cbnz" | "tbz" | "tbnz"
        ) || q.mn.starts_with("b.")
            || EXCL_ST.contains(&q.mn)
        {
            return None;
        }
        p -= 1;
    }
    None
}
