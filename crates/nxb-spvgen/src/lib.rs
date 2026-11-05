#![allow(missing_docs)]
//! `Spv.Generator.Module`: the SPIR-V module builder (id allocation, type and constant deduplication, section order).
//!
//! Instructions live in an arena and are named by [`Inst`] handles, because a label's id is assigned after branches to it
//! were built. Equality for deduplication compares operands the way the C# does (literal kind and value, referenced ids).

use std::collections::HashMap;

/// Handle of an instruction in the module arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Inst(pub usize);

const INVALID_ID: u32 = u32::MAX;
const MAGIC: u32 = 0x0723_0203;

/// `LiteralInteger`: the kind takes part in equality (an int 1 and a uint 1 are different operands).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lit {
    U32(u32),
    I32(i32),
    U64(u64),
    I64(i64),
    F32(u32),
    F64(u64),
}

impl Lit {
    /// A float operand from its value.
    pub fn f32(v: f32) -> Lit {
        Lit::F32(v.to_bits())
    }

    /// A double operand from its value.
    pub fn f64(v: f64) -> Lit {
        Lit::F64(v.to_bits())
    }

    fn data(self) -> u64 {
        match self {
            Lit::U32(v) | Lit::F32(v) => v as u64,
            Lit::I32(v) => v as u64,
            Lit::U64(v) | Lit::F64(v) => v,
            Lit::I64(v) => v as u64,
        }
    }

    fn words(self) -> usize {
        match self {
            Lit::U32(_) | Lit::I32(_) | Lit::F32(_) => 1,
            _ => 2,
        }
    }
}

/// One operand of an instruction.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Operand {
    Lit(Lit),
    Str(String),
    Ref(Inst),
}

#[derive(Clone, Debug)]
struct Instr {
    op: u16,
    id: u32,
    result_type: Option<Inst>,
    operands: Vec<Operand>,
}

/// Structural key: opcode, result type id, operands with referenced ids resolved.
#[derive(PartialEq, Eq, Hash)]
enum Key {
    Lit(Lit),
    Str(String),
    Id(u32),
}

type TypeKey = (u16, Vec<Key>);
type ConstKey = (u16, Vec<Key>, Option<TypeKey>);

/// The module under construction.
pub struct Module {
    version: u32,
    bound: u32,
    arena: Vec<Instr>,
    capabilities: Vec<u32>,
    extensions: Vec<String>,
    ext_inst_imports: Vec<(String, Inst)>,
    addressing_model: u32,
    memory_model: u32,
    entrypoints: Vec<Inst>,
    execution_modes: Vec<Inst>,
    debug: Vec<Inst>,
    annotations: Vec<Inst>,
    type_map: HashMap<(u16, Vec<Key>), Inst>,
    type_list: Vec<Inst>,
    globals: Vec<Inst>,
    const_map: HashMap<ConstKey, Inst>,
    const_list: Vec<Inst>,
    functions_declarations: Vec<Inst>,
    functions_definitions: Vec<Inst>,
}

/// Opcodes the module logic itself needs.
pub mod op {
    pub const EXT_INST_IMPORT: u16 = 11;
    pub const EXT_INST: u16 = 12;
    pub const MEMORY_MODEL: u16 = 14;
    pub const ENTRY_POINT: u16 = 15;
    pub const EXECUTION_MODE: u16 = 16;
    pub const CAPABILITY: u16 = 17;
    pub const EXTENSION: u16 = 10;
    pub const VARIABLE: u16 = 59;
    pub const LABEL: u16 = 248;
}

impl Module {
    /// `new Module(version)`, ids start at 1.
    pub fn new(version: u32) -> Module {
        Module {
            version,
            bound: 1,
            arena: Vec::new(),
            capabilities: Vec::new(),
            extensions: Vec::new(),
            ext_inst_imports: Vec::new(),
            addressing_model: 0,
            memory_model: 0,
            entrypoints: Vec::new(),
            execution_modes: Vec::new(),
            debug: Vec::new(),
            annotations: Vec::new(),
            type_map: HashMap::new(),
            type_list: Vec::new(),
            globals: Vec::new(),
            const_map: HashMap::new(),
            const_list: Vec::new(),
            functions_declarations: Vec::new(),
            functions_definitions: Vec::new(),
        }
    }

