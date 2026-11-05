//! Spin waits, part one and a half: the search for the region of a loop head.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::rw::{R, Rw};
use crate::spin::{Found, NK, Node, Region, SR, spin_kind, spin_rw};

const SPIN_PATHS: i64 = 256;
const SPIN_STEPS: i64 = 4096;
const SPIN_STORES: usize = 4;

impl Rw {
    /// The spin wait region with head `t` (bes: the backward branches found by the scan).
    pub(crate) fn spin_region(
        &self,
        t: i64,
        bes: &[i64],
        blocks: &[(i64, i64)],
        touched: &HashSet<i64>,
    ) -> R<Option<Region>> {
        let mut ban: HashSet<i64> = HashSet::new();
        let mut g = Found::No;
        for _ in 0..8 {
            g = self.spin_region1(t, bes, &ban, true, blocks, touched)?;
            match &g {
                Found::Vary(v) => ban.extend(v.iter().copied()),
                _ => break,
            }
        }
        let old = self.spin_region1(t, bes, &HashSet::new(), false, blocks, touched)?;
        Ok(match (g, old) {
            (Found::Yes(g), Found::Yes(o)) => {
                if o.valid.is_subset(&g.valid) {
                    Some(g)
                } else {
                    Some(o)
                }
            }
            (Found::Yes(g), _) => Some(g),
            (_, Found::Yes(o)) => Some(o),
            _ => None,
        })
    }

    fn spin_region1(
        &self,
        t: i64,
        bes: &[i64],
        ban: &HashSet<i64>,
        stores: bool,
        blocks: &[(i64, i64)],
        touched: &HashSet<i64>,
    ) -> R<Found> {
        let hi = bes.iter().copied().max().unwrap_or(t);
        if blocks.iter().any(|(s, e)| *s < hi + 4 && t < *e) || !self.spin_quick(t, hi) {
            return Ok(Found::No);
        }
        let mut nodes: HashMap<i64, Node> = HashMap::new();
        let mut back: BTreeSet<i64> = BTreeSet::new();
        let mut bad: HashSet<i64> = HashSet::new();
        let mut inner: HashSet<i64> = HashSet::new();
        let mut todo = vec![t];
        while let Some(a) = todo.pop() {
            if nodes.contains_key(&a) || bad.contains(&a) {
                continue;
            }
            if ban.contains(&a) {
                bad.insert(a);
                continue;
            }
            let rmw = if a + 12 <= hi && self.w32(a) & 0x3fe0_0000 == 0x0840_0000 {
                self.spin_rmw(a)
            } else {
                None
            };
            let lk = if rmw.is_some() {
                None
            } else {
                self.spin_lock(a)
            };
            let lk = lk.filter(|l| !(l.0 == NK::Tst && a == t));
            let n: Node;
            if let Some((base, k, defs, ins)) = rmw {
                n = Node {
                    kind: NK::Rmw,
                    base,
                    k,
                    ins,
                    pre: 0,
                    uses: Some(HashSet::from([base])),
                    defs: Some(defs),
                    succ: vec![a + 16],
                };
            } else if let Some((kind, base, ins, pre, cnt, uses, defs)) = lk {
                let k = match kind {
                    NK::Acq => 1,
                    NK::Rel => -1,
                    _ => 0,
                };
                n = Node {
                    kind,
                    base,
                    k,
                    ins,
                    pre,
                    uses: Some(uses),
                    defs: Some(defs),
                    succ: vec![a + 4 * cnt as i64],
                };
            } else if stores && Self::spin_store(self.spin_ins(a).as_ref()).is_some() {
                let x = self.spin_ins(a);
                let Some(x) = x else { continue };
                let st = Self::spin_store(Some(&x));
                let Some((v, b)) = st else { continue };
                let mut u: HashSet<SR> = HashSet::from([b]);
                u.extend(v);
                n = Node {
                    kind: NK::Stc,
                    base: b,
                    k: 0,
                    ins: vec![x],
                    pre: 0,
                    uses: Some(u),
                    defs: Some(HashSet::new()),
                    succ: vec![a + 4],
                };
            } else {
                let x = self.spin_ins(a);
                let Some(k) = spin_kind(x.as_ref()) else {
                    bad.insert(a);
                    continue;
                };
                let Some(x) = x else { continue };
                let succ = if k == NK::Br {
                    let tg = x.target().unwrap_or(0) as i64;
                    if x.mn == "b" {
                        vec![tg]
                    } else {
                        vec![tg, a + 4]
                    }
                } else {
                    vec![a + 4]
                };
                n = Node {
                    kind: k,
                    base: 0,
                    k: 0,
                    ins: vec![x],
                    pre: 0,
                    uses: None,
                    defs: None,
                    succ,
                };
            }
            if (0..n.ins.len() as i64).any(|i| touched.contains(&(a + 4 * (i - n.pre as i64)))) {
                bad.insert(a);
                continue;
            }
            if n.kind == NK::Rmw {
                inner.extend([a + 4, a + 8, a + 12]);
            }
            if matches!(n.kind, NK::Acq | NK::Rel | NK::Tst) {
                for i in 0..n.ins.len() as i64 {
                    let o = a + 4 * (i - n.pre as i64);
                    if o != a {
                        inner.insert(o);
                    }
                }
            }
            for s in n.succ.clone() {
                if s == t {
                    back.insert(a);
                } else if t < s && s <= hi {
                    todo.push(s);
                }
            }
            nodes.insert(a, n);
        }
        if back.is_empty()
            || inner.iter().any(|a| nodes.contains_key(a))
            || nodes
                .values()
                .any(|n| n.succ.iter().any(|s| inner.contains(s)))
        {
            return Ok(Found::No);
        }
        let prune = |nodes: &HashMap<i64, Node>, ends: &BTreeSet<i64>| -> HashSet<i64> {
            let mut keep: HashSet<i64> = ends.iter().copied().collect();
            let mut more = true;
            while more {
                more = false;
                for (a, n) in nodes {
                    if !keep.contains(a) && n.succ.iter().any(|s| keep.contains(s) && *s != t) {
                        keep.insert(*a);
                        more = true;
                    }
                }
            }
            keep
        };
        let rset = prune(&nodes, &back);
        for a in &rset {
            if let Some(n) = nodes.get_mut(a)
                && n.uses.is_none()
            {
                let (u, d) = spin_rw(&n.ins[0]);
                n.uses = Some(u);
                n.defs = Some(d);
            }
        }
        // every path from T to a back-edge: no inner cycle, and the exclusive adds on each address sum to 0
        let mut bad_back: BTreeSet<i64> = BTreeSet::new();
        let mut cnt = [0i64, 0i64];
        let mut onpath: HashSet<i64> = HashSet::from([t]);
        let ok = Self::walk(
            &nodes,
            &rset,
            t,
            t,
            &mut onpath,
            &HashMap::new(),
            &HashMap::new(),
            &mut cnt,
            &mut bad_back,
        );
        if !ok {
            return Ok(Found::No);
        }
        let valid: BTreeSet<i64> = back.difference(&bad_back).copied().collect();
        if valid.is_empty() {
            return Ok(Found::No);
        }
        let rset = prune(&nodes, &valid);
        let mut live: HashMap<i64, HashSet<SR>> =
            rset.iter().map(|a| (*a, HashSet::new())).collect();
        let mut order: Vec<i64> = rset.iter().copied().collect();
        order.sort_unstable_by(|a, b| b.cmp(a));
        let mut more = true;
        while more {
            more = false;
            for a in &order {
                let n = &nodes[a];
                let mut out: HashSet<SR> = HashSet::new();
                for s in &n.succ {
                    if *s == t {
                        if valid.contains(a) {
                            out.extend(live.get(&t).cloned().unwrap_or_default());
                        }
                    } else if rset.contains(s) {
                        out.extend(live[s].iter().copied());
                    }
                }
                let defs = n.defs.clone().unwrap_or_default();
                let mut new: HashSet<SR> = n.uses.clone().unwrap_or_default();
                new.extend(out.difference(&defs).copied());
                if new != live[a] {
                    live.insert(*a, new);
                    more = true;
                }
            }
        }
        let mut defs: HashSet<SR> = HashSet::new();
        for a in &rset {
            defs.extend(nodes[a].defs.clone().unwrap_or_default());
        }
        let any_kind = |f: &dyn Fn(&Node) -> bool| rset.iter().any(|a| f(&nodes[a]));
        if any_kind(&|n| n.kind == NK::Stc)
            && !any_kind(&|n| {
                matches!(n.kind, NK::Rmw | NK::Acq | NK::Rel | NK::Tst)
                    || n.ins[0].mn.starts_with("ld")
            })
        {
            return Ok(Found::No);
        }
        let vary: HashSet<i64> = rset
            .iter()
            .filter(|a| {
                let n = &nodes[a];
                n.kind == NK::Stc
                    && n.uses
                        .as_ref()
                        .is_some_and(|u| u.intersection(&defs).next().is_some())
            })
            .copied()
            .collect();
        if !vary.is_empty() {
            return Ok(Found::Vary(vary));
        }
        if rset.iter().filter(|a| nodes[a].kind == NK::Stc).count() > SPIN_STORES {
            return Ok(Found::No);
        }
        if live[&t].intersection(&defs).next().is_some() {
            return Ok(Found::No);
        }
        let mut r: Vec<i64> = rset.iter().copied().collect();
        r.sort_unstable();
        Ok(Found::Yes(Region { t, r, nodes, valid }))
    }

