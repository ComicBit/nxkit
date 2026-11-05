//! Operands, operations, phi nodes and the arena that owns them.
#![allow(clippy::panic)] // a node of the wrong kind is an internal invariant violation, as the C# cast would throw

use super::dotnet_set::DotnetSet;
use super::inst::{Inst, StorageKind};
use crate::types::SamplerType;

/// Handle of an operand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Opd(pub u32);

/// Handle of a node (operation, phi or comment).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);

/// `OperandType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperandType {
    Argument,
    Constant,
    ConstantBuffer,
    Label,
    LocalVariable,
    Register,
    Undefined,
}

/// Register file kinds (`RegisterType`).
pub mod regtype {
    pub const FLAG: i32 = 0;
    pub const GPR: i32 = 1;
    pub const PREDICATE: i32 = 2;
    pub const UNIFORM: i32 = 3;
    pub const UNIFORM_PREDICATE: i32 = 4;
    pub const SHADOW: i32 = 5;
}

/// `TextureFlags`.
pub mod texflags {
    pub const NONE: u32 = 0;
    pub const BINDLESS: u32 = 1 << 0;
    pub const GATHER: u32 = 1 << 1;
    pub const DERIVATIVES: u32 = 1 << 2;
    pub const INT_COORDS: u32 = 1 << 3;
    pub const LOD_BIAS: u32 = 1 << 4;
    pub const LOD_LEVEL: u32 = 1 << 5;
    pub const OFFSET: u32 = 1 << 6;
    pub const OFFSETS: u32 = 1 << 7;
    pub const COHERENT: u32 = 1 << 8;
    pub const ATOMIC_BOUNDS_MASK: u32 = 3 << 20;
    pub const ATOMIC_DISCARD_RESULT: u32 = 1 << 22;
    pub const ATOMIC_METAL: u32 = 1 << 23;
    pub const ATOMIC_PREDICATE_RESULT: u32 = 1 << 24;
    pub const ATOMIC_MASK: u32 = 15 << 16;
    pub const ADD: u32 = 0;
    pub const MINIMUM: u32 = 1 << 16;
    pub const MAXIMUM: u32 = 2 << 16;
    pub const INCREMENT: u32 = 3 << 16;
    pub const DECREMENT: u32 = 4 << 16;
    pub const BITWISE_AND: u32 = 5 << 16;
    pub const BITWISE_OR: u32 = 6 << 16;
    pub const BITWISE_XOR: u32 = 7 << 16;
    pub const SWAP: u32 = 8 << 16;
    pub const CAS: u32 = 9 << 16;
}

/// `IrConsts.True`.
pub const TRUE: i32 = -1;
/// `IrConsts.False`.
pub const FALSE: i32 = 0;

const CBUF_SLOT_BITS: i32 = 5;
const CBUF_SLOT_LSB: i32 = 32 - CBUF_SLOT_BITS;
const CBUF_SLOT_MASK: i32 = (1 << CBUF_SLOT_BITS) - 1;

/// An operand with its def and use links (links only for local variables).
#[derive(Clone, Debug)]
pub struct OperandData {
    pub ty: OperandType,
    pub value: i32,
    pub asg_op: Option<NodeId>,
    pub use_ops: DotnetSet<NodeId>,
}

/// Texture fields of a `TextureOperation`.
#[derive(Clone, Debug)]
pub struct TexData {
    pub ty: SamplerType,
    pub format: usize,
    pub flags: u32,
    pub set: i32,
    pub binding: i32,
    pub sampler_set: i32,
    pub sampler_binding: i32,
}

/// A node of a basic block.
#[derive(Clone, Debug)]
pub enum Node {
    Op(Operation),
    Phi(PhiNode),
    Comment(String),
}

/// `Operation` and `TextureOperation`.
#[derive(Clone, Debug)]
pub struct Operation {
    pub inst: Inst,
    pub storage_kind: StorageKind,
    pub force_precise: bool,
    pub dests: Vec<Option<Opd>>,
    pub sources: Vec<Option<Opd>>,
    pub index: i32,
    pub tex: Option<TexData>,
}

/// `PhiNode`: sources keyed by block.
#[derive(Clone, Debug)]
pub struct PhiNode {
    pub dest: Option<Opd>,
    pub sources: Vec<(u32, Opd)>,
}

