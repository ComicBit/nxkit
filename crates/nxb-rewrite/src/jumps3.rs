//! Jump table recognizers, part three: the masked-predecessor form, tail calls, byte and halfword tables.

use std::collections::HashSet;

use nxb_a64::{Insn, Op, Reg, Sh, Wb};

use crate::jumps::{An, cmp_imm, mov_ww};
use crate::pat::{gn, is_branch, ldrsw_idx, memop, nops, pimm, preg, tx, wname};
use crate::rw::R;

impl An<'_> {
    /// The check on a masked copy made in the block, the index masked in the block that branches to it.
    pub(crate) fn jump_masked_pred(&self, insl: &[Insn], j: usize) -> R<Option<Vec<i64>>> {
        let is_cf = |p: &Insn| is_branch(p.mn) || matches!(p.mn, "br" | "blr" | "bl" | "ret");
        let writes = |p: &Insn| -> HashSet<u32> {
            if matches!(p.mn, "cmp" | "cmn" | "tst") {
                HashSet::new()
            } else {
                p.writes
                    .iter()
                    .filter_map(|r| r.gpr().map(u32::from))
                    .collect()
            }
        };
        let floor = j.saturating_sub(32);
        let Some(kh) = (floor..j).rev().find(|q| is_cf(&insl[*q])) else {
            return Ok(None);
        };
        if kh < 3 || insl[kh].mn != "b.hi" {
            return Ok(None);
        }
        let k = kh - 2;
        let (a_, c_) = (&insl[kh - 2], &insl[kh - 1]);
        let ma = if a_.mn == "and" && nops(a_, 3) {
            match (preg(a_, 0), preg(a_, 1), pimm(a_, 2)) {
                (Some(Reg::W(d)), Some(Reg::W(s)), Some(m)) => Some((d, s, m)),
                _ => None,
            }
        } else {
            None
        };
        let mc = cmp_imm(c_, false);
        let (Some((ad, asrc, mask)), Some((cd, cimm))) = (ma, mc) else {
            return Ok(None);
        };
        if ad != cd {
            return Ok(None);
        }
        if (k.saturating_sub(1)..j).any(|q| insl[q + 1].addr != insl[q].addr + 4) {
            return Ok(None);
        }
        let Some(kl) =
            (kh + 1..j).find(|q| insl[*q].mn == "ldrsw" && preg(&insl[*q], 0) == Some(Reg::X(18)))
        else {
            return Ok(None);
        };
        let Some((_, rb, ixn)) = ldrsw_idx(&insl[kl]).filter(|(t, _, _)| *t == Reg::X(18)) else {
            return Ok(None);
        };
        let ix = u32::from(ixn);
        let (src, n) = (u32::from(asrc), cimm as usize + 1);
        let rbn = gn(rb).map_or(99, u32::from);
        if [src, u32::from(ad), 18].contains(&ix) || [ix, 18].contains(&rbn) || mask >> 32 != 0 {
            return Ok(None);
        }
        if insl[kh + 1..kl].iter().any(|p| writes(p).contains(&ix)) {
            return Ok(None);
        }
        let Some(ka) = (kl + 1..j).find(|q| writes(&insl[*q]).contains(&18)) else {
            return Ok(None);
        };
        let rbt = tx(rb);
        if insl[ka].mn != "add" || insl[ka].op_str() != format!("x18, x18, {rbt}") {
            return Ok(None);
        }
        if insl[kl + 1..ka].iter().any(|p| writes(p).contains(&rbn))
            || insl[ka + 1..j].iter().any(|p| writes(p).contains(&18))
        {
            return Ok(None);
        }
        if !matches!(insl[k - 1].mn, "b" | "br" | "ret") {
            return Ok(None);
        }
        let brs: Vec<usize> = insl
            .iter()
            .enumerate()
            .filter(|(_, p)| (is_branch(p.mn) || p.mn == "bl") && p.target().is_some())
            .map(|(q, _)| q)
            .collect();
        let into: HashSet<u64> = brs.iter().filter_map(|q| insl[*q].target()).collect();
        if insl[k + 1..=j].iter().any(|p| into.contains(&p.addr)) {
            return Ok(None);
        }
        let tabs: HashSet<i64> = self.tabjumps.values().flatten().copied().collect();
        let table_target = |a: i64| {
            tabs.contains(&a)
                || self
                    .jumps
                    .values()
                    .any(|(b, w)| *b <= a && a < *b + 4 * *w as i64)
        };
        if insl[k..=j].iter().any(|p| table_target(p.addr as i64)) {
            return Ok(None);
        }
        let preds: Vec<usize> = brs
            .iter()
            .copied()
            .filter(|q| insl[*q].target() == Some(a_.addr))
            .collect();
        if preds.is_empty() || preds.iter().any(|q| insl[*q].mn == "bl") {
            return Ok(None);
        }
        for q in preds {
            let mut done = false;
            let mut i = q as i64 - 1;
            let stop = (q as i64 - 16).max(0);
            while i >= stop {
                let p = &insl[i as usize];
                let m = if p.mn == "and" && nops(p, 3) {
                    match (preg(p, 0).and_then(gn), preg(p, 1).and_then(gn), pimm(p, 2)) {
                        (Some(d), Some(s), Some(v)) => Some((d, s, v)),
                        _ => None,
                    }
                } else {
                    None
                };
                if let Some((d, s, v)) = m
                    && u32::from(d) == ix
                {
                    if u32::from(s) != src || v != mask {
                        return Ok(None);
                    }
                    if insl[i as usize + 1..=q]
                        .iter()
                        .any(|x| into.contains(&x.addr) || table_target(x.addr as i64))
                    {
                        return Ok(None);
                    }
                    if (i as usize..q).any(|z| insl[z + 1].addr != insl[z].addr + 4) {
                        return Ok(None);
                    }
                    done = true;
                    break;
                }
                let w = writes(p);
                if is_cf(p) || w.contains(&ix) || w.contains(&src) {
                    return Ok(None);
                }
                i -= 1;
            }
            if !done {
                return Ok(None);
            }
        }
        let Some(tb) = self.copied_base(insl, kl, &rbt) else {
            return Ok(None);
        };
        if !self.in_img(tb, n) {
            return Ok(None);
        }
        let t = self.table_entries(tb, n)?;
        let addrs: HashSet<i64> = insl.iter().map(|p| p.addr as i64).collect();
        Ok(t.iter().all(|x| addrs.contains(x)).then_some(t))
    }

    fn ldr_x18_mem(p: &Insn) -> bool {
        if p.mn != "ldr" || !nops(p, 2) || preg(p, 0) != Some(Reg::X(18)) {
            return false;
        }
        match memop(p, 1) {
            Some(m) => {
                matches!(m.base, Reg::X(_) | Reg::Sp)
                    && m.index.is_none()
                    && m.wb == Wb::None
                    && m.disp >= 0
            }
            None => false,
        }
    }

    fn tail_call_at_end(insl: &[Insn], j: usize) -> bool {
        if j + 1 != insl.len() {
            return false;
        }
        for p in insl[..j].iter().rev() {
            if !p.writes.iter().any(|r| r.name() == "x18") {
                continue;
            }
            return Self::ldr_x18_mem(p);
        }
        false
    }

    /// The `br x18` is an indirect tail call, not a jump table.
    pub(crate) fn tail_call(insl: &[Insn], j: usize) -> bool {
        if j + 1 == insl.len() {
            return Self::tail_call_at_end(insl, j);
        }
        if j < 4 || insl.windows(2).any(|p| p[1].addr != p[0].addr + 4) {
            return false;
        }
        let suffix = &insl[j + 1..];
        if suffix[0].mn != "bl" || suffix[1..].iter().any(|p| p.mn != "udf") {
            return false;
        }
        let st = suffix[0].target().unwrap_or(0);
        if insl[0].addr <= st && st <= insl[insl.len() - 1].addr {
            return false;
        }
        if insl[0].mn != "stp" || insl[0].op_str() != "x29, x30, [sp, #-0x10]!" {
            return false;
        }
        if insl[1].mn != "mov" || insl[1].op_str() != "x29, sp" {
            return false;
        }
        if insl[j - 1].mn != "ldp" || insl[j - 1].op_str() != "x29, x30, [sp], #0x10" {
            return false;
        }
        let guard = &insl[2];
        if guard.mn != "cbz" || guard.target() != Some(suffix[0].addr) {
            return false;
        }
        let mut load = false;
        for p in &insl[3..j - 1] {
            if !matches!(
                p.mn,
                "mov" | "ldr" | "ldur" | "ldp" | "str" | "stur" | "stp"
            ) {
                return false;
            }
            let w: Vec<String> = p.writes.iter().map(|r| r.name()).collect();
            if w.iter().any(|n| {
                matches!(
                    n.as_str(),
                    "sp" | "wsp" | "fp" | "lr" | "x29" | "w29" | "x30" | "w30"
                )
            }) {
                return false;
            }
            if matches!(p.mn, "str" | "stur" | "stp") {
                if p.wb {
                    return false;
                }
                let Some(Op::Mem {
                    base, index, disp, ..
                }) = p.ops.last()
                else {
                    return false;
                };
                if !matches!(base.name().as_str(), "sp" | "fp" | "x29")
                    || index.is_some()
                    || *disp < 16
                {
                    return false;
                }
            }
            if w.iter().any(|n| n == "x18" || n == "w18") {
                if load || !Self::ldr_x18_mem(p) {
                    return false;
                }
                load = true;
            }
        }
        load
    }

    /// The byte or halfword table form with its `add` further back.
    pub(crate) fn jump_bytes_far(&self, insl: &[Insn], j: usize) -> R<Option<(i64, usize)>> {
        let lo0 = j.saturating_sub(32);
        let ctx = &insl[lo0..j];
        let mut found = None;
        for (k, p) in ctx.iter().enumerate().rev() {
            if p.mn == "add"
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
                )
            {
                found = Some(k);
                break;
            }
            if p.writes.iter().any(|r| r.name() == "x18") {
                return Ok(None);
            }
        }
        let Some(k) = found else { return Ok(None) };
        let idx = match p_reg2(&ctx[k]) {
            Some(r) => gn(r),
            None => None,
        };
        let base = ctx[..k]
            .iter()
            .rev()
            .find(|q| q.mn == "adr" && preg(q, 0) == Some(Reg::X(18)))
            .and_then(|q| q.imm(1));
        let ld = ctx[..k].iter().enumerate().rev().find(|(_, q)| {
            matches!(q.mn, "ldrb" | "ldrsb" | "ldrh" | "ldrsh")
                && preg(q, 0).and_then(|r| r.gpr()).is_some()
                && preg(q, 0).and_then(|r| {
                    if matches!(r, Reg::W(_) | Reg::X(_)) {
                        r.gpr()
                    } else {
                        None
                    }
                }) == idx
        });
        let (Some(base), Some((lq, ld))) = (base, ld) else {
            return Ok(None);
        };
        if matches!(ld.mn, "ldrb" | "ldrsb") {
            return Ok(Some((base, 256)));
        }
        Ok(Some((
            base,
            self.half_count(insl, lo0 + lq, base)?.unwrap_or(65536),
        )))
    }

    /// The landing pads a halfword table needs: the largest entry + 1.
    pub(crate) fn half_count(&self, insl: &[Insn], k: usize, base: i64) -> R<Option<usize>> {
        let ld = &insl[k];
        if ld.mn != "ldrh" || !nops(ld, 2) || !matches!(preg(ld, 0), Some(Reg::W(_))) {
            return Ok(None);
        }
        let Some(m) = memop(ld, 1) else {
            return Ok(None);
        };
        let Some((ir, sh)) = m.index else {
            return Ok(None);
        };
        if !matches!(m.base, Reg::X(_))
            || m.disp != 0
            || !matches!(sh.kind, "lsl" | "uxtw")
            || sh.amt != 1
        {
            return Ok(None);
        }
        let Some(ixn) = gn(ir) else { return Ok(None) };
        let tb_reg = tx(m.base);
        let mut idx: HashSet<u8> = HashSet::from([ixn]);
        let mut cmp = None;
        for q in (k.saturating_sub(32)..k).rev() {
            let p = &insl[q];
            if let Some((a, b)) = mov_ww(p)
                && idx.contains(&a)
            {
                idx.insert(b);
                continue;
            }
            if let Some((cn, _)) = cmp_imm(p, false)
                && idx.contains(&cn)
            {
                cmp = Some(q);
                break;
            }
            if p.writes
                .iter()
                .filter_map(wname)
                .any(|x| x < 256 && idx.contains(&(x as u8)))
            {
                return Ok(None);
            }
        }
        let Some(kc) = cmp else { return Ok(None) };
        if !insl
            .get(kc + 1)
            .is_some_and(|x| matches!(x.mn, "b.hi" | "b.hs" | "b.cs"))
        {
            return Ok(None);
        }
        let n = pimm(&insl[kc], 1).unwrap_or(0) as usize + 1;
        let fa = insl[0].addr as i64;
        let fb = insl[insl.len() - 1].addr as i64 + 4;
        let entries = |tb: i64| -> R<Option<Vec<i64>>> {
            if tb >= 0 && tb + 2 * n as i64 <= self.img.len() as i64 {
                Ok(Some(self.halfwords(tb, n)?))
            } else {
                Ok(None)
            }
        };
        let fits = |e: &Option<Vec<i64>>| {
            e.as_ref()
                .is_some_and(|e| e.iter().all(|x| fa <= base + 4 * x && base + 4 * x < fb))
        };
        let mut tb = self.table_base(insl, k, &tb_reg, false);
        if tb.is_none() {
            let mut ok = Vec::new();
            for c in self.table_cands(insl, &tb_reg) {
                if fits(&entries(c)?) {
                    ok.push(c);
                }
            }
            tb = if ok.len() == 1 { Some(ok[0]) } else { None };
        }
        let e = match tb {
            Some(t) => entries(t)?,
            None => None,
        };
        Ok(if fits(&e) {
            e.and_then(|v| v.into_iter().max()).map(|m| m as usize + 1)
        } else {
            None
        })
    }
}

fn p_reg2(p: &Insn) -> Option<Reg> {
    match p.ops.get(2)? {
        Op::Reg(r, _) => Some(*r),
        _ => None,
    }
}
