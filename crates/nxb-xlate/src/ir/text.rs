//! Text form of the IR: the contract with the C# probe (`tools/xlate-probe/Probe.cs`).
//!
//! operand: `_` none, `a<n>` argument, `#<hex>` constant, `c<slot>:<off>` cbuf, `L<k>` label, `l<k>` local,
//! `R<type>:<idx>` register, `undef`. Locals and labels are numbered by first appearance in a function.
//! op line: `op <inst hex> <storage kind> <precise> <index> d=<a,b,> s=<a,b,>` (+ ` t=<type>,<fmt>,<flags>,<set>,<bind>,<sset>,<sbind>`),
//! phi line `phi <dest> <block>:<opd>,...`, comment line `cm <text>`.
//! Function header `func <n>`, block header ` block <i> next=<i|-1> branch=<i|-1> pred=<i>,...`.`== attempt` starts a run.

use super::block::Block;
use super::inst::{Inst, StorageKind};
use super::ops::*;
use crate::types::SamplerType;
use std::collections::HashMap;
use std::fmt::Write;

/// Numbering of locals and labels within one function.
#[derive(Default)]
pub struct Names {
    locals: HashMap<Opd, usize>,
    labels: HashMap<Opd, usize>,
}

impl Names {
    pub fn of(&mut self, ir: &Ir, o: Option<Opd>) -> String {
        let Some(o) = o else { return "_".into() };
        let d = ir.data(o);
        match d.ty {
            OperandType::Argument => format!("a{}", d.value),
            OperandType::Constant => format!("#{:x}", d.value as u32),
            OperandType::ConstantBuffer => format!("c{}:{}", ir.cbuf_slot(o), ir.cbuf_offset(o)),
            OperandType::Label => {
                let n = self.labels.len();
                format!("L{}", *self.labels.entry(o).or_insert(n))
            }
            OperandType::LocalVariable => {
                let n = self.locals.len();
                format!("l{}", *self.locals.entry(o).or_insert(n))
            }
            OperandType::Register => format!("R{}:{}", d.value >> 24, d.value & 0xffffff),
            OperandType::Undefined => "undef".into(),
        }
    }
}

/// One node as text (two spaces of indent, newline at the end).
pub fn print_node(ir: &Ir, n: NodeId, nm: &mut Names, out: &mut String) {
    match &ir.nodes[n.0 as usize] {
        Node::Comment(c) => {
            let _ = writeln!(out, "  cm {}", c.replace('\n', " "));
        }
        Node::Phi(p) => {
            let _ = write!(out, "  phi {} ", nm.of(ir, p.dest));
            for (b, s) in &p.sources {
                let _ = write!(out, "{}:{},", b, nm.of(ir, Some(*s)));
            }
            out.push('\n');
        }
        Node::Op(o) => {
            let _ = write!(
                out,
                "  op {:x} {} {} {} d=",
                o.inst.0, o.storage_kind.0, o.force_precise as i32, o.index
            );
            for d in &o.dests {
                let _ = write!(out, "{},", nm.of(ir, *d));
            }
            out.push_str(" s=");
            for s in &o.sources {
                let _ = write!(out, "{},", nm.of(ir, *s));
            }
            if let Some(t) = &o.tex {
                let _ = write!(
                    out,
                    " t={},{},{},{},{},{},{}",
                    t.ty.0, t.format, t.flags, t.set, t.binding, t.sampler_set, t.sampler_binding
                );
            }
            out.push('\n');
        }
    }
}

/// A linear operation list (the `emit` phase).
pub fn print_ops(ir: &Ir, func: usize, ops: &[NodeId], out: &mut String) {
    let _ = writeln!(out, "== attempt\nfunc {func}");
    let mut nm = Names::default();
    for n in ops {
        print_node(ir, *n, &mut nm, out);
    }
}

