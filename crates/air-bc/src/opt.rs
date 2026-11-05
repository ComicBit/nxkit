//! A light optimiser on the module tree, so raw libraries stay small: promotion of scalar and vector allocas to SSA
//! values (mem2reg, with phi placement on the dominance frontier) and dead instruction elimination. Semantics are
//! unchanged, the GPU compiler optimises again.

use crate::ast::*;
use std::collections::{HashMap, HashSet};

fn vals_mut(op: &mut Op) -> Vec<&mut Val> {
    match op {
        Op::Alloca(..) | Op::Br(_) | Op::Unreachable => vec![],
        Op::Load(_, p, _) => vec![&mut p.1],
        Op::Store(a, b, _) => vec![&mut a.1, &mut b.1],
        Op::Gep(_, _, p, idx) => std::iter::once(&mut p.1)
            .chain(idx.iter_mut().map(|x| &mut x.1))
            .collect(),
        Op::Cast(_, v, _) => vec![&mut v.1],
        Op::Bin(_, _, _, a, b) | Op::Cmp(_, _, _, _, a, b) => vec![a, b],
        Op::FNeg(_, _, a) => vec![a],
        Op::Select(_, a, b, c) | Op::InsertElt(a, b, c) | Op::Shuffle(a, b, c) => {
            vec![&mut a.1, &mut b.1, &mut c.1]
        }
        Op::Phi(_, _, ins) => ins.iter_mut().map(|x| &mut x.0).collect(),
        Op::CondBr(c, ..) => vec![c],
        Op::Switch(_, v, _, cases) => std::iter::once(v)
            .chain(cases.iter_mut().map(|x| &mut x.0))
            .collect(),
        Op::Ret(v) => v.iter_mut().map(|x| &mut x.1).collect(),
        Op::Call(_, _, _, _, args, _) => args.iter_mut().map(|x| &mut x.2).collect(),
        Op::ExtractElt(a, b) => vec![&mut a.1, &mut b.1],
        Op::ExtractVal(a, _) => vec![&mut a.1],
        Op::InsertVal(a, b, _) => vec![&mut a.1, &mut b.1],
    }
}

fn locals(op: &Op, out: &mut Vec<String>) {
    let mut op = op.clone();
    for v in vals_mut(&mut op) {
        collect(v, out);
    }
}

fn collect(v: &Val, out: &mut Vec<String>) {
    match v {
        Val::Local(n) => out.push(n.clone()),
        Val::Aggregate(items) => items.iter().for_each(|(_, x)| collect(x, out)),
        Val::Cast(_, inner, _) => collect(&inner.1, out),
        _ => {}
    }
}

fn successors(insts: &[Inst]) -> Vec<String> {
    match insts.last().map(|i| &i.op) {
        Some(Op::Br(l)) => vec![l.clone()],
        Some(Op::CondBr(_, a, b)) => vec![a.clone(), b.clone()],
        Some(Op::Switch(_, _, d, cases)) => std::iter::once(d.clone())
            .chain(cases.iter().map(|c| c.1.clone()))
            .collect(),
        _ => vec![],
    }
}

fn promotable(t: &Ty) -> bool {
    matches!(
        t,
        Ty::Int(_) | Ty::Half | Ty::Float | Ty::Double | Ty::Vector(..) | Ty::Ptr(..)
    )
}

fn is_lifetime(op: &Op) -> bool {
    matches!(op, Op::Call(_, _, _, c, _, _) if c.starts_with("llvm.lifetime."))
}

/// mem2reg and dead code elimination on every defined function.
pub fn optimize(m: &mut Module) {
    for f in &mut m.functions {
        if !f.blocks.is_empty() {
            mem2reg(f);
            trivial_phis(f);
            cse(f);
            dce(f);
        }
    }
}

