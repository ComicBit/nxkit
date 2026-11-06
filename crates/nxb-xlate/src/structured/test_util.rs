//! Test helpers for the structured program: probe dumps of the last attempt of a key.

use crate::structured::StructResources;
use std::path::Path;

/// The text of the last `== attempt` section of a phase dump (the attempt that produced the final output).
pub(super) fn last_attempt(dir: &Path, key: &str, phase: &str) -> Option<String> {
    let t = std::fs::read_to_string(dir.join(format!("{key}.{phase}.txt"))).ok()?;
    let part = t.split("== attempt\n").filter(|p| !p.is_empty()).last()?;
    Some(format!("== attempt\n{part}"))
}

/// A resource manager answering from the dumped final slot to binding map.
pub(super) struct ReplayResources {
    pub(super) map: Vec<(i32, i32, i32)>,
}

impl StructResources for ReplayResources {
    fn constant_buffer_binding(&mut self, slot: i32) -> Result<i32, String> {
        self.map
            .iter()
            .find(|(s, _, _)| *s == slot)
            .map(|(_, _, b)| *b)
            .ok_or_else(|| format!("no binding for constant buffer slot {slot}"))
    }

    fn set_used_constant_buffer_binding(&mut self, _binding: i32) {}
}

/// Reorders the use sets of the parsed locals as the probe's `suses` dump lists them (the C# `HashSet` enumeration order
/// depends on the history of the optimisations, which a text dump cannot carry).
pub(super) fn apply_use_order(
    ir: &mut crate::ir::Ir,
    funcs: &[crate::ir::text::FuncDump],
    suses: &str,
) -> Result<(), String> {
    use crate::ir::{DotnetSet, Node, Opd, OperandType};
    let mut all = Vec::new();
    for f in funcs {
        for b in &f.blocks {
            all.extend(b.ops.iter().copied());
        }
    }
    let mut cur: Vec<Opd> = Vec::new();
    for line in suses.lines() {
        if let Some(r) = line.strip_prefix("f ") {
            let fi: usize = r.trim().parse().map_err(|_| "suses: func")?;
            let mut order: Vec<Opd> = Vec::new();
            let mut see = |ir: &crate::ir::Ir, o: Option<Opd>| {
                if let Some(o) = o
                    && ir.ty(o) == OperandType::LocalVariable
                    && !order.contains(&o)
                {
                    order.push(o);
                }
            };
            for b in &funcs[fi].blocks {
                for n in &b.ops {
                    match &ir.nodes[n.0 as usize] {
                        Node::Phi(p) => {
                            see(ir, p.dest);
                            for (_, s) in &p.sources {
                                see(ir, Some(*s));
                            }
                        }
                        Node::Op(o) => {
                            for d in &o.dests {
                                see(ir, *d);
                            }
                            for s in &o.sources {
                                see(ir, *s);
                            }
                        }
                        Node::Comment(_) => {}
                    }
                }
            }
            cur = order;
        } else if let Some(r) = line.strip_prefix("u ") {
            let (k, rest) = r.split_once(' ').ok_or("suses: u line")?;
            let k: usize = k.parse().map_err(|_| "suses: k")?;
            let (slots, free) = rest.split_once(" free=").ok_or("suses: free")?;
            let mut set = DotnetSet::default();
            let mut holes: Vec<(usize, crate::ir::NodeId)> = Vec::new();
            for (slot, id) in slots.split(',').filter(|x| !x.is_empty()).enumerate() {
                let node = if id == "_" {
                    let h = ir.new_comment("hole");
                    holes.push((slot, h));
                    h
                } else if id == "-" {
                    ir.new_comment("stale")
                } else {
                    let i: usize = id.parse().map_err(|_| "suses: id")?;
                    *all.get(i).ok_or("suses: node id")?
                };
                set.add(node);
            }
            // the head of the free chain is reused first: it must be removed last
            let chain: Vec<usize> = free
                .split(',')
                .filter(|x| !x.is_empty())
                .map(|x| x.parse().map_err(|_| "suses: free idx"))
                .collect::<Result<_, _>>()?;
            for slot in chain.iter().rev() {
                let (_, h) = holes
                    .iter()
                    .find(|(s, _)| s == slot)
                    .ok_or("suses: free slot is not a hole")?;
                set.remove(*h);
            }
            let o = *cur.get(k).ok_or("suses: local")?;
            ir.operands[o.0 as usize].use_ops = set;
        }
    }
    Ok(())
}