/// A function as blocks (the `cfg`, `ssa`, `opt`, `xform` phases).
pub fn print_blocks(ir: &Ir, func: usize, blocks: &[Block], out: &mut String) {
    let _ = writeln!(out, "== attempt\nfunc {func}");
    let mut nm = Names::default();
    let idx = |o: Option<usize>| o.map_or(-1, |v| v as i64);
    for b in blocks {
        let _ = write!(
            out,
            " block {} next={} branch={} pred=",
            b.index,
            idx(b.next),
            idx(b.branch)
        );
        for p in &b.preds {
            let _ = write!(out, "{p},");
        }
        out.push('\n');
        for n in &b.ops {
            print_node(ir, *n, &mut nm, out);
        }
    }
}

struct Parser<'a> {
    ir: &'a mut Ir,
    locals: HashMap<String, Opd>,
    labels: HashMap<String, Opd>,
}

impl Parser<'_> {
    fn opd(&mut self, t: &str) -> Result<Option<Opd>, String> {
        let bad = || format!("bad operand {t}");
        Ok(Some(match t.as_bytes().first().ok_or_else(bad)? {
            b'_' => return Ok(None),
            b'u' => self.ir.undef(),
            b'a' => self.ir.argument(t[1..].parse().map_err(|_| bad())?),
            b'#' => self
                .ir
                .const_(u32::from_str_radix(&t[1..], 16).map_err(|_| bad())? as i32),
            b'c' => {
                let (s, o) = t[1..].split_once(':').ok_or_else(bad)?;
                self.ir
                    .cbuf(s.parse().map_err(|_| bad())?, o.parse().map_err(|_| bad())?)
            }
            b'L' => match self.labels.get(t) {
                Some(o) => *o,
                None => {
                    let o = self.ir.label();
                    self.labels.insert(t.to_string(), o);
                    o
                }
            },
            b'l' => match self.locals.get(t) {
                Some(o) => *o,
                None => {
                    let o = self.ir.local();
                    self.locals.insert(t.to_string(), o);
                    o
                }
            },
            b'R' => {
                let (ty, i) = t[1..].split_once(':').ok_or_else(bad)?;
                let ty: i32 = ty.parse().map_err(|_| bad())?;
                let i: i32 = i.parse().map_err(|_| bad())?;
                self.ir.operand(OperandType::Register, (ty << 24) | i)
            }
            _ => return Err(bad()),
        }))
    }

    fn list(&mut self, t: &str) -> Result<Vec<Option<Opd>>, String> {
        t.split(',')
            .filter(|s| !s.is_empty())
            .map(|s| self.opd(s))
            .collect()
    }

    fn node(&mut self, line: &str) -> Result<NodeId, String> {
        let line = line.trim_start();
        if let Some(c) = line.strip_prefix("cm ") {
            return Ok(self.ir.new_comment(c));
        }
        if let Some(r) = line.strip_prefix("phi ") {
            let mut it = r.split(' ');
            let dest = self.opd(it.next().unwrap_or(""))?.ok_or("phi dest")?;
            let n = self.ir.new_phi(dest);
            for s in it.next().unwrap_or("").split(',').filter(|s| !s.is_empty()) {
                let (b, o) = s.split_once(':').ok_or("phi source")?;
                let o = self.opd(o)?.ok_or("phi source")?;
                self.ir
                    .phi_add_source(n, b.parse().map_err(|_| "phi block")?, o);
            }
            return Ok(n);
        }
        let r = line
            .strip_prefix("op ")
            .ok_or_else(|| format!("bad line {line}"))?;
        let f: Vec<&str> = r.split(' ').collect();
        if f.len() < 6 {
            return Err(format!("short op line {line}"));
        }
        let inst = Inst(u32::from_str_radix(f[0], 16).map_err(|_| "inst")?);
        let kind = StorageKind(f[1].parse().map_err(|_| "kind")?);
        let precise = f[2] == "1";
        let index: i32 = f[3].parse().map_err(|_| "index")?;
        let dests = self.list(f[4].strip_prefix("d=").ok_or("d=")?)?;
        let srcs: Vec<Opd> = self
            .list(f[5].strip_prefix("s=").ok_or("s=")?)?
            .into_iter()
            .map(|o| o.ok_or("null source"))
            .collect::<Result<_, _>>()?;
        let n = self.ir.new_op_multi(inst, index, &dests, &srcs);
        let op = self.ir.op_mut(n);
        op.storage_kind = kind;
        op.force_precise = precise;
        if let Some(t) = f.get(6).and_then(|t| t.strip_prefix("t=")) {
            let v: Vec<i64> = t.split(',').map(|x| x.parse().unwrap_or(0)).collect();
            if v.len() != 7 {
                return Err("tex fields".into());
            }
            op.tex = Some(TexData {
                ty: SamplerType(v[0] as u32),
                format: v[1] as usize,
                flags: v[2] as u32,
                set: v[3] as i32,
                binding: v[4] as i32,
                sampler_set: v[5] as i32,
                sampler_binding: v[6] as i32,
            });
        }
        Ok(n)
    }
}