    fn new_id(&mut self) -> u32 {
        let id = self.bound;
        self.bound += 1;
        id
    }

    /// `NewInstruction(opcode, id, resultType)`, without an id (`InvalidId`).
    pub fn new_instruction(&mut self, op: u16, result_type: Option<Inst>) -> Inst {
        self.arena.push(Instr {
            op,
            id: INVALID_ID,
            result_type,
            operands: Vec::new(),
        });
        Inst(self.arena.len() - 1)
    }

    /// An instruction with a fresh id (result-producing function body instructions).
    pub fn new_with_id(&mut self, op: u16, result_type: Option<Inst>) -> Inst {
        let id = self.new_id();
        let i = self.new_instruction(op, result_type);
        self.arena[i.0].id = id;
        i
    }

    /// `AddOperand`.
    pub fn add(&mut self, i: Inst, o: Operand) {
        self.arena[i.0].operands.push(o);
    }

    /// Add several operands.
    pub fn add_all(&mut self, i: Inst, o: impl IntoIterator<Item = Operand>) {
        self.arena[i.0].operands.extend(o);
    }

    /// The id of an instruction (`INVALID_ID` while unbound).
    pub fn id(&self, i: Inst) -> u32 {
        self.arena[i.0].id
    }

    /// The opcode of an instruction.
    pub fn opcode(&self, i: Inst) -> u16 {
        self.arena[i.0].op
    }

    /// The number of ids allocated so far plus one.
    pub fn bound(&self) -> u32 {
        self.bound
    }

    /// `AddCapability`.
    pub fn add_capability(&mut self, c: u32) {
        self.capabilities.push(c);
    }

    /// `AddExtension`.
    pub fn add_extension(&mut self, e: &str) {
        self.extensions.push(e.to_string());
    }

    /// `SetMemoryModel`.
    pub fn set_memory_model(&mut self, addressing: u32, memory: u32) {
        self.addressing_model = addressing;
        self.memory_model = memory;
    }

    /// `AddExtInstImport`: one instruction per distinct name.
    pub fn add_ext_inst_import(&mut self, name: &str) -> Inst {
        if let Some((_, i)) = self.ext_inst_imports.iter().find(|(n, _)| n == name) {
            return *i;
        }
        let i = self.new_instruction(op::EXT_INST_IMPORT, None);
        self.add(i, Operand::Str(name.to_string()));
        let id = self.new_id();
        self.arena[i.0].id = id;
        self.ext_inst_imports.push((name.to_string(), i));
        i
    }

    fn keys(&self, i: Inst) -> Vec<Key> {
        self.arena[i.0]
            .operands
            .iter()
            .map(|o| match o {
                Operand::Lit(l) => Key::Lit(*l),
                Operand::Str(s) => Key::Str(s.clone()),
                Operand::Ref(r) => Key::Id(self.arena[r.0].id),
            })
            .collect()
    }

    /// `AddTypeDeclaration`: the instruction takes the id of an equal earlier declaration, else a new one.
    /// Returns the handle that owns the id (the earlier one when deduplicated, still with its own id).
    pub fn add_type(&mut self, i: Inst, force_id: bool) -> Inst {
        let key = (self.arena[i.0].op, self.keys(i));
        if !force_id && let Some(&t) = self.type_map.get(&key) {
            self.arena[i.0].id = self.arena[t.0].id;
            return t;
        }
        let id = self.new_id();
        self.arena[i.0].id = id;
        self.type_map.insert(key, i);
        self.type_list.push(i);
        i
    }