/// The arena.
#[derive(Default, Debug)]
pub struct Ir {
    pub operands: Vec<OperandData>,
    pub nodes: Vec<Node>,
}

impl Ir {
    /// A new operand of any kind (parsers and tests).
    pub fn operand(&mut self, ty: OperandType, value: i32) -> Opd {
        self.operands.push(OperandData {
            ty,
            value,
            asg_op: None,
            use_ops: DotnetSet::default(),
        });
        Opd(self.operands.len() as u32 - 1)
    }

    pub fn data(&self, o: Opd) -> &OperandData {
        &self.operands[o.0 as usize]
    }

    pub fn ty(&self, o: Opd) -> OperandType {
        self.data(o).ty
    }

    pub fn value(&self, o: Opd) -> i32 {
        self.data(o).value
    }

    pub fn argument(&mut self, v: i32) -> Opd {
        self.operand(OperandType::Argument, v)
    }

    pub fn cbuf(&mut self, slot: i32, offset: i32) -> Opd {
        self.operand(
            OperandType::ConstantBuffer,
            (slot << CBUF_SLOT_LSB) | offset,
        )
    }

    pub fn const_(&mut self, v: i32) -> Opd {
        self.operand(OperandType::Constant, v)
    }

    pub fn const_f(&mut self, v: f32) -> Opd {
        self.operand(OperandType::Constant, v.to_bits() as i32)
    }

    pub fn label(&mut self) -> Opd {
        self.operand(OperandType::Label, 0)
    }

    pub fn local(&mut self) -> Opd {
        self.operand(OperandType::LocalVariable, 0)
    }

    pub fn undef(&mut self) -> Opd {
        self.operand(OperandType::Undefined, 0)
    }

    /// `OperandHelper.Register`: RZ and URZ are 0, PT and UPT are true.
    pub fn register(&mut self, index: i32, ty: i32) -> Opd {
        let zero = (ty == regtype::GPR && index == 255) || (ty == regtype::UNIFORM && index == 63);
        let one = (ty == regtype::PREDICATE && index == 7)
            || (ty == regtype::UNIFORM_PREDICATE && index == 7);
        if zero {
            self.const_(0)
        } else if one {
            self.const_(TRUE)
        } else {
            self.operand(OperandType::Register, (ty << 24) | index)
        }
    }

    pub fn cbuf_slot(&self, o: Opd) -> i32 {
        (self.value(o) >> CBUF_SLOT_LSB) & CBUF_SLOT_MASK
    }

    pub fn cbuf_offset(&self, o: Opd) -> i32 {
        self.value(o) & !(CBUF_SLOT_MASK << CBUF_SLOT_LSB)
    }

    /// (index, type) of a register operand.
    pub fn reg_of(&self, o: Opd) -> (i32, i32) {
        let v = self.value(o);
        (v & 0xffffff, v >> 24)
    }

    fn add_node(&mut self, n: Node) -> NodeId {
        self.nodes.push(n);
        NodeId(self.nodes.len() as u32 - 1)
    }

    fn link_source(&mut self, node: NodeId, s: Option<Opd>) {
        if let Some(s) = s
            && self.ty(s) == OperandType::LocalVariable
        {
            self.operands[s.0 as usize].use_ops.add(node);
        }
    }

    fn link_dest(&mut self, node: NodeId, d: Option<Opd>) {
        if let Some(d) = d
            && self.ty(d) == OperandType::LocalVariable
        {
            self.operands[d.0 as usize].asg_op = Some(node);
        }
    }

    /// `new Operation(inst, dest, sources)` and the other plain constructors.
    pub fn new_op(
        &mut self,
        inst: Inst,
        kind: StorageKind,
        dest: Option<Opd>,
        sources: &[Opd],
    ) -> NodeId {
        let id = self.add_node(Node::Op(Operation {
            inst,
            storage_kind: kind,
            force_precise: false,
            dests: dest.into_iter().map(Some).collect(),
            sources: sources.iter().map(|s| Some(*s)).collect(),
            index: 0,
            tex: None,
        }));
        for s in sources {
            self.link_source(id, Some(*s));
        }
        // the C# constructors assign AsgOp for any dest type
        if let Some(d) = dest {
            self.operands[d.0 as usize].asg_op = Some(id);
        }
        id
    }