fn mem2reg(f: &mut Function) {
    // candidates: allocas of first-class types used only by whole loads and stores (as the address), plus bitcasts
    // whose only uses are lifetime markers
    let mut allocas: HashMap<String, Ty> = HashMap::new();
    for (_, insts) in &f.blocks {
        for i in insts {
            if let (Some(r), Op::Alloca(t, _)) = (&i.res, &i.op) {
                if promotable(t) {
                    allocas.insert(r.clone(), t.clone());
                }
            }
        }
    }
    if allocas.is_empty() {
        return;
    }
    let mut marker_casts: HashMap<String, String> = HashMap::new();
    for (_, insts) in &f.blocks {
        for i in insts {
            if let (Some(r), Op::Cast(k, (_, Val::Local(src)), _)) = (&i.res, &i.op) {
                if k == "bitcast" && allocas.contains_key(src) {
                    marker_casts.insert(r.clone(), src.clone());
                }
            }
        }
    }
    let mut bad: HashSet<String> = HashSet::new();
    for (_, insts) in &f.blocks {
        for i in insts {
            match &i.op {
                Op::Load(t, (_, Val::Local(p)), _) if allocas.contains_key(p) => {
                    if &allocas[p] != t {
                        bad.insert(p.clone());
                    }
                }
                Op::Store((t, v), (_, Val::Local(p)), _) if allocas.contains_key(p) => {
                    if &allocas[p] != t {
                        bad.insert(p.clone());
                    }
                    let mut u = Vec::new();
                    collect(v, &mut u);
                    bad.extend(
                        u.into_iter()
                            .filter(|x| allocas.contains_key(x) || marker_casts.contains_key(x))
                            .map(|x| marker_casts.get(&x).cloned().unwrap_or(x)),
                    );
                }
                Op::Cast(k, (_, Val::Local(src)), _)
                    if k == "bitcast" && allocas.contains_key(src) => {}
                op if is_lifetime(op) => {}
                op => {
                    let mut u = Vec::new();
                    locals(op, &mut u);
                    for x in u {
                        if allocas.contains_key(&x) {
                            bad.insert(x);
                        } else if let Some(a) = marker_casts.get(&x) {
                            bad.insert(a.clone());
                        }
                    }
                }
            }
        }
    }
    // a cast of a candidate used by anything but lifetime markers disqualifies it (checked above via `op` arm)
    allocas.retain(|k, _| !bad.contains(k));
    if allocas.is_empty() {
        return;
    }
    // CFG, reverse post order, dominators (Cooper, Harvey, Kennedy)
    let n = f.blocks.len();
    let idx: HashMap<String, usize> = f
        .blocks
        .iter()
        .enumerate()
        .map(|(k, b)| (b.0.clone(), k))
        .collect();
    let succ: Vec<Vec<usize>> = f
        .blocks
        .iter()
        .map(|b| {
            successors(&b.1)
                .iter()
                .filter_map(|l| idx.get(l).copied())
                .collect()
        })
        .collect();
    let mut pred: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (b, ss) in succ.iter().enumerate() {
        for &s in ss {
            if !pred[s].contains(&b) {
                pred[s].push(b);
            }
        }
    }
    let mut order = Vec::new();
    let mut seen = vec![false; n];
    let mut stack = vec![(0usize, 0usize)];
    seen[0] = true;
    while let Some((b, k)) = stack.pop() {
        if k < succ[b].len() {
            stack.push((b, k + 1));
            let s = succ[b][k];
            if !seen[s] {
                seen[s] = true;
                stack.push((s, 0));
            }
        } else {
            order.push(b);
        }
    }
    order.reverse();
    let mut rpo = vec![usize::MAX; n];
    for (k, &b) in order.iter().enumerate() {
        rpo[b] = k;
    }
    let mut idom = vec![usize::MAX; n];
    idom[0] = 0;
    let mut changed = true;
    while changed {
        changed = false;
        for &b in order.iter().skip(1) {
            let mut new = usize::MAX;
            for &p in &pred[b] {
                if idom[p] == usize::MAX {
                    continue;
                }
                new = if new == usize::MAX {
                    p
                } else {
                    let (mut x, mut y) = (p, new);
                    while x != y {
                        while rpo[x] > rpo[y] {
                            x = idom[x];
                        }
                        while rpo[y] > rpo[x] {
                            y = idom[y];
                        }
                    }
                    x
                };
            }
            if new != usize::MAX && idom[b] != new {
                idom[b] = new;
                changed = true;
            }
        }
    }
    // dominance frontiers
    let mut df: Vec<HashSet<usize>> = vec![HashSet::new(); n];
    for b in 0..n {
        if pred[b].len() >= 2 && idom[b] != usize::MAX {
            for &p in &pred[b] {
                let mut r = p;
                while r != idom[b] && idom[r] != usize::MAX {
                    df[r].insert(b);
                    if r == idom[r] {
                        break;
                    }
                    r = idom[r];
                }
            }
        }
    }
    // phi placement
    let names: Vec<String> = {
        let mut v: Vec<String> = allocas.keys().cloned().collect();
        v.sort();
        v
    };
    let mut phis: Vec<Vec<(String, String)>> = vec![Vec::new(); n]; // block -> (alloca, phi name)
    let mut counter = 0usize;
    for a in &names {
        let mut work: Vec<usize> = f
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| {
                b.1.iter()
                    .any(|i| matches!(&i.op, Op::Store(_, (_, Val::Local(p)), _) if p == a))
            })
            .map(|(k, _)| k)
            .collect();
        let mut has: HashSet<usize> = HashSet::new();
        while let Some(b) = work.pop() {
            for &d in &df[b] {
                if has.insert(d) {
                    counter += 1;
                    phis[d].push((a.clone(), format!("m2r.{counter}")));
                    work.push(d);
                }
            }
        }
    }
    // renaming over the dominator tree
    let mut kids: Vec<Vec<usize>> = vec![Vec::new(); n];
    for b in 1..n {
        if idom[b] != usize::MAX && idom[b] != b {
            kids[idom[b]].push(b);
        }
    }
    let mut subst: HashMap<String, Val> = HashMap::new();
    let mut phi_in: Vec<Vec<Vec<(Val, String)>>> =
        phis.iter().map(|p| vec![Vec::new(); p.len()]).collect();
    let mut remove: HashSet<(usize, usize)> = HashSet::new();
    let mut stack: Vec<(usize, HashMap<String, Val>)> =
        vec![(0, names.iter().map(|a| (a.clone(), Val::Undef)).collect())];
    while let Some((b, mut cur)) = stack.pop() {
        for (a, p) in &phis[b] {
            cur.insert(a.clone(), Val::Local(p.clone()));
        }
        for (k, i) in f.blocks[b].1.iter().enumerate() {
            match &i.op {
                Op::Alloca(..) if i.res.as_ref().is_some_and(|r| allocas.contains_key(r)) => {
                    remove.insert((b, k));
                }
                Op::Load(_, (_, Val::Local(p)), _) if allocas.contains_key(p) => {
                    let v = cur[p].clone();
                    if let Some(r) = &i.res {
                        subst.insert(r.clone(), v);
                    }
                    remove.insert((b, k));
                }
                Op::Store((_, v), (_, Val::Local(p)), _) if allocas.contains_key(p) => {
                    cur.insert(p.clone(), v.clone());
                    remove.insert((b, k));
                }
                Op::Cast(_, (_, Val::Local(src)), _) if allocas.contains_key(src) => {
                    remove.insert((b, k));
                }
                op if is_lifetime(op) => {
                    let mut u = Vec::new();
                    locals(op, &mut u);
                    if u.iter()
                        .any(|x| marker_casts.get(x).is_some_and(|a| allocas.contains_key(a)))
                    {
                        remove.insert((b, k));
                    }
                }
                _ => {}
            }
        }
        let label = f.blocks[b].0.clone();
        for &s in &succ[b] {
            for (j, (a, _)) in phis[s].iter().enumerate() {
                phi_in[s][j].push((cur[a].clone(), label.clone()));
            }
        }
        for &c in &kids[b] {
            stack.push((c, cur.clone()));
        }
    }
    // edges from unreachable predecessors carry undef
    for s in 0..n {
        for &p in &pred[s] {
            if !seen[p] {
                for (j, _) in phis[s].iter().enumerate() {
                    for _ in succ[p].iter().filter(|&&x| x == s) {
                        phi_in[s][j].push((Val::Undef, f.blocks[p].0.clone()));
                    }
                }
            }
        }
    }
    // rewrite: drop promoted instructions, insert phis, substitute loaded values (chains resolved)
    let resolve = |v: &Val, subst: &HashMap<String, Val>| -> Val {
        let mut v = v.clone();
        let mut guard = 0;
        while let Val::Local(n) = &v {
            match subst.get(n) {
                Some(x) if guard < 10_000 => {
                    v = x.clone();
                    guard += 1;
                }
                _ => break,
            }
        }
        v
    };
    for (b, (_, insts)) in f.blocks.iter_mut().enumerate() {
        let mut out: Vec<Inst> = Vec::with_capacity(insts.len());
        for (j, (a, p)) in phis[b].iter().enumerate() {
            if !seen[b] {
                continue;
            }
            let ty = allocas[a].clone();
            out.push(Inst {
                res: Some(p.clone()),
                op: Op::Phi(Vec::new(), ty, phi_in[b][j].clone()),
                md: Vec::new(),
            });
        }
        for (k, i) in insts.drain(..).enumerate() {
            if !remove.contains(&(b, k)) {
                out.push(i);
            }
        }
        *insts = out;
    }
    for (_, insts) in &mut f.blocks {
        for i in insts.iter_mut() {
            for v in vals_mut(&mut i.op) {
                *v = subst_deep(v, &subst, &resolve);
            }
        }
    }
}

