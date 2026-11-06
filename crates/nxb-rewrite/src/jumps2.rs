//! Jump table recognizers, part two: the masked and complemented index forms, adr-relative tables, byte and
//! halfword tables, and indirect tail calls.

use std::collections::HashSet;

use nxb_a64::{Insn, Reg};

use crate::jumps::{An, add3, and_imm, cmp_imm, mov_ww, wr_named, xnum};
use crate::pat::{is_direct_branch, ldrsw_idx, nops, pimm, preg, tx, writes_reg, written_w};
use crate::rw::R;

impl An<'_> {
    pub(crate) fn in_img(&self, tb: i64, n: usize) -> bool {
        tb >= 0 && tb + 4 * n as i64 <= self.img.len() as i64
    }

    /// A loop's base initialized after its dispatch in address order.
    pub(crate) fn forward_table_base(
        &self,
        insl: &[Insn],
        k: usize,
        rb: &str,
        n: usize,
    ) -> R<Option<i64>> {
        if k < 2 || k + 2 >= insl.len() {
            return Ok(None);
        }
        let w = &insl[k - 2..k + 3];
        let (guard, branch, load, add, jump) = (&w[0], &w[1], &w[2], &w[3], &w[4]);
        let Some((dst, base, ix)) = ldrsw_idx(load) else {
            return Ok(None);
        };
        if tx(base) != rb || !matches!(dst, Reg::X(_)) || guard.mn != "cmp" {
            return Ok(None);
        }
        let ok_bound = nops(guard, 2)
            && preg(guard, 0) == Some(Reg::W(ix))
            && pimm(guard, 1).is_some_and(|v| v as usize == n.wrapping_sub(1));
        if !ok_bound {
            return Ok(None);
        }
        if branch.mn != "b.hi"
            || add.mn != "add"
            || add.op_str() != format!("x18, {}, {rb}", tx(dst))
            || jump.mn != "br"
            || jump.op_str() != "x18"
        {
            return Ok(None);
        }
        if w.windows(2).any(|p| p[1].addr != p[0].addr + 4) {
            return Ok(None);
        }
        let mut protected: HashSet<u64> = insl[k - 1..k + 3].iter().map(|p| p.addr).collect();
        if insl.iter().any(|p| matches!(p.mn, "bl" | "blr")) && xnum(rb) <= 18 {
            return Ok(None);
        }
        let mut names: HashSet<String> = HashSet::from([rb.to_string(), format!("w{}", &rb[1..])]);
        if rb == "x30" {
            names.insert("lr".into());
        }
        if rb == "x29" {
            names.insert("fp".into());
        }
        let writers: Vec<(usize, &Insn)> = insl
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                !matches!(p.mn, "cmp" | "cmn" | "tst")
                    && p.writes.iter().any(|r| names.contains(&r.name()))
            })
            .collect();
        if writers.len() != 2 {
            return Ok(None);
        }
        let ((i, p), (_, q)) = (writers[0], writers[1]);
        if i <= k
            || q.addr != p.addr + 4
            || p.mn != "adrp"
            || !p.reg(0).is_some_and(|r| tx(r) == rb)
        {
            return Ok(None);
        }
        let Some(lo) = crate::jumps::add_self_imm(q, rb) else {
            return Ok(None);
        };
        protected.insert(q.addr);
        for x in insl {
            if is_direct_branch(x.mn) && x.target().is_some_and(|t| protected.contains(&t)) {
                return Ok(None);
            }
        }
        if self
            .tabjumps
            .values()
            .any(|t| t.iter().any(|a| protected.contains(&(*a as u64))))
        {
            return Ok(None);
        }
        if self.jumps.values().any(|(b, wd)| {
            protected
                .iter()
                .any(|x| *b <= *x as i64 && (*x as i64) < b + 4 * *wd as i64)
        }) {
            return Ok(None);
        }
        let tb = p.imm(1).unwrap_or(0) + lo as i64;
        if !(0 <= tb && tb <= self.img.len() as i64 - 4 * n as i64) {
            return Ok(None);
        }
        let targets: HashSet<i64> = self.table_entries(tb, n)?.into_iter().collect();
        let addrs: HashSet<i64> = insl.iter().map(|p| p.addr as i64).collect();
        let hit = targets.iter().any(|t| protected.contains(&(*t as u64)));
        Ok((!hit && targets.is_subset(&addrs)).then_some(tb))
    }

    /// A zero-guarded complemented index.
    pub(crate) fn jump_masked_not(&self, insl: &[Insn], j: usize) -> R<Option<Vec<i64>>> {
        let lo0 = j.saturating_sub(16);
        let ctx = &insl[lo0..=j];
        let cl = ctx.len();
        let Some(ka) = ctx[..cl - 1]
            .iter()
            .rposition(|p| add3(p, Some(18)).is_some())
        else {
            return Ok(None);
        };
        let (_, ent, base) = add3(&ctx[ka], Some(18)).unwrap_or((Reg::Xzr, Reg::Xzr, Reg::Xzr));
        let Some(kl) = ctx[..ka]
            .iter()
            .rposition(|p| ldrsw_idx(p).is_some_and(|(t, b, _)| t == ent && b == base))
        else {
            return Ok(None);
        };
        let idx_n = ldrsw_idx(&ctx[kl]).map_or(0, |(_, _, x)| x);
        let idx = Reg::X(idx_n);
        if base == ent || base == idx {
            return Ok(None);
        }
        let Some(km) = ctx[..kl].iter().rposition(|p| {
            p.mn == "and"
                && preg(p, 0) == Some(idx)
                && matches!(preg(p, 1), Some(Reg::X(_)))
                && and_imm(p).is_some()
        }) else {
            return Ok(None);
        };
        if km < 3 {
            return Ok(None);
        }
        let mask = &ctx[km];
        let (src_n, imm) = (
            and_imm(mask).map_or(0, |x| x.1),
            and_imm(mask).map_or(0, |x| x.2),
        );
        if imm == 0 || imm & (imm + 1) != 0 || imm > 0xffff_ffff {
            return Ok(None);
        }
        let n = imm as usize;
        let mask_txt = mask.op_str();
        let imm_txt = mask_txt.rsplit(", ").next().unwrap_or("").to_string();
        let (test, branch, invert) = (&ctx[km - 3], &ctx[km - 2], &ctx[km - 1]);
        if test.mn != "tst"
            || test.op_str() != format!("w{src_n}, {imm_txt}")
            || branch.mn != "b.eq"
            || invert.mn != "mvn"
            || invert.op_str() != format!("w{src_n}, w{src_n}")
        {
            return Ok(None);
        }
        let run = &ctx[km - 3..];
        if run.windows(2).any(|p| p[1].addr != p[0].addr + 4) {
            return Ok(None);
        }
        let (head, end) = (test.addr, insl[j].addr);
        let bt = branch.imm(0).unwrap_or(0) as u64;
        if head <= bt && bt <= end {
            return Ok(None);
        }
        let writes = |p: &Insn, reg: &str| -> bool {
            if matches!(p.mn, "cmp" | "cmn" | "tst") {
                return false;
            }
            if matches!(p.mn, "bl" | "blr") && xnum(reg) <= 18 {
                return true;
            }
            writes_reg(p, reg)
        };
        let (idt, entt, baset) = (tx(idx), tx(ent), tx(base));
        if ctx[km + 1..kl].iter().any(|p| writes(p, &idt))
            || ctx[kl + 1..ka].iter().any(|p| writes(p, &entt))
            || ctx[kl + 1..=ka].iter().any(|p| writes(p, &baset))
            || ctx[ka + 1..cl - 1].iter().any(|p| writes(p, "x18"))
        {
            return Ok(None);
        }
        if ctx[km + 1..cl - 1].iter().any(|p| {
            matches!(
                p.mn,
                "b" | "bl" | "br" | "blr" | "ret" | "cbz" | "cbnz" | "tbz" | "tbnz"
            ) || p.mn.starts_with("b.")
        }) {
            return Ok(None);
        }
        for p in insl {
            if is_direct_branch(p.mn) && p.target().is_some_and(|t| head < t && t <= end) {
                return Ok(None);
            }
        }
        if self
            .tabjumps
            .values()
            .any(|t| t.iter().any(|a| head < *a as u64 && *a as u64 <= end))
        {
            return Ok(None);
        }
        if self
            .jumps
            .values()
            .any(|(b, wd)| *b as u64 <= end && (*b + 4 * *wd as i64) as u64 > head + 4)
        {
            return Ok(None);
        }
        let Some(tb) = self.copied_base(insl, lo0 + kl, &baset) else {
            return Ok(None);
        };
        if !self.in_img(tb, n) {
            return Ok(None);
        }
        let targets = self.table_entries(tb, n)?;
        let addrs: HashSet<i64> = insl.iter().map(|p| p.addr as i64).collect();
        let ok = targets
            .iter()
            .all(|t| addrs.contains(t) && !(head < *t as u64 && *t as u64 <= end));
        Ok(ok.then_some(targets))
    }

    /// Entries relative to an `adr`.
    pub(crate) fn jump_adr_offsets(&self, insl: &[Insn], j: usize) -> R<Option<Vec<i64>>> {
        let lo0 = j.saturating_sub(16);
        let ctx = &insl[lo0..j];
        let Some(ka) = ctx.iter().rposition(|p| add3(p, Some(18)).is_some()) else {
            return Ok(None);
        };
        let (_, o1, o2) = add3(&ctx[ka], Some(18)).unwrap_or((Reg::Xzr, Reg::Xzr, Reg::Xzr));
        for (e, a) in [(o1, o2), (o2, o1)] {
            let Some(kld) = ctx[..ka]
                .iter()
                .rposition(|p| ldrsw_idx(p).is_some_and(|(t, _, _)| t == e))
            else {
                continue;
            };
            let at_t = tx(a);
            let Some(wa) = ctx[..ka].iter().rev().find(|p| wr_named(p, &at_t)) else {
                continue;
            };
            if wa.mn != "adr" {
                continue;
            }
            let Some((_, tb_reg, ix)) = ldrsw_idx(&ctx[kld]) else {
                continue;
            };
            let mut idx: HashSet<u8> = HashSet::from([ix]);
            let mut cmp: Option<&Insn> = None;
            for p in ctx[..kld].iter().rev() {
                if let Some((x, y)) = mov_ww(p)
                    && idx.contains(&x)
                {
                    idx.remove(&x);
                    idx.insert(y);
                    continue;
                }
                // a check of the full xN index bounds the same table
                if let Some((cn, _)) = cmp_imm(p, true)
                    && idx.contains(&cn)
                {
                    cmp = Some(p);
                    break;
                }
                if written_w(p)
                    .iter()
                    .any(|x| idx.contains(&(*x as u8)) && *x < 256)
                {
                    return Ok(None);
                }
            }
            let Some(cmp) = cmp else { return Ok(None) };
            let n = pimm(cmp, 1).unwrap_or(0) as usize + 1;
            let at = wa.imm(1).unwrap_or(0);
            let targets = |tb: i64| -> R<Option<Vec<i64>>> {
                if tb >= 0 && tb + 4 * n as i64 <= self.img.len() as i64 {
                    (0..n as i64)
                        .map(|i| Ok(at + self.i32_at(tb + 4 * i)?))
                        .collect::<R<Vec<_>>>()
                        .map(Some)
                } else {
                    Ok(None)
                }
            };
            let tbt = tx(tb_reg);
            if let Some(tb) = self.table_base(insl, lo0 + kld, &tbt, false) {
                return targets(tb);
            }
            let fa = insl[0].addr as i64;
            let fb = insl[insl.len() - 1].addr as i64 + 4;
            let mut ok = Vec::new();
            for tb in self.table_cands(insl, &tbt) {
                if let Some(t) = targets(tb)?
                    && t.iter().all(|x| fa <= *x && *x < fb && x % 4 == 0)
                {
                    ok.push(tb);
                }
            }
            return if ok.len() == 1 {
                targets(ok[0])
            } else {
                Ok(None)
            };
        }
        Ok(None)
    }
}