    /// An operation with several destinations and a component index (texture and memory forms).
    pub fn new_op_multi(
        &mut self,
        inst: Inst,
        index: i32,
        dests: &[Option<Opd>],
        sources: &[Opd],
    ) -> NodeId {
        let id = self.add_node(Node::Op(Operation {
            inst,
            storage_kind: StorageKind::None,
            force_precise: false,
            dests: dests.to_vec(),
            sources: sources.iter().map(|s| Some(*s)).collect(),
            index,
            tex: None,
        }));
        for s in sources {
            self.link_source(id, Some(*s));
        }
        for d in dests {
            if let Some(d) = d
                && self.ty(*d) == OperandType::LocalVariable
            {
                self.operands[d.0 as usize].asg_op = Some(id);
            }
        }
        id
    }

    /// `new TextureOperation(inst, type, format, flags, set, binding, compIndex, dests, sources)`.
    #[allow(clippy::too_many_arguments)]
    pub fn new_tex_op(
        &mut self,
        inst: Inst,
        ty: SamplerType,
        format: usize,
        flags: u32,
        set: i32,
        binding: i32,
        comp_index: i32,
        dests: &[Option<Opd>],
        sources: &[Opd],
    ) -> NodeId {
        let n = self.new_op_multi(inst, comp_index, dests, sources);
        self.op_mut(n).tex = Some(TexData {
            ty,
            format,
            flags,
            set,
            binding,
            sampler_set: -1,
            sampler_binding: -1,
        });
        n
    }

    pub fn op(&self, n: NodeId) -> &Operation {
        match &self.nodes[n.0 as usize] {
            Node::Op(o) => o,
            _ => panic!("node is not an operation"),
        }
    }

    pub fn op_mut(&mut self, n: NodeId) -> &mut Operation {
        match &mut self.nodes[n.0 as usize] {
            Node::Op(o) => o,
            _ => panic!("node is not an operation"),
        }
    }

    /// `SetSource`.
    pub fn set_source(&mut self, n: NodeId, i: usize, s: Option<Opd>) {
        let old = self.op(n).sources[i];
        if let Some(o) = old
            && self.ty(o) == OperandType::LocalVariable
        {
            self.operands[o.0 as usize].use_ops.remove(n);
        }
        self.link_source(n, s);
        self.op_mut(n).sources[i] = s;
    }

    /// `SetDest`.
    pub fn set_dest(&mut self, n: NodeId, i: usize, d: Option<Opd>) {
        let old = self.op(n).dests[i];
        if let Some(o) = old
            && self.ty(o) == OperandType::LocalVariable
        {
            self.operands[o.0 as usize].asg_op = None;
        }
        self.link_dest(n, d);
        self.op_mut(n).dests[i] = d;
    }

    /// `TurnInto(inst, source)`.
    pub fn turn_into(&mut self, n: NodeId, inst: Inst, source: Opd) {
        self.op_mut(n).inst = inst;
        let olds: Vec<Option<Opd>> = self.op(n).sources.clone();
        for o in olds.into_iter().flatten() {
            if self.ty(o) == OperandType::LocalVariable {
                self.operands[o.0 as usize].use_ops.remove(n);
            }
        }
        self.link_source(n, Some(source));
        self.op_mut(n).sources = vec![Some(source)];
    }

    pub fn turn_into_copy(&mut self, n: NodeId, source: Opd) {
        self.turn_into(n, Inst::Copy, source);
    }

    /// `InsertSource`.
    pub fn insert_source(&mut self, n: NodeId, index: usize, s: Opd) {
        self.link_source(n, Some(s));
        self.op_mut(n).sources.insert(index, Some(s));
    }

    /// `Operation.AppendSources`.
    pub fn append_sources(&mut self, n: NodeId, sources: &[Opd]) {
        for s in sources {
            self.link_source(n, Some(*s));
        }
        self.op_mut(n)
            .sources
            .extend(sources.iter().map(|s| Some(*s)));
    }

    /// `Operation.AppendDests`.
    pub fn append_dests(&mut self, n: NodeId, dests: &[Option<Opd>]) {
        for d in dests {
            self.link_dest(n, *d);
        }
        self.op_mut(n).dests.extend_from_slice(dests);
    }