fn subst_deep(
    v: &Val,
    subst: &HashMap<String, Val>,
    resolve: &dyn Fn(&Val, &HashMap<String, Val>) -> Val,
) -> Val {
    match v {
        Val::Local(_) => resolve(v, subst),
        Val::Aggregate(items) => Val::Aggregate(
            items
                .iter()
                .map(|(t, x)| (t.clone(), subst_deep(x, subst, resolve)))
                .collect(),
        ),
        Val::Cast(k, inner, to) => Val::Cast(
            k.clone(),
            Box::new((inner.0.clone(), subst_deep(&inner.1, subst, resolve))),
            to.clone(),
        ),
        _ => v.clone(),
    }
}

fn side_effect(op: &Op) -> bool {
    match op {
        Op::Store(..)
        | Op::Br(_)
        | Op::CondBr(..)
        | Op::Switch(..)
        | Op::Ret(_)
        | Op::Unreachable => true,
        // calls: keep unless pure intrinsic math, air.* reads and samples have no side effect either, but keep them
        // (convergent, and some write), llvm.lifetime of dead allocas goes with the alloca
        Op::Call(_, _, _, c, _, _) => {
            !(c.starts_with("air.convert")
                || c.starts_with("llvm.fmuladd")
                || c.starts_with("air.fast_")
                || c.starts_with("air.fma"))
        }
        _ => false,
    }
}

