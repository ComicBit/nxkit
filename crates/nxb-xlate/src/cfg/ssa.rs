//! `Ssa.Rename`.
use super::Cfg;
use crate::ir::{Block, Ir, Node, NodeId, Opd, OperandType, regtype};
use std::collections::{HashMap, HashSet};

const GPRS: i32 = 255;
const PREDS: i32 = 7;
const FLAGS: i32 = 4;
const UNIFORMS: i32 = 63;
const UNIFORM_PREDS: i32 = 7;
const SHADOWS: i32 = 256;
const GPRS_AND_PREDS: i32 = GPRS + PREDS;
const UNIFORMS_BASE: i32 = GPRS_AND_PREDS + FLAGS;
const UNIFORM_PREDS_BASE: i32 = UNIFORMS_BASE + UNIFORMS;
const SHADOWS_BASE: i32 = UNIFORM_PREDS_BASE + UNIFORM_PREDS;
const TOTAL: usize = (SHADOWS_BASE + SHADOWS) as usize;

fn key_of(ir: &Ir, o: Opd) -> i32 {
    let (idx, ty) = ir.reg_of(o);
    key_from(idx, ty)
}

fn key_from(idx: i32, ty: i32) -> i32 {
    match ty {
        regtype::GPR => idx,
        regtype::PREDICATE => GPRS + idx,
        regtype::FLAG => GPRS_AND_PREDS + idx,
        regtype::UNIFORM => UNIFORMS_BASE + idx,
        regtype::UNIFORM_PREDICATE => UNIFORM_PREDS_BASE + idx,
        _ => SHADOWS_BASE + idx,
    }
}

#[derive(Default)]
struct DefMap {
    map: HashMap<i32, Opd>,
    phis: HashSet<i32>,
}

struct LocalDefs {
    map: Vec<Option<Opd>>,
    uses: Vec<i32>,
}

impl LocalDefs {
    fn new() -> Self {
        LocalDefs {
            map: vec![None; TOTAL],
            uses: vec![],
        }
    }
    fn add(&mut self, key: i32, o: Opd) {
        if self.map[key as usize].is_none() {
            self.uses.push(key);
        }
        self.map[key as usize] = Some(o);
    }
    fn clear(&mut self) {
        for k in &self.uses {
            self.map[*k as usize] = None;
        }
        self.uses.clear();
    }
}

struct Def {
    block: usize,
    local: Opd,
}

struct St<'a> {
    ir: &'a mut Ir,
    blocks: &'a mut Vec<Block>,
    idom: &'a [Option<usize>],
    defs: Vec<DefMap>,
}

impl St<'_> {
    fn idom(&self, b: usize) -> Result<usize, String> {
        self.idom[b].ok_or_else(|| "block without immediate dominator".to_string())
    }

    fn find_definition_for_curr(&mut self, cur: usize, key: i32) -> Result<Opd, String> {
        if self.defs[cur].phis.contains(&key) {
            return self.insert_phi(cur, key);
        }
        let id = self.idom(cur)?;
        if cur != id {
            return Ok(self.find_definition(id, key)?.local);
        }
        Ok(self.ir.undef())
    }

    fn find_definition(&mut self, cur: usize, key: i32) -> Result<Def, String> {
        let mut block = cur;
        loop {
            if let Some(&d) = self.defs[block].map.get(&key) {
                return Ok(Def { block, local: d });
            }
            if self.defs[block].phis.contains(&key) {
                let local = self.insert_phi(block, key)?;
                return Ok(Def { block, local });
            }
            let id = self.idom(block)?;
            if block == id {
                break;
            }
            block = id;
        }
        Ok(Def {
            block: cur,
            local: self.ir.undef(),
        })
    }

    fn insert_phi(&mut self, block: usize, key: i32) -> Result<Opd, String> {
        let local = self.ir.local();
        let phi = self.ir.new_phi(local);
        self.add_phi(block, phi);
        self.defs[block].map.entry(key).or_insert(local);
        let preds = self.blocks[block].preds.clone();
        for p in preds {
            let d = self.find_definition(p, key)?;
            self.ir.phi_add_source(phi, d.block as u32, d.local);
        }
        Ok(local)
    }

    fn add_phi(&mut self, block: usize, phi: NodeId) {
        let ops = &mut self.blocks[block].ops;
        let mut n = 0;
        while n < ops.len() && matches!(self.ir.nodes[ops[n].0 as usize], Node::Phi(_)) {
            n += 1;
        }
        ops.insert(n, phi);
    }
}

/// `Ssa.Rename`.
pub fn rename(ir: &mut Ir, cfg: &mut Cfg) -> Result<(), String> {
    let nblocks = cfg.blocks.len();
    let mut local_defs = LocalDefs::new();
    let mut defs: Vec<DefMap> = (0..nblocks).map(|_| DefMap::default()).collect();

    // First pass.
    for bi in 0..nblocks {
        let ops = cfg.blocks[bi].ops.clone();
        for n in ops {
            if !matches!(ir.nodes[n.0 as usize], Node::Op(_)) {
                continue;
            }
            for i in 0..ir.op(n).sources.len() {
                let mut s = ir.op(n).sources[i];
                if let Some(o) = s
                    && ir.ty(o) == OperandType::Register
                    && let Some(l) = local_defs.map[key_of(ir, o) as usize]
                {
                    s = Some(l);
                }
                ir.set_source(n, i, s);
            }
            for i in 0..ir.op(n).dests.len() {
                if let Some(d) = ir.op(n).dests[i]
                    && ir.ty(d) == OperandType::Register
                {
                    let local = ir.local();
                    local_defs.add(key_of(ir, d), local);
                    ir.set_dest(n, i, Some(local));
                }
            }
        }
        let uses = local_defs.uses.clone();
        for key in uses {
            let local = local_defs.map[key as usize].ok_or("local def")?;
            defs[bi].map.entry(key).or_insert(local);
            let mut queue = std::collections::VecDeque::new();
            queue.push_back(bi);
            while let Some(b) = queue.pop_front() {
                for df in cfg.frontiers[b].iter() {
                    if defs[df].phis.insert(key) {
                        queue.push_back(df);
                    }
                }
            }
        }
        local_defs.clear();
    }

    // Second pass.
    let idom = cfg.idom.clone();
    let mut st = St {
        ir,
        blocks: &mut cfg.blocks,
        idom: &idom,
        defs,
    };
    for bi in 0..nblocks {
        let ops = st.blocks[bi].ops.clone();
        for n in ops {
            if !matches!(st.ir.nodes[n.0 as usize], Node::Op(_)) {
                continue;
            }
            for i in 0..st.ir.op(n).sources.len() {
                let mut s = st.ir.op(n).sources[i];
                if let Some(o) = s
                    && st.ir.ty(o) == OperandType::Register
                {
                    let key = key_of(st.ir, o);
                    let r = match local_defs.map[key as usize] {
                        Some(l) => l,
                        None => {
                            let d = st.find_definition_for_curr(bi, key)?;
                            local_defs.add(key, d);
                            d
                        }
                    };
                    s = Some(r);
                }
                st.ir.set_source(n, i, s);
            }
        }
        if bi + 1 < nblocks {
            local_defs.clear();
        }
    }
    Ok(())
}
