//! Indirect tail calls and the zero-extended jump table form (`tail_call_padded`, `tail_call` and
//! `jump_zero_extended` of `patch_nso.py`).

use std::collections::HashSet;

use nxb_a64::{Insn, Op};

use crate::jumps::An;
use crate::pat::{is_direct_branch, writes_reg};
use crate::rw::R;

/// A decimal or `0x` hexadecimal literal as the script's regexes take it.
fn lit(s: &str) -> Option<i64> {
    match s.strip_prefix("0x") {
        Some(h)
            if !h.is_empty()
                && h.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) =>
        {
            i64::from_str_radix(h, 16).ok()
        }
        Some(_) => None,
        None => (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .then(|| s.parse().ok())
            .flatten(),
    }
}

fn reg_n(s: &str) -> bool {
    s.strip_prefix('x')
        .is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
}

/// `[(x\d+|sp)(, #(0x[0-9a-f]+|\d+))?]`
fn mem_x_sp(s: &str) -> bool {
    let Some(inner) = s.strip_prefix('[').and_then(|r| r.strip_suffix(']')) else {
        return false;
    };
    let (base, off) = match inner.split_once(", #") {
        Some((b, o)) => (b, Some(o)),
        None => (inner, None),
    };
    (base == "sp" || reg_n(base)) && off.is_none_or(|o| lit(o).is_some())
}

/// `x18, (x\d+, )?\[(x\d+|sp)(, #(0x[0-9a-f]+|\d+))?\]`, for `ldr` and `ldp` only.
fn x18_pointer_load(p: &Insn) -> bool {
    if !matches!(p.mn, "ldr" | "ldp") {
        return false;
    }
    let s = p.op_str();
    let Some(rest) = s.strip_prefix("x18, ") else {
        return false;
    };
    match rest.split_once(", ") {
        Some((r, m)) if reg_n(r) && m.starts_with('[') => mem_x_sp(m),
        _ => mem_x_sp(rest),
    }
}

fn names(p: &Insn) -> Vec<String> {
    p.writes.iter().map(|r| r.name()).collect()
}

fn writes_x18(p: &Insn) -> bool {
    names(p).iter().any(|n| n == "x18" || n == "w18")
}

fn is_cond(p: &Insn) -> bool {
    matches!(p.mn, "cbz" | "cbnz" | "tbz" | "tbnz") || p.mn.starts_with("b.")
}

fn tgt(p: &Insn) -> i64 {
    p.target().unwrap_or(0) as i64
}

fn contiguous(insl: &[Insn]) -> bool {
    insl.windows(2).all(|p| p[1].addr == p[0].addr + 4)
}

/// `sp, sp, #N`: N.
fn sp_adjust(p: &Insn, mn: &str) -> Option<i64> {
    if p.mn != mn {
        return None;
    }
    lit(p.op_str().strip_prefix("sp, sp, #")?)
}

fn tail_call_padded(insl: &[Insn], j: usize) -> bool {
    if j + 1 >= insl.len()
        || insl[j + 1..]
            .iter()
            .any(|p| p.mn != "udf" || p.op_str() != "#0")
    {
        return false;
    }
    if !contiguous(insl) {
        return false;
    }
    let bad = ["sp", "wsp", "fp", "lr", "x29", "w29", "x30", "w30"];
    for p in &insl[..j] {
        if !matches!(p.mn, "mov" | "ldr" | "ldrh" | "ldp" | "add" | "str" | "stp") {
            return false;
        }
        if names(p).iter().any(|n| bad.contains(&n.as_str())) {
            return false;
        }
    }
    for p in insl[..j].iter().rev() {
        if !writes_x18(p) {
            continue;
        }
        return x18_pointer_load(p);
    }
    false
}