fn dce(f: &mut Function) {
    loop {
        let mut used: HashSet<String> = HashSet::new();
        for (_, insts) in &f.blocks {
            for i in insts {
                let mut u = Vec::new();
                locals(&i.op, &mut u);
                used.extend(u);
            }
        }
        let mut removed = false;
        for (_, insts) in &mut f.blocks {
            let before = insts.len();
            insts.retain(|i| match &i.res {
                Some(r) => side_effect(&i.op) || used.contains(r),
                None => true,
            });
            removed |= insts.len() != before;
        }
        if !removed {
            break;
        }
    }
}

fn substitute(f: &mut Function, subst: &HashMap<String, Val>) {
    if subst.is_empty() {
        return;
    }
    let resolve = |v: &Val, subst: &HashMap<String, Val>| -> Val {
        let mut v = v.clone();
        let mut guard = 0;
        while let Val::Local(n) = &v {
            match subst.get(n) {
                Some(x) if guard < 10_000 => {
                    v = x.clone();
                    guard += 1;
                }
                _ => break,
            }
        }
        v
    };
    for (_, insts) in &mut f.blocks {
        for i in insts.iter_mut() {
            for v in vals_mut(&mut i.op) {
                *v = subst_deep(v, subst, &resolve);
            }
        }
    }
}

/// Phis whose incoming values are all the same value (or the phi itself) become that value.
fn trivial_phis(f: &mut Function) {
    loop {
        let mut subst: HashMap<String, Val> = HashMap::new();
        for (_, insts) in &f.blocks {
            for i in insts {
                if let (Some(r), Op::Phi(_, _, ins)) = (&i.res, &i.op) {
                    let mut uniq: Option<&Val> = None;
                    let mut ok = true;
                    for (v, _) in ins {
                        if matches!(v, Val::Local(n) if n == r) {
                            continue;
                        }
                        match uniq {
                            None => uniq = Some(v),
                            Some(u) if u == v => {}
                            _ => {
                                ok = false;
                                break;
                            }
                        }
                    }
                    if ok {
                        subst.insert(r.clone(), uniq.cloned().unwrap_or(Val::Undef));
                    }
                }
            }
        }
        if subst.is_empty() {
            return;
        }
        for (_, insts) in &mut f.blocks {
            insts.retain(|i| !i.res.as_ref().is_some_and(|r| subst.contains_key(r)));
        }
        substitute(f, &subst);
    }
}

/// Block-local common subexpressions: pure operations and loads from the constant address space.
fn cse(f: &mut Function) {
    let mut subst: HashMap<String, Val> = HashMap::new();
    for (_, insts) in &mut f.blocks {
        let mut seen: HashMap<String, String> = HashMap::new();
        let mut drop: HashSet<usize> = HashSet::new();
        for (k, i) in insts.iter().enumerate() {
            let Some(r) = &i.res else { continue };
            let pure = match &i.op {
                Op::Gep(..)
                | Op::Cast(..)
                | Op::Bin(..)
                | Op::Cmp(..)
                | Op::Select(..)
                | Op::ExtractElt(..)
                | Op::InsertElt(..)
                | Op::Shuffle(..)
                | Op::ExtractVal(..)
                | Op::InsertVal(..)
                | Op::FNeg(..) => true,
                Op::Load(_, (Ty::Ptr(_, 2), _), _) => true,
                _ => false,
            };
            if !pure {
                continue;
            }
            // operands already rewritten by earlier substitutions of this block
            let mut op = i.op.clone();
            for v in vals_mut(&mut op) {
                if let Val::Local(n) = v {
                    if let Some(x) = subst.get(n) {
                        *v = x.clone();
                    }
                }
            }
            let key = format!("{op:?}|{:?}", i.md);
            match seen.get(&key) {
                Some(prev) => {
                    subst.insert(r.clone(), Val::Local(prev.clone()));
                    drop.insert(k);
                }
                None => {
                    seen.insert(key, r.clone());
                }
            }
        }
        let mut k = 0;
        insts.retain(|_| {
            k += 1;
            !drop.contains(&(k - 1))
        });
    }
    substitute(f, &subst);
}
