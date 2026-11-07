//! Jump table recognizers, part one: the offset table form (`jump_offsets`) and the base register helpers.

use std::collections::{HashMap, HashSet};

use nxb_a64::{Insn, Op, Reg};

use crate::error::fail;
use crate::pat::{
    canon, gn, ldrsw_idx, nops, pimm, preg, tx, writes_reg, writes_reg_any, written_w,
};
use crate::rw::R;

/// Context of the recognizers: the image and the jump tables found so far.
pub(crate) struct An<'a> {
    pub(crate) img: &'a [u8],
    pub(crate) jumps: &'a HashMap<i64, (i64, usize)>,
    pub(crate) tabjumps: &'a HashMap<i64, Vec<i64>>,
}

/// `add xA, xB, xC` shapes: `(A, B, C)` for `add x18, xB, xC` style matches with the given first register.
pub(crate) fn add3(p: &Insn, first: Option<u8>) -> Option<(Reg, Reg, Reg)> {
    if p.mn != "add" || !nops(p, 3) {
        return None;
    }
    let (a, b, c) = (preg(p, 0)?, preg(p, 1)?, preg(p, 2)?);
    match (a, b, c) {
        (Reg::X(a8), Reg::X(_), Reg::X(_)) if first.is_none_or(|f| f == a8) => Some((a, b, c)),
        _ => None,
    }
}

/// `mov wA, wB`: `(A, B)`.
pub(crate) fn mov_ww(p: &Insn) -> Option<(u8, u8)> {
    if p.mn != "mov" || !nops(p, 2) {
        return None;
    }
    match (preg(p, 0)?, preg(p, 1)?) {
        (Reg::W(a), Reg::W(b)) => Some((a, b)),
        _ => None,
    }
}

/// `cmp wN, #imm` (or `xN` when `wide`): `(N, imm)`.
pub(crate) fn cmp_imm(p: &Insn, wide: bool) -> Option<(u8, u64)> {
    if p.mn != "cmp" || !nops(p, 2) {
        return None;
    }
    let n = match preg(p, 0)? {
        Reg::W(n) => n,
        Reg::X(n) if wide => n,
        _ => return None,
    };
    Some((n, pimm(p, 1)?))
}

/// How many entries the compare `ctx[kc]` (`cmp index, #n`) lets the table load `ctx[kl]` read, or `None`
/// when no branch on its flags guards the load. A compare bounds the index only through the branch that acts
/// on it: `cmp w18,#4; b.hi exit; cmp w18,#1; ldrsw x18,[x4,x18,lsl #2]` has a nearer compare that bounds
/// nothing. Between the compare and the load, the first branch on an unsigned condition decides, with the
/// flags still the compare's (no instruction that sets them and no call in between):
/// - `b.hi` out gives n + 1 entries, `b.hs` / `b.cs` out gives n. The branch leaves the dispatch.
/// - `b.ls` in gives n + 1, `b.lo` / `b.cc` in gives n. The branch enters the dispatch, and the instruction
///   before its target is a `b`, `br` or `ret`, so the failed check does not fall into it.
///
/// Any other branch on the flags (`b.eq`, `b.ge`, ...) is no bounds check, and neither is a compare with none.
pub(crate) fn guarded_count(ctx: &[Insn], kc: usize, kl: usize, n: u64) -> Option<usize> {
    const SETS_FLAGS: &[&str] = &[
        "cmp", "cmn", "tst", "ccmp", "ccmn", "fcmp", "fcmpe", "fccmp", "fccmpe", "adds", "subs",
        "ands", "bics", "adcs", "sbcs", "negs", "ngcs", "msr", "bl", "blr",
    ];
    let n = n as usize;
    let ld = ctx.get(kl)?.addr;
    for q in kc + 1..kl {
        let p = &ctx[q];
        if matches!(p.mn, "b.hi" | "b.hs" | "b.cs" | "b.ls" | "b.lo" | "b.cc") {
            let t = p.target()?;
            let inside = p.addr < t && t <= ld;
            if matches!(p.mn, "b.hi" | "b.hs" | "b.cs") {
                return (!inside).then_some(n + usize::from(p.mn == "b.hi"));
            }
            let before = ctx[q + 1..=kl]
                .iter()
                .find(|x| x.addr + 4 == t)
                .is_some_and(|x| matches!(x.mn, "b" | "br" | "ret"));
            return (inside && before).then_some(n + usize::from(p.mn == "b.ls"));
        }
        if SETS_FLAGS.contains(&p.mn)
            || p.mn.starts_with("b.")
            || matches!(p.mn, "b" | "br" | "ret")
        {
            return None;
        }
    }
    None
}