    /// `new PhiNode(dest)`.
    pub fn new_phi(&mut self, dest: Opd) -> NodeId {
        let id = self.add_node(Node::Phi(PhiNode {
            dest: Some(dest),
            sources: vec![],
        }));
        self.operands[dest.0 as usize].asg_op = Some(id);
        id
    }

    /// `PhiNode.AddSource`: ignored when the block already has a source.
    pub fn phi_add_source(&mut self, n: NodeId, block: u32, o: Opd) {
        let Node::Phi(p) = &self.nodes[n.0 as usize] else {
            panic!("not a phi")
        };
        if p.sources.iter().any(|(b, _)| *b == block) {
            return;
        }
        self.link_source(n, Some(o));
        let Node::Phi(p) = &mut self.nodes[n.0 as usize] else {
            unreachable!()
        };
        p.sources.push((block, o));
    }

    /// Sources of any node (`INode.GetSource`): phi sources, operation sources, none for comments.
    pub fn node_sources(&self, n: NodeId) -> Vec<Option<Opd>> {
        match &self.nodes[n.0 as usize] {
            Node::Op(o) => o.sources.clone(),
            Node::Phi(p) => p.sources.iter().map(|(_, o)| Some(*o)).collect(),
            Node::Comment(_) => vec![],
        }
    }

    /// Destinations of any node (`INode.GetDest`).
    pub fn node_dests(&self, n: NodeId) -> Vec<Option<Opd>> {
        match &self.nodes[n.0 as usize] {
            Node::Op(o) => o.dests.clone(),
            Node::Phi(p) => vec![p.dest],
            Node::Comment(_) => vec![],
        }
    }

    /// `INode.SetSource` for operations and phi nodes.
    pub fn set_node_source(&mut self, n: NodeId, i: usize, s: Option<Opd>) {
        if let Node::Phi(p) = &self.nodes[n.0 as usize] {
            let old = p.sources[i].1;
            if self.ty(old) == OperandType::LocalVariable {
                self.operands[old.0 as usize].use_ops.remove(n);
            }
            if let Some(s) = s {
                self.link_source(n, Some(s));
                if let Node::Phi(p) = &mut self.nodes[n.0 as usize] {
                    p.sources[i].1 = s;
                }
            }
        } else {
            self.set_source(n, i, s);
        }
    }

    /// The operation (not phi, not comment) that assigns an operand (`AsgOp is Operation`).
    pub fn asg_operation(&self, o: Opd) -> Option<NodeId> {
        self.data(o)
            .asg_op
            .filter(|n| matches!(self.nodes[n.0 as usize], Node::Op(_)))
    }

    pub fn is_op(&self, n: NodeId) -> bool {
        matches!(self.nodes[n.0 as usize], Node::Op(_))
    }

    pub fn is_phi(&self, n: NodeId) -> bool {
        matches!(self.nodes[n.0 as usize], Node::Phi(_))
    }

    /// `Operand.AsFloat`.
    pub fn as_float(&self, o: Opd) -> f32 {
        f32::from_bits(self.value(o) as u32)
    }

    /// `Operation.RemoveSource`: clear the source (use link) and drop the slot.
    pub fn remove_source(&mut self, n: NodeId, i: usize) {
        self.set_source(n, i, None);
        self.op_mut(n).sources.remove(i);
    }

    /// `Operation.Dest = null` (no destinations left).
    pub fn clear_dests(&mut self, n: NodeId) {
        self.op_mut(n).dests = vec![];
    }

    pub fn new_comment(&mut self, text: &str) -> NodeId {
        self.add_node(Node::Comment(text.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links() {
        let mut ir = Ir::default();
        let a = ir.local();
        let b = ir.local();
        let one = ir.const_(1);
        let n = ir.new_op(Inst::Add, StorageKind::None, Some(a), &[one, b]);
        assert_eq!(ir.data(a).asg_op, Some(n));
        assert_eq!(ir.data(b).use_ops.iter().collect::<Vec<_>>(), vec![n]);
        ir.turn_into_copy(n, one);
        assert!(ir.data(b).use_ops.is_empty());
        assert_eq!(
            ir.register(255, regtype::GPR),
            Opd(ir.operands.len() as u32 - 1)
        );
        assert_eq!(ir.value(Opd(ir.operands.len() as u32 - 1)), 0);
    }
}