    #[allow(clippy::too_many_arguments)]
    fn walk(
        nodes: &HashMap<i64, Node>,
        rset: &HashSet<i64>,
        t: i64,
        a: i64,
        onpath: &mut HashSet<i64>,
        ver: &HashMap<SR, i64>,
        delta: &HashMap<(bool, SR, i64), i64>,
        cnt: &mut [i64; 2],
        bad_back: &mut BTreeSet<i64>,
    ) -> bool {
        cnt[1] += 1;
        if cnt[1] > SPIN_STEPS {
            return false;
        }
        let n = &nodes[&a];
        let mut delta = delta.clone();
        if matches!(n.kind, NK::Rmw | NK::Acq | NK::Rel) {
            let key = (
                n.kind != NK::Rmw,
                n.base,
                ver.get(&n.base).copied().unwrap_or(0),
            );
            let e = delta.entry(key).or_insert(0);
            *e += n.k;
            if n.kind != NK::Rmw && !(0..=1).contains(e) {
                return false;
            }
        }
        let mut ver = ver.clone();
        if let Some(d) = &n.defs
            && !d.is_empty()
        {
            for r in d {
                *ver.entry(*r).or_insert(0) += 1;
            }
        }
        for s in &n.succ {
            if *s == t {
                cnt[0] += 1;
                if cnt[0] > SPIN_PATHS {
                    return false;
                }
                if delta.values().any(|v| *v != 0) {
                    bad_back.insert(a);
                }
            } else if rset.contains(s) {
                if onpath.contains(s) {
                    return false;
                }
                onpath.insert(*s);
                let ok = Self::walk(nodes, rset, t, *s, onpath, &ver, &delta, cnt, bad_back);
                onpath.remove(s);
                if !ok {
                    return false;
                }
            }
        }
        true
    }
}