/// `and rA, rB, #imm` with w or x registers: `(A, B, imm)`.
pub(crate) fn and_imm(p: &Insn) -> Option<(u8, u8, u64)> {
    if p.mn != "and" || !nops(p, 3) {
        return None;
    }
    Some((gn(preg(p, 0)?)?, gn(preg(p, 1)?)?, pimm(p, 2)?))
}

/// `add xR, xR, #imm`: the immediate.
pub(crate) fn add_self_imm(p: &Insn, rb: &str) -> Option<u64> {
    if p.mn != "add" || !nops(p, 3) {
        return None;
    }
    let (a, b) = (preg(p, 0)?, preg(p, 1)?);
    if !matches!(a, Reg::X(_)) || tx(a) != rb || tx(b) != rb {
        return None;
    }
    pimm(p, 2)
}

/// The number of an `xN` text.
pub(crate) fn xnum(s: &str) -> i64 {
    s[1..].parse().unwrap_or(0)
}

/// Whether a write of `p` is named `name` (`x5`).
pub(crate) fn wr_named(p: &Insn, name: &str) -> bool {
    writes_reg(p, name)
}

impl An<'_> {
    pub(crate) fn i32_at(&self, o: i64) -> R<i64> {
        match self.img.get(o as usize..o as usize + 4) {
            Some(s) if o >= 0 => Ok(i64::from(i32::from_le_bytes([s[0], s[1], s[2], s[3]]))),
            _ => fail(format!("struct.error reading {o:#x}")),
        }
    }

    fn u16_at(&self, o: i64) -> R<i64> {
        match self.img.get(o as usize..o as usize + 2) {
            Some(s) if o >= 0 => Ok(i64::from(u16::from_le_bytes([s[0], s[1]]))),
            _ => fail(format!("struct.error reading {o:#x}")),
        }
    }

    pub(crate) fn table_entries(&self, tb: i64, n: usize) -> R<Vec<i64>> {
        (0..n as i64)
            .map(|i| Ok(tb + self.i32_at(tb + 4 * i)?))
            .collect()
    }

    pub(crate) fn halfwords(&self, tb: i64, n: usize) -> R<Vec<i64>> {
        (0..n as i64).map(|i| self.u16_at(tb + 2 * i)).collect()
    }

    /// The writer chain of a table base: `add rb, rb, #lo` after `adrp rb`.
    pub(crate) fn table_base(&self, insl: &[Insn], k: usize, rb: &str, far: bool) -> Option<i64> {
        let mut lo: Option<u64> = None;
        let from = if far { 0 } else { k.saturating_sub(256) };
        for p in insl[from..k].iter().rev() {
            if far && matches!(p.mn, "bl" | "blr") && xnum(rb) <= 18 {
                return None;
            }
            if !wr_named(p, rb) {
                continue;
            }
            if lo.is_none()
                && let Some(v) = add_self_imm(p, rb)
            {
                lo = Some(v);
                continue;
            }
            if let Some(l) = lo
                && p.mn == "adrp"
                && p.reg(0).is_some_and(|r| tx(r) == rb)
            {
                return Some(p.imm(1)? + l as i64);
            }
            return None;
        }
        None
    }

    /// The address `rb` holds at `insl[k]` when it was built by `adrp` + `add` and only copied since.
    pub(crate) fn copied_base(&self, insl: &[Insn], k: usize, rb: &str) -> Option<i64> {
        let mut lo: Option<u64> = None;
        let mut r = rb.to_string();
        for p in insl[..k].iter().rev() {
            if matches!(p.mn, "bl" | "blr") && xnum(&r) <= 18 {
                return None;
            }
            if !p.writes.iter().any(|x| canon(x) == r) {
                continue;
            }
            let t0 = match p.ops.first() {
                Some(Op::Reg(x, _)) => Some(tx(*x)),
                _ => None,
            };
            if lo.is_none()
                && p.mn == "mov"
                && nops(p, 2)
                && t0.as_deref() == Some(&r)
                && let Some(Reg::X(n)) = preg(p, 1)
            {
                r = format!("x{n}");
                continue;
            }
            if lo.is_none()
                && p.mn == "add"
                && nops(p, 3)
                && t0.as_deref() == Some(&r)
                && preg(p, 1).is_some_and(|x| tx(x) == r)
                && let Some(v) = pimm(p, 2)
            {
                lo = Some(v);
                continue;
            }
            if let Some(l) = lo
                && p.mn == "adrp"
                && t0.as_deref() == Some(&r)
            {
                return Some(p.imm(1)? + l as i64);
            }
            return None;
        }
        None
    }

    /// Every address an `adrp rb` + `add rb, rb, #lo` pair in the function builds, sorted.
    pub(crate) fn table_cands(&self, insl: &[Insn], rb: &str) -> Vec<i64> {
        let mut c = HashSet::new();
        for (k, p) in insl.iter().enumerate() {
            if p.mn != "adrp" || !p.reg(0).is_some_and(|r| tx(r) == rb) {
                continue;
            }
            let q = insl[k + 1..(k + 9).min(insl.len())]
                .iter()
                .find(|x| wr_named(x, rb));
            if let Some(q) = q
                && let Some(v) = add_self_imm(q, rb)
                && let Some(i) = p.imm(1)
            {
                c.insert(i + v as i64);
            }
        }
        let mut v: Vec<i64> = c.into_iter().collect();
        v.sort_unstable();
        v
    }

    fn check_before_copy(
        &self,
        insl: &[Insn],
        lo0: usize,
        k: usize,
        ld: usize,
        idx: &HashSet<u8>,
    ) -> Option<usize> {
        // k: index of the load in ctx, ld: its index in insl
        if k < 2 || insl[lo0 + k - 1].mn != "b.hi" || insl[lo0 + k - 2].mn != "cmp" {
            return None;
        }
        let (src, _) = cmp_imm(&insl[lo0 + k - 2], false)?;
        let kl = ld;
        let writes = |p: &Insn| -> HashSet<u32> { written_w(p) };
        let mut found = false;
        let mut q = kl as i64 - 3;
        while q >= 0 {
            let p = &insl[q as usize];
            if p.addr + 4 != insl[q as usize + 1].addr {
                return None;
            }
            if matches!(p.mn, "bl" | "blr")
                && idx.iter().chain(std::iter::once(&src)).any(|r| *r <= 18)
            {
                return None;
            }
            if let Some((a, b)) = mov_ww(p)
                && idx.contains(&a)
                && b == src
            {
                found = true;
                break;
            }
            let w = writes(p);
            if idx
                .iter()
                .chain(std::iter::once(&src))
                .any(|r| w.contains(&u32::from(*r)))
            {
                return None;
            }
            q -= 1;
        }
        if !found {
            return None;
        }
        Some(k - 2)
    }

    /// The offset-table form: `cmp w18,#N; b.hi; ldrsw x18,[xB,x18,lsl #2]; add x18,x18,xB; br x18`.
    pub(crate) fn jump_offsets(
        &self,
        insl: &[Insn],
        j: usize,
        copy_first: bool,
        far_base: bool,
        forward_base: bool,
    ) -> R<Option<Vec<i64>>> {
        let lo0 = j.saturating_sub(16);
        let ctx = &insl[lo0..j];
        let Some((_, t, rb)) = ctx.iter().rev().find_map(|p| add3(p, Some(18))) else {
            return Ok(None);
        };
        let Some((k0, ix)) = ctx.iter().enumerate().rev().find_map(|(i, p)| {
            ldrsw_idx(p)
                .filter(|(tt, bb, _)| *tt == t && *bb == rb)
                .map(|(_, _, x)| (i, x))
        }) else {
            return Ok(None);
        };
        let mut idx: HashSet<u8> = HashSet::from([ix]);
        let mut masked: Option<(u8, u64, usize)> = None;
        let mut cmp: Option<usize> = None;
        let mut n = 0usize;
        // a copy replaces the index (a check of its old value bounds nothing).`changed`: the index is written
        // after the check that the walk finds
        let (mut copies, mut changed): (HashSet<u8>, bool) = (HashSet::new(), false);
        let mut i = k0 as i64 - 1;
        while i >= 0 {
            let p = &ctx[i as usize];
            if let Some((a, b)) = mov_ww(p)
                && idx.contains(&a)
            {
                idx.remove(&a);
                idx.insert(b);
                copies.insert(a);
                i -= 1;
                continue;
            }
            if let Some((a, b, v)) = and_imm(p)
                && idx.contains(&a)
                && masked.is_none()
            {
                masked = Some((b, v, i as usize));
                i -= 1;
                continue;
            }
            let Some((cn, cv)) = cmp_imm(p, far_base) else {
                // a call counts for the caller-saved registers (Capstone lists only x30 as its write)
                if writes_reg_any(p, &idx)
                    || (matches!(p.mn, "bl" | "blr") && idx.iter().any(|r| *r <= 18))
                {
                    changed = true;
                }
                i -= 1;
                continue;
            };
            // only a compare a branch acts on bounds the index. One that guards nothing only sets the flags,
            // and the walk goes on to the check before it
            if idx.contains(&cn) {
                if let Some(g) = guarded_count(ctx, i as usize, k0, cv) {
                    cmp = Some(i as usize);
                    n = g;
                    break;
                }
                i -= 1;
                continue;
            }
            if let Some((mreg, mmask, mpos)) = masked {
                let mut q = i - 1;
                while q >= 0 {
                    let cq = &ctx[q as usize];
                    let w = written_w(cq);
                    if !w.contains(&u32::from(cn)) {
                        q -= 1;
                        continue;
                    }
                    if let Some((_, b, v)) = and_imm(cq)
                        && b == mreg
                        && v == mmask
                        && !(q as usize + 1..mpos)
                            .any(|z| written_w(&ctx[z]).contains(&u32::from(mreg)))
                        && let Some(g) = guarded_count(ctx, i as usize, k0, cv)
                    {
                        cmp = Some(i as usize);
                        n = g;
                    }
                    break;
                }
                if cmp.is_some() {
                    break;
                }
            }
            i -= 1;
        }
        if cmp.is_some() && changed {
            return Ok(None);
        }
        if cmp.is_none() && copy_first {
            let all: HashSet<u8> = idx.union(&copies).copied().collect();
            // cmp; b.hi right before the load
            cmp = self.check_before_copy(insl, lo0, k0, lo0 + k0, &all);
            if let Some(ci) = cmp {
                n = pimm(&insl[lo0 + ci], 1).unwrap_or(0) as usize + 1;
            }
        }
        if cmp.is_none() {
            return Ok(None);
        }
        let k = lo0 + k0;
        let rbt = tx(rb);
        let (mut lo, mut page): (Option<u64>, Option<i64>) = (None, None);
        let mut broke = false;
        for p in insl[k.saturating_sub(256)..k].iter().rev() {
            if !wr_named(p, &rbt) {
                continue;
            }
            if lo.is_none()
                && let Some(v) = add_self_imm(p, &rbt)
            {
                lo = Some(v);
                continue;
            }
            if lo.is_some() && p.mn == "adrp" && p.reg(0).is_some_and(|r| tx(r) == rbt) {
                page = p.imm(1);
            }
            broke = true;
            break;
        }
        let fa = insl[0].addr as i64;
        let fb = insl[insl.len() - 1].addr as i64 + 4;
        let all_in = |t: &[i64]| t.iter().all(|x| fa <= *x && *x < fb && x % 4 == 0);
        if !broke && lo.is_none() && far_base {
            let mut tb = self.table_base(insl, k, &rbt, true);
            if tb.is_none() {
                tb = self.copied_base(insl, k, &rbt);
            }
            if tb.is_none() && forward_base {
                tb = self.forward_table_base(insl, k, &rbt, n)?;
            }
            let Some(tb) = tb else { return Ok(None) };
            if tb + 4 * n as i64 > self.img.len() as i64 {
                return Ok(None);
            }
            let t = self.table_entries(tb, n)?;
            return Ok(all_in(&t).then_some(t));
        }
        if page.is_none() && far_base {
            let Some(tb) = self.copied_base(insl, k, &rbt) else {
                return Ok(None);
            };
            if tb < 0 || tb + 4 * n as i64 > self.img.len() as i64 {
                return Ok(None);
            }
            let t = self.table_entries(tb, n)?;
            return Ok(all_in(&t).then_some(t));
        }
        let Some(page) = page else { return Ok(None) };
        let tb = page + lo.unwrap_or(0) as i64;
        Ok(Some(self.table_entries(tb, n)?))
    }
}