/// A parsed function: its blocks (a linear list is one block).
pub struct FuncDump {
    pub index: usize,
    pub blocks: Vec<Block>,
}

/// Parse the text of the first attempt (or of one attempt section) into functions, creating nodes in `ir`.
pub fn parse(ir: &mut Ir, text: &str) -> Result<Vec<FuncDump>, String> {
    let mut funcs: Vec<FuncDump> = Vec::new();
    let mut p = Parser {
        ir,
        locals: HashMap::new(),
        labels: HashMap::new(),
    };
    for line in text.lines() {
        if line.starts_with("==") {
            continue;
        }
        if let Some(r) = line.strip_prefix("func ") {
            p.locals.clear();
            p.labels.clear();
            funcs.push(FuncDump {
                index: r.trim().parse().map_err(|_| "func index")?,
                blocks: vec![],
            });
        } else if let Some(r) = line.strip_prefix(" block ") {
            let f: Vec<&str> = r.split(' ').collect();
            let num = |s: &str, k: &str| -> Result<i64, String> {
                s.strip_prefix(k)
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| format!("bad block header {line}"))
            };
            let opt = |v: i64| if v < 0 { None } else { Some(v as usize) };
            let preds = f
                .get(3)
                .and_then(|s| s.strip_prefix("pred="))
                .ok_or("pred=")?
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.parse().map_err(|_| "pred"))
                .collect::<Result<Vec<usize>, _>>()?;
            let b = Block {
                index: f[0].parse().map_err(|_| "block index")?,
                ops: vec![],
                next: opt(num(f.get(1).unwrap_or(&""), "next=")?),
                branch: opt(num(f.get(2).unwrap_or(&""), "branch=")?),
                preds,
            };
            funcs.last_mut().ok_or("block before func")?.blocks.push(b);
        } else if !line.trim().is_empty() {
            let n = p.node(line)?;
            let f = funcs.last_mut().ok_or("node before func")?;
            if f.blocks.is_empty() {
                f.blocks.push(Block::default());
            }
            if let Some(b) = f.blocks.last_mut() {
                b.ops.push(n);
            }
        }
    }
    Ok(funcs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_all_phases() {
        let Some(entries) = crate::corpus::load() else {
            return;
        };
        for e in entries.iter().take(40) {
            for phase in ["emit", "cfg", "ssa", "opt", "xform"] {
                let Some(text) = e.first_attempt(phase) else {
                    continue;
                };
                let mut ir = Ir::default();
                let funcs = parse(&mut ir, &text).expect("parse");
                let mut out = String::new();
                for f in &funcs {
                    if phase == "emit" {
                        print_ops(&ir, f.index, &f.blocks[0].ops, &mut out);
                    } else {
                        print_blocks(&ir, f.index, &f.blocks, &mut out);
                    }
                }
                assert_eq!(out, text, "{} {phase}", e.job.key);
            }
        }
    }
}