    /// `AddConstant`.
    pub fn add_constant(&mut self, i: Inst) -> Inst {
        let rt = self.arena[i.0]
            .result_type
            .map(|r| (self.arena[r.0].op, self.keys(r)));
        let key = (self.arena[i.0].op, self.keys(i), rt);
        if let Some(&c) = self.const_map.get(&key) {
            self.arena[i.0].id = self.arena[c.0].id;
            return c;
        }
        let id = self.new_id();
        self.arena[i.0].id = id;
        self.const_map.insert(key, i);
        self.const_list.push(i);
        i
    }

    /// `AddGlobalVariable`.
    pub fn add_global_variable(&mut self, v: Inst) {
        let id = self.new_id();
        self.arena[v.0].id = id;
        self.globals.push(v);
    }

    /// `AddLocalVariable`.
    pub fn add_local_variable(&mut self, v: Inst) {
        let id = self.new_id();
        self.arena[v.0].id = id;
        self.functions_definitions.push(v);
    }

    /// `AddLabel`.
    pub fn add_label(&mut self, l: Inst) {
        let id = self.new_id();
        self.arena[l.0].id = id;
        self.functions_definitions.push(l);
    }

    /// Append an instruction to the function section (it keeps the id it has).
    pub fn add_to_function_definitions(&mut self, i: Inst) {
        self.functions_definitions.push(i);
    }

    /// Append to the debug section.
    pub fn add_debug(&mut self, i: Inst) {
        self.debug.push(i);
    }

    /// Append to the annotation section.
    pub fn add_annotation(&mut self, i: Inst) {
        self.annotations.push(i);
    }

    /// `AddEntryPoint`.
    pub fn add_entry_point(&mut self, model: u32, function: Inst, name: &str, interfaces: &[Inst]) {
        let e = self.new_instruction(op::ENTRY_POINT, None);
        self.add(e, Operand::Lit(Lit::I32(model as i32)));
        self.add(e, Operand::Ref(function));
        self.add(e, Operand::Str(name.to_string()));
        self.add_all(e, interfaces.iter().map(|&x| Operand::Ref(x)));
        self.entrypoints.push(e);
    }

    /// `AddExecutionMode`.
    pub fn add_execution_mode(&mut self, function: Inst, mode: u32, params: &[Operand]) {
        let e = self.new_instruction(op::EXECUTION_MODE, None);
        self.add(e, Operand::Ref(function));
        self.add(e, Operand::Lit(Lit::I32(mode as i32)));
        self.add_all(e, params.iter().cloned());
        self.execution_modes.push(e);
    }

    fn words_of(&self, i: Inst) -> usize {
        let n = &self.arena[i.0];
        let mut w = 1;
        if n.id != INVALID_ID {
            w += 1;
        }
        if n.result_type.is_some() {
            w += 1;
        }
        for o in &n.operands {
            w += match o {
                Operand::Lit(l) => l.words(),
                Operand::Str(s) => s.len() / 4 + 1,
                Operand::Ref(_) => 1,
            };
        }
        w
    }

    fn write(&self, i: Inst, out: &mut Vec<u8>) -> Result<(), String> {
        let n = &self.arena[i.0];
        let head = (n.op as u32) | ((self.words_of(i) as u32) << 16);
        out.extend(head.to_le_bytes());
        if let Some(rt) = n.result_type {
            let id = self.arena[rt.0].id;
            if id == INVALID_ID {
                return Err("Id wasn't bound to the module".into());
            }
            out.extend(id.to_le_bytes());
        }
        if n.id != INVALID_ID {
            out.extend(n.id.to_le_bytes());
        }
        for o in &n.operands {
            match o {
                Operand::Lit(l) => {
                    if l.words() == 1 {
                        out.extend((l.data() as u32).to_le_bytes());
                    } else {
                        out.extend(l.data().to_le_bytes());
                    }
                }
                Operand::Str(s) => {
                    // BinaryWriter with an ASCII encoding: other characters become '?'
                    out.extend(s.chars().map(|c| if c.is_ascii() { c as u8 } else { b'?' }));
                    let pad = 4 - (s.chars().count() % 4);
                    out.extend(std::iter::repeat_n(0u8, pad));
                }
                Operand::Ref(r) => {
                    let id = self.arena[r.0].id;
                    if id == INVALID_ID {
                        return Err("Id wasn't bound to the module".into());
                    }
                    out.extend(id.to_le_bytes());
                }
            }
        }
        Ok(())
    }