impl An<'_> {
    /// After terminal or padded unframed tails, a `br x18` with other code after it is a tail call only when the
    /// frame is provably gone.
    #[allow(
        clippy::too_many_lines,
        reason = "one function of the script, kept in its order"
    )]
    pub(crate) fn tail_call_framed(insl: &[Insn], j: usize) -> bool {
        if j < 4 || !contiguous(insl) {
            return false;
        }
        let Some(load) = (0..j).rev().find(|&k| writes_x18(&insl[k])) else {
            return false;
        };
        if !x18_pointer_load(&insl[load]) {
            return false;
        }
        let thunk = insl[0].mn == "stp" && insl[0].op_str() == "x29, x30, [sp, #-0x10]!";
        let (restore, start, disp, mut saved, mut fp_set);
        if thunk {
            if insl[1].mn != "mov" || insl[1].op_str() != "x29, sp" {
                return false;
            }
            if insl[j - 1].mn != "ldp" || insl[j - 1].op_str() != "x29, x30, [sp], #0x10" {
                return false;
            }
            (restore, start, disp, saved, fp_set) = (j - 1, 2usize, 0i64, true, true);
        } else {
            let (Some(sub), Some(add)) =
                (sp_adjust(&insl[0], "sub"), sp_adjust(&insl[j - 1], "add"))
            else {
                return false;
            };
            if sub != add {
                return false;
            }
            let Some(r) = (1..=j - 2).rev().find(|&k| {
                let s = insl[k].op_str();
                insl[k].mn == "ldp"
                    && s.strip_prefix("x29, x30, [sp")
                        .and_then(|r| r.strip_suffix(']'))
                        .is_some_and(|r| {
                            r.is_empty() || r.strip_prefix(", #").is_some_and(|o| lit(o).is_some())
                        })
            }) else {
                return false;
            };
            let Some(Op::Mem { disp: d, .. }) = insl[r].ops.last() else {
                return false;
            };
            if *d < 0 || *d + 16 > sub {
                return false;
            }
            (restore, start, disp, saved, fp_set) = (r, 1usize, *d, false, false);
        }
        let tail = load.min(restore);
        let after_br = insl[j].addr as i64;
        let mut setup_end = insl[start].addr as i64;
        let mut targets: Vec<i64> = Vec::new();
        let mut guarded = false;
        for k in start..j - 1 {
            let p = &insl[k];
            if k == restore {
                continue;
            }
            if is_cond(p) || p.mn == "b" {
                let t = tgt(p);
                if !saved || k >= restore || (p.mn == "b" && k >= tail) {
                    return false;
                }
                if t > after_br {
                    guarded = true;
                } else if thunk {
                    return false;
                }
                targets.push(t);
                continue;
            }
            if matches!(p.mn, "bl" | "blr") {
                if thunk || !saved || k >= tail {
                    return false;
                }
                continue;
            }
            if matches!(
                p.mn,
                "br" | "ret"
                    | "udf"
                    | "brk"
                    | "hlt"
                    | "svc"
                    | "hvc"
                    | "smc"
                    | "msr"
                    | "sys"
                    | "dc"
                    | "ic"
            ) {
                return false;
            }
            let w = names(p);
            let has = |set: &[&str]| w.iter().any(|n| set.contains(&n.as_str()));
            if has(&["sp", "wsp", "lr", "x30", "w30"]) {
                return false;
            }
            if has(&["fp", "x29", "w29"]) {
                let sets_fp = if p.mn == "mov" {
                    p.op_str() == "x29, sp" && disp == 0
                } else {
                    p.mn == "add" && p.op_str().starts_with("x29, sp, #") && p.imm(2) == Some(disp)
                };
                if k > restore || fp_set || !saved || !sets_fp {
                    return false;
                }
                fp_set = true;
                setup_end = insl[k + 1].addr as i64;
                continue;
            }
            if !saved && p.mn == "stp" && p.op_str() == insl[restore].op_str() {
                saved = true;
                setup_end = insl[k + 1].addr as i64;
                continue;
            }
            let s = p.op_str();
            if !s.contains('[')
                || ["ldr", "ldur", "ldp", "ldar", "prfm"]
                    .iter()
                    .any(|m| p.mn.starts_with(m))
            {
                continue;
            }
            if k > restore
                || !matches!(
                    p.mn,
                    "str" | "strb" | "strh" | "stur" | "sturb" | "sturh" | "stp"
                )
                || p.wb
            {
                return false;
            }
            let Some(Op::Mem {
                base,
                index,
                disp: md,
                ..
            }) = p.ops.last()
            else {
                return false;
            };
            let bname = base.name();
            if !matches!(bname.as_str(), "sp" | "fp" | "x29") {
                if thunk {
                    return false;
                }
                continue;
            }
            if index.is_some() || (bname != "sp" && !fp_set) {
                return false;
            }
            let at = md + if bname != "sp" { disp } else { 0 };
            let mut size: i64 = match p.mn.chars().last() {
                Some('b') => 1,
                Some('h') => 2,
                _ => match p.reg(0).and_then(|r| r.name().chars().next()) {
                    Some('x') | Some('d') => 8,
                    Some('w') | Some('s') => 4,
                    Some('q') => 16,
                    Some('h') => 2,
                    Some('b') => 1,
                    _ => 8,
                },
            };
            if p.mn == "stp" {
                size *= 2;
            }
            if at < 0 || (at < disp + 16 && at + size > disp) {
                return false;
            }
        }
        if !saved {
            return false;
        }
        let suffix = &insl[j + 1..];
        let mut leaves = false;
        for p in suffix {
            if p.mn == "bl" {
                if insl[0].addr as i64 <= tgt(p) && tgt(p) <= insl[insl.len() - 1].addr as i64 {
                    return false;
                }
                leaves = true;
            } else if (is_cond(p) || p.mn == "b") && tgt(p) <= after_br {
                return false;
            }
        }
        if thunk
            && (!guarded
                || !leaves
                || !suffix
                    .last()
                    .is_some_and(|p| matches!(p.mn, "bl" | "udf" | "b"))
                || suffix.iter().any(|p| matches!(p.mn, "ret" | "br" | "blr")))
        {
            return false;
        }
        !targets
            .iter()
            .any(|t| *t < setup_end || (insl[restore].addr as i64) < *t && *t <= after_br)
    }

    /// `tail_call(insl, j)` of the script.
    pub(crate) fn tail_call(insl: &[Insn], j: usize) -> bool {
        if j + 1 == insl.len() {
            return Self::tail_call_at_end(insl, j);
        }
        if tail_call_padded(insl, j) {
            return true;
        }
        Self::tail_call_framed(insl, j)
    }

    /// `cmp wS,#N; b.hi exit; and xI,xS,#0xffffffff; ldrsw xE,[xB,xI,lsl #2]; add x18,xE,xB; br x18`: a 32-bit
    /// checked value zero-extended into the index, with its full path to the `br` verified.
    #[allow(
        clippy::too_many_lines,
        reason = "one function of the script, kept in its order"
    )]
    pub(crate) fn jump_zero_extended(&self, insl: &[Insn], j: usize) -> R<Option<Vec<i64>>> {
        let lo0 = j.saturating_sub(16);
        let ctx = &insl[lo0..=j];
        let cl = ctx.len();
        let ops_of =
            |p: &Insn| -> Vec<String> { p.op_str().split(", ").map(str::to_string).collect() };
        let Some(ka) = ctx[..cl - 1].iter().rposition(|p| {
            let o = ops_of(p);
            p.mn == "add" && o.len() == 3 && o[0] == "x18" && reg_n(&o[1]) && reg_n(&o[2])
        }) else {
            return Ok(None);
        };
        let ao = ops_of(&ctx[ka]);
        let (ent, base) = (ao[1].clone(), ao[2].clone());
        let Some(kl) = ctx[..ka].iter().rposition(|p| {
            let o = ops_of(p);
            p.mn == "ldrsw"
                && o.len() == 4
                && o[0] == ent
                && o[1] == format!("[{base}")
                && reg_n(&o[2])
                && o[3] == "lsl #2]"
        }) else {
            return Ok(None);
        };
        let idx = ops_of(&ctx[kl])[2].clone();
        if base == ent || base == idx {
            return Ok(None);
        }
        let Some(km) = ctx[..kl].iter().rposition(|p| {
            let o = ops_of(p);
            p.mn == "and" && o.len() == 3 && o[0] == idx && reg_n(&o[1]) && o[2] == "#0xffffffff"
        }) else {
            return Ok(None);
        };
        let src = ops_of(&ctx[km])[1].clone();
        let wsrc = format!("w{}", &src[1..]);
        let Some(kc) = ctx[..km].iter().rposition(|p| {
            let o = ops_of(p);
            p.mn == "cmp"
                && o.len() == 2
                && o[0] == wsrc
                && o[1].strip_prefix('#').is_some_and(|v| lit(v).is_some())
        }) else {
            return Ok(None);
        };
        let check = &ctx[kc];
        let Some(branch) = ctx.get(kc + 1) else {
            return Ok(None);
        };
        if branch.mn != "b.hi" {
            return Ok(None);
        }
        let run = &ctx[kc..];
        let (head, end) = (check.addr as i64, insl[j].addr as i64);
        if !contiguous(run) {
            return Ok(None);
        }
        let bt = branch.imm(0).unwrap_or(0);
        if head <= bt && bt <= end {
            return Ok(None);
        }
        if ctx[kc + 2..cl - 1].iter().any(|p| {
            matches!(
                p.mn,
                "b" | "bl"
                    | "br"
                    | "blr"
                    | "ret"
                    | "cbz"
                    | "cbnz"
                    | "tbz"
                    | "tbnz"
                    | "svc"
                    | "hvc"
                    | "smc"
                    | "brk"
                    | "hlt"
                    | "eret"
                    | "drps"
            ) || p.mn.starts_with("b.")
        }) {
            return Ok(None);
        }
        let w = |p: &Insn, r: &str| writes_reg(p, r);
        if ctx
            .get(kc + 2..km)
            .unwrap_or(&[])
            .iter()
            .any(|p| w(p, &src))
            || ctx[km + 1..kl].iter().any(|p| w(p, &idx))
            || ctx[kl + 1..ka].iter().any(|p| w(p, &ent) || w(p, &base))
            || ctx[ka + 1..cl - 1].iter().any(|p| w(p, "x18"))
        {
            return Ok(None);
        }
        for p in insl {
            if is_direct_branch(p.mn) {
                let t = tgt(p);
                if head < t && t <= end {
                    return Ok(None);
                }
            }
        }
        if self
            .tabjumps
            .values()
            .flatten()
            .any(|t| head < *t && *t <= end)
        {
            return Ok(None);
        }
        if self
            .jumps
            .values()
            .any(|(b, wd)| *b <= end && *b + 4 * *wd as i64 > head + 4)
        {
            return Ok(None);
        }
        let n = check
            .op_str()
            .split('#')
            .nth(1)
            .and_then(lit)
            .map_or(0, |v| v as usize)
            + 1;
        let Some(tb) = self.copied_base(insl, lo0 + kl, &base) else {
            return Ok(None);
        };
        if tb < 0 || tb + 4 * n as i64 > self.img.len() as i64 {
            return Ok(None);
        }
        let targets = self.table_entries(tb, n)?;
        let addrs: HashSet<i64> = insl.iter().map(|p| p.addr as i64).collect();
        Ok(targets
            .iter()
            .all(|t| addrs.contains(t) && !(head < *t && *t <= end))
            .then_some(targets))
    }
}