    fn simple(&mut self, op: u16, operands: Vec<Operand>) -> Inst {
        let i = self.new_instruction(op, None);
        self.add_all(i, operands);
        i
    }

    /// `Generate()`: the module bytes.
    pub fn generate(&mut self) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        for w in [MAGIC, self.version, 0, self.bound, 0] {
            out.extend(w.to_le_bytes());
        }
        for c in self.capabilities.clone() {
            let i = self.simple(op::CAPABILITY, vec![Operand::Lit(Lit::I32(c as i32))]);
            self.write(i, &mut out)?;
        }
        for e in self.extensions.clone() {
            let i = self.simple(op::EXTENSION, vec![Operand::Str(e)]);
            self.write(i, &mut out)?;
        }
        for (_, i) in self.ext_inst_imports.clone() {
            self.write(i, &mut out)?;
        }
        let mm = self.simple(
            op::MEMORY_MODEL,
            vec![
                Operand::Lit(Lit::I32(self.addressing_model as i32)),
                Operand::Lit(Lit::I32(self.memory_model as i32)),
            ],
        );
        self.write(mm, &mut out)?;
        for list in [
            &self.entrypoints,
            &self.execution_modes,
            &self.debug,
            &self.annotations,
        ] {
            for &i in list {
                self.write(i, &mut out)?;
            }
        }
        let mut decls: Vec<Inst> = self.type_list.clone();
        decls.extend(&self.globals);
        decls.extend(&self.const_list);
        decls.sort_by_key(|i| self.arena[i.0].id);
        for list in [
            &decls,
            &self.functions_declarations,
            &self.functions_definitions,
        ] {
            for &i in list {
                self.write(i, &mut out)?;
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int_type(m: &mut Module, width: u32, signed: bool) -> Inst {
        let t = m.new_instruction(21, None);
        m.add(t, Operand::Lit(Lit::U32(width)));
        m.add(t, Operand::Lit(Lit::U32(signed as u32)));
        m.add_type(t, false)
    }

    #[test]
    fn dedup_and_order() {
        let mut m = Module::new(0x10300);
        m.add_capability(1);
        let a = int_type(&mut m, 32, false);
        let b = int_type(&mut m, 32, false);
        let c = int_type(&mut m, 32, true);
        assert_eq!(m.id(a), m.id(b));
        assert_ne!(m.id(a), m.id(c));
        let k = m.new_instruction(43, Some(a));
        m.add(k, Operand::Lit(Lit::U32(7)));
        let k1 = m.add_constant(k);
        let k2i = m.new_instruction(43, Some(b));
        m.add(k2i, Operand::Lit(Lit::U32(7)));
        let k2 = m.add_constant(k2i);
        assert_eq!(m.id(k1), m.id(k2));
        // an int 7 and a uint 7 differ
        let k3i = m.new_instruction(43, Some(a));
        m.add(k3i, Operand::Lit(Lit::I32(7)));
        m.add_constant(k3i);
        assert_eq!(m.bound(), 5);
        let bytes = m.generate().unwrap();
        assert_eq!(&bytes[..4], &MAGIC.to_le_bytes());
        // header 5 words, capability 2, memory model 3, int 4+4, constant 4, constant 4
        assert_eq!(bytes.len(), 4 * (5 + 2 + 3 + 4 + 4 + 4 + 4));
    }

    #[test]
    fn string_padding() {
        let mut m = Module::new(0x10300);
        let i = m.add_ext_inst_import("GLSL.std.450");
        let j = m.add_ext_inst_import("GLSL.std.450");
        assert_eq!(i, j);
        let b = m.generate().unwrap();
        // import: head, id, 12 chars plus 4 padding bytes = 4 words, header 5, memory model 3
        assert_eq!(b.len(), 4 * (5 + 3 + 6));
    }
}
