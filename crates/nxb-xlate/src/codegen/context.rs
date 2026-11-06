//! `CodeGenContext`: the SPIR-V module under construction plus the state of the generator (declared variables, labels,
//! locals, types), and the helpers that turn AST operands into SPIR-V values.

use super::props::*;
use super::spirv_op::op;
use crate::ir::{FALSE, IoVariable, OperandType, StorageKind, TRUE};
use crate::spv::{Inst, Lit, Module, Operand};
use crate::structured::ast::*;
use crate::structured::types::AggType;
use crate::structured::{IoDef, StructFunction, StructInfo};
use crate::types::SamplerType;
use std::collections::HashMap;

const SPIRV_VERSION_PACKED: u32 = (1 << 16) | (3 << 8);

/// A `Dictionary` that is only added to: enumeration is insertion order.
#[derive(Clone, Debug)]
pub struct OMap<K: PartialEq + Copy, V: Copy> {
    items: Vec<(K, V)>,
}

impl<K: PartialEq + Copy, V: Copy> Default for OMap<K, V> {
    fn default() -> Self {
        OMap { items: Vec::new() }
    }
}

impl<K: PartialEq + Copy, V: Copy> OMap<K, V> {
    pub fn insert(&mut self, k: K, v: V) {
        self.items.push((k, v));
    }

    pub fn get(&self, k: K) -> Option<V> {
        self.items.iter().find(|(x, _)| *x == k).map(|(_, v)| *v)
    }

    pub fn values(&self) -> impl Iterator<Item = V> + '_ {
        self.items.iter().map(|(_, v)| *v)
    }
}

/// `OperationResult`.
#[derive(Clone, Copy, Debug)]
pub struct OpResult {
    pub ty: AggType,
    pub value: Option<Inst>,
}

impl OpResult {
    pub const INVALID: OpResult = OpResult {
        ty: AggType::INVALID,
        value: None,
    };

    pub fn new(ty: AggType, value: Inst) -> OpResult {
        OpResult {
            ty,
            value: Some(value),
        }
    }
}

/// `SamplerDeclaration`.
#[derive(Clone, Copy, Debug)]
pub struct SamplerDecl {
    pub image_type: Inst,
    pub sampled_image_type: Inst,
    pub sampled_image_pointer_type: Inst,
    pub image: Inst,
    pub is_indexed: bool,
}

/// `ImageDeclaration`.
#[derive(Clone, Copy, Debug)]
pub struct ImageDecl {
    pub image_type: Inst,
    pub image_pointer_type: Inst,
    pub image: Inst,
    pub is_indexed: bool,
}

#[derive(Default)]
struct BlockState {
    entry_count: usize,
    labels: Vec<Inst>,
}

pub struct CodeGenContext {
    pub m: Module,
    pub ast: Ast,
    pub functions: Vec<StructFunction>,
    pub io_definitions: Vec<IoDef>,
    pub helper_functions_mask: u32,
    pub attribute_passthrough: i32,
    pub definitions: Definitions,
    pub properties: Properties,
    pub host: HostCaps,
    pub target_api: i32,
    pub constant_buffers: HashMap<i32, Inst>,
    pub storage_buffers: HashMap<i32, Inst>,
    pub local_memories: HashMap<i32, Inst>,
    pub shared_memories: HashMap<i32, Inst>,
    pub samplers_types: HashMap<(i32, i32), SamplerType>,
    pub samplers: HashMap<(i32, i32), SamplerDecl>,
    pub images: HashMap<(i32, i32), ImageDecl>,
    pub inputs: OMap<IoDef, Inst>,
    pub outputs: OMap<IoDef, Inst>,
    pub inputs_per_patch: OMap<IoDef, Inst>,
    pub outputs_per_patch: OMap<IoDef, Inst>,
    pub current_function: usize,
    locals: HashMap<A, Inst>,
    func_args: HashMap<i32, Inst>,
    pub spv_functions: Vec<(usize, Inst)>,
    labels: HashMap<A, BlockState>,
    pub loop_targets: HashMap<A, (Inst, Inst)>,
    pub current_block: Option<A>,
    pub is_main_function: bool,
    pub may_have_returned: bool,
    pub was_non_uniform_access_declared: bool,
}

type R<T> = Result<T, String>;

impl CodeGenContext {
    pub fn new(info: StructInfo, props: Props) -> CodeGenContext {
        CodeGenContext {
            m: Module::new(SPIRV_VERSION_PACKED),
            ast: info.ast,
            functions: info.functions,
            io_definitions: info.io_definitions,
            helper_functions_mask: info.helper_functions_mask,
            attribute_passthrough: props.passthrough_attributes,
            definitions: props.definitions,
            properties: props.properties,
            host: props.host,
            target_api: props.target_api,
            constant_buffers: HashMap::new(),
            storage_buffers: HashMap::new(),
            local_memories: HashMap::new(),
            shared_memories: HashMap::new(),
            samplers_types: HashMap::new(),
            samplers: HashMap::new(),
            images: HashMap::new(),
            inputs: OMap::default(),
            outputs: OMap::default(),
            inputs_per_patch: OMap::default(),
            outputs_per_patch: OMap::default(),
            current_function: 0,
            locals: HashMap::new(),
            func_args: HashMap::new(),
            spv_functions: Vec::new(),
            labels: HashMap::new(),
            loop_targets: HashMap::new(),
            current_block: None,
            is_main_function: false,
            may_have_returned: false,
            was_non_uniform_access_declared: false,
        }
    }

    // ---- Module wrappers ----

    pub fn add_capability(&mut self, c: u32) {
        self.m.add_capability(c);
    }

    pub fn add_extension(&mut self, e: &str) {
        self.m.add_extension(e);
    }

    pub fn set_memory_model(&mut self, a: u32, mm: u32) {
        self.m.set_memory_model(a, mm);
    }

    pub fn add_label(&mut self, l: Inst) {
        self.m.add_label(l);
    }

    pub fn add_local_variable(&mut self, v: Inst) {
        self.m.add_local_variable(v);
    }

    pub fn add_global_variable(&mut self, v: Inst) {
        self.m.add_global_variable(v);
    }

    pub fn add_function(&mut self, f: Inst) {
        self.m.add_to_function_definitions(f);
    }

    /// `ExtInst(resultType, set, instruction, parameters)`.
    pub fn ext_inst(
        &mut self,
        result_type: Inst,
        set: Inst,
        instruction: i32,
        params: &[Inst],
    ) -> Inst {
        let i = self.m.new_with_id(op::OpExtInst as u16, Some(result_type));
        self.m.add(i, Operand::Ref(set));
        self.m.add(i, Operand::Lit(Lit::I32(instruction)));
        self.m.add_all(i, params.iter().map(|&x| Operand::Ref(x)));
        self.m.add_to_function_definitions(i);
        i
    }

    // ---- function and block state ----

    pub fn start_function(&mut self, is_main: bool) {
        self.is_main_function = is_main;
        self.may_have_returned = false;
        self.locals.clear();
        self.func_args.clear();
    }

    pub fn enter_block(&mut self, block: A) {
        self.current_block = Some(block);
        let l = self.next_label_auto_increment(block);
        self.add_label(l);
    }

    fn state(&mut self, block: A) -> &mut BlockState {
        self.labels.entry(block).or_default()
    }

    fn label_at(&mut self, block: A, index: usize) -> Inst {
        while index >= self.state(block).labels.len() {
            let l = self.label();
            self.state(block).labels.push(l);
        }
        self.state(block).labels[index]
    }

    fn next_label_auto_increment(&mut self, block: A) -> Inst {
        let idx = self.state(block).entry_count;
        self.state(block).entry_count += 1;
        self.label_at(block, idx)
    }

    pub fn first_label(&mut self, block: A) -> Inst {
        self.label_at(block, 0)
    }

    pub fn next_label(&mut self, block: A) -> Inst {
        let idx = self.state(block).entry_count;
        self.label_at(block, idx)
    }

    pub fn new_block(&mut self) -> Inst {
        let label = self.label();
        self.branch(label);
        self.add_label(label);
        label
    }

    pub fn main_interface(&self) -> Vec<Inst> {
        let mut v: Vec<Inst> = self.inputs.values().collect();
        v.extend(self.outputs.values());
        v.extend(self.inputs_per_patch.values());
        v.extend(self.outputs_per_patch.values());
        v
    }

    pub fn declare_local(&mut self, local: A, spv: Inst) {
        self.locals.insert(local, spv);
    }

    pub fn declare_argument(&mut self, index: i32, spv: Inst) {
        self.func_args.insert(index, spv);
    }

    pub fn local_pointer(&self, local: A) -> R<Inst> {
        self.locals
            .get(&local)
            .copied()
            .ok_or_else(|| "local variable not declared".to_string())
    }

    pub fn argument_pointer(&self, value: i32) -> R<Inst> {
        self.func_args
            .get(&value)
            .copied()
            .ok_or_else(|| "argument not declared".to_string())
    }

    // ---- types ----

    pub fn type_s32(&mut self) -> Inst {
        self.type_int(Lit::I32(32), Lit::I32(1), false)
    }

    pub fn type_u32(&mut self) -> Inst {
        self.type_int(Lit::I32(32), Lit::I32(0), false)
    }

    pub fn type_fp32(&mut self) -> Inst {
        self.type_float(Lit::I32(32), false)
    }

    pub fn type_fp64(&mut self) -> Inst {
        self.type_float(Lit::I32(64), false)
    }

    /// `GetType(type, length)`.
    pub fn get_type(&mut self, ty: AggType, length: i32) -> R<Inst> {
        if ty.has(AggType::ARRAY) {
            let elem = self.get_type(AggType(ty.0 & !AggType::ARRAY.0), 1)?;
            if length > 0 {
                let u = self.type_u32();
                let len = self.constant(u, Lit::I32(length));
                return Ok(self.type_array(elem, len, false));
            }
            return Ok(self.type_runtime_array(elem, false));
        } else if ty.0 & AggType::ELEMENT_COUNT_MASK != 0 {
            let n = match ty.0 & AggType::ELEMENT_COUNT_MASK {
                x if x == AggType::VECTOR2.0 => 2,
                x if x == AggType::VECTOR3.0 => 3,
                x if x == AggType::VECTOR4.0 => 4,
                _ => 1,
            };
            let elem = self.get_type(AggType(ty.0 & !AggType::ELEMENT_COUNT_MASK), 1)?;
            return Ok(self.type_vector(elem, Lit::I32(n), false));
        }
        Ok(match ty {
            AggType::VOID => self.type_void(false),
            AggType::BOOL => self.type_bool(false),
            AggType::FP32 => self.type_fp32(),
            AggType::FP64 => self.type_fp64(),
            AggType::S32 => self.type_s32(),
            AggType::U32 => self.type_u32(),
            _ => return Err(format!("Invalid attribute type \"{}\".", ty.0)),
        })
    }

    pub fn ty(&mut self, ty: AggType) -> R<Inst> {
        self.get_type(ty, 1)
    }

    // ---- values ----

    pub fn get_fp32(&mut self, n: A) -> R<Inst> {
        self.get(AggType::FP32, n)
    }

    pub fn get_fp64(&mut self, n: A) -> R<Inst> {
        self.get(AggType::FP64, n)
    }

    pub fn get_s32(&mut self, n: A) -> R<Inst> {
        self.get(AggType::S32, n)
    }

    pub fn get_u32(&mut self, n: A) -> R<Inst> {
        self.get(AggType::U32, n)
    }

    /// `Get(type, node)`.
    pub fn get(&mut self, ty: AggType, node: A) -> R<Inst> {
        match self.ast.kind(node).clone() {
            Kind::Operation(o) => {
                let r = super::instructions::generate(self, &o)?;
                let v = r.value.ok_or("operation produced no value")?;
                self.bitcast_if_needed(ty, r.ty, v)
            }
            Kind::Operand {
                ty: oty,
                var_type,
                value,
            } => match oty {
                OperandType::Argument => self.get_argument(ty, var_type, value),
                OperandType::Constant => self.get_constant(ty, value),
                OperandType::LocalVariable => self.get_local(ty, node, var_type),
                OperandType::Undefined => self.get_undefined(ty),
                other => Err(format!("Invalid operand type \"{other:?}\".")),
            },
            _ => Err("get: node is neither an operation nor an operand".into()),
        }
    }

    /// `GetWithType`.
    pub fn get_with_type(&mut self, node: A) -> R<(Inst, AggType)> {
        match self.ast.kind(node).clone() {
            Kind::Operation(o) => {
                let r = super::instructions::generate(self, &o)?;
                Ok((r.value.ok_or("operation produced no value")?, r.ty))
            }
            Kind::Operand {
                ty: OperandType::LocalVariable,
                var_type,
                ..
            } => Ok((self.get_local(var_type, node, var_type)?, var_type)),
            _ => Err("Invalid operand type.".into()),
        }
    }

    fn get_undefined(&mut self, ty: AggType) -> R<Inst> {
        Ok(match ty {
            AggType::BOOL => {
                let t = self.type_bool(false);
                self.constant_false(t)
            }
            AggType::FP32 => {
                let t = self.type_fp32();
                self.constant(t, Lit::f32(0.0))
            }
            AggType::FP64 => {
                let t = self.type_fp64();
                self.constant(t, Lit::f64(0.0))
            }
            _ => {
                let t = self.get_type(ty, 1)?;
                self.constant(t, Lit::I32(0))
            }
        })
    }

    pub fn get_constant(&mut self, ty: AggType, value: i32) -> R<Inst> {
        Ok(match ty {
            AggType::BOOL => {
                let t = self.type_bool(false);
                if value != 0 {
                    self.constant_true(t)
                } else {
                    self.constant_false(t)
                }
            }
            AggType::FP32 => {
                let t = self.type_fp32();
                self.constant(t, Lit::F32(value as u32))
            }
            AggType::FP64 => {
                let t = self.type_fp64();
                self.constant(t, Lit::f64(f32::from_bits(value as u32) as f64))
            }
            AggType::S32 => {
                let t = self.type_s32();
                self.constant(t, Lit::I32(value))
            }
            AggType::U32 => {
                let t = self.type_u32();
                self.constant(t, Lit::U32(value as u32))
            }
            _ => return Err(format!("Invalid type \"{}\".", ty.0)),
        })
    }

    fn get_local(&mut self, dst: AggType, local: A, src: AggType) -> R<Inst> {
        let t = self.get_type(src, 1)?;
        let p = self.local_pointer(local)?;
        let l = self.load(t, p, None);
        self.bitcast_if_needed(dst, src, l)
    }

    fn get_argument(&mut self, dst: AggType, src: AggType, value: i32) -> R<Inst> {
        let t = self.get_type(src, 1)?;
        let p = self.argument_pointer(value)?;
        let l = self.load(t, p, None);
        self.bitcast_if_needed(dst, src, l)
    }

    /// `BitcastIfNeeded`.
    pub fn bitcast_if_needed(&mut self, dst: AggType, src: AggType, value: Inst) -> R<Inst> {
        if dst == src {
            return Ok(value);
        }
        if dst == AggType::BOOL {
            let tb = self.type_bool(false);
            let v = self.bitcast_if_needed(AggType::S32, src, value)?;
            let ts = self.type_s32();
            let zero = self.constant(ts, Lit::I32(0));
            Ok(self.i_not_equal(tb, v, zero))
        } else if src == AggType::BOOL {
            let ts = self.type_s32();
            let int_true = self.constant(ts, Lit::I32(TRUE));
            let ts = self.type_s32();
            let int_false = self.constant(ts, Lit::I32(FALSE));
            let ts = self.type_s32();
            let sel = self.select(ts, value, int_true, int_false);
            self.bitcast_if_needed(dst, AggType::S32, sel)
        } else {
            let t = self.get_type(dst, 1)?;
            Ok(self.bitcast(t, value))
        }
    }

    /// `Constant(TypeU32(), value)` with an `int` literal.
    pub fn const_u32_i(&mut self, v: i32) -> Inst {
        let t = self.type_u32();
        self.constant(t, Lit::I32(v))
    }

    /// `Constant(TypeS32(), value)`.
    pub fn const_s32(&mut self, v: i32) -> Inst {
        let t = self.type_s32();
        self.constant(t, Lit::I32(v))
    }

    pub fn io_variable_stage(&self) -> Option<crate::attr::Stage> {
        self.definitions.stage
    }
}

/// `IoMap.GetSpirvBuiltIn`: (BuiltIn, type), the type is Invalid for an unknown variable.
pub fn spirv_built_in(v: IoVariable) -> (u32, AggType) {
    use super::spirv_enums::built_in as b;
    let f32v = |n: AggType| AggType::FP32 | n;
    match v {
        IoVariable::BaseInstance => (b::BaseInstance, AggType::S32),
        IoVariable::BaseVertex => (b::BaseVertex, AggType::S32),
        IoVariable::ClipDistance => (b::ClipDistance, AggType::ARRAY | AggType::FP32),
        IoVariable::CtaId => (b::WorkgroupId, AggType::VECTOR3 | AggType::U32),
        IoVariable::DrawIndex => (b::DrawIndex, AggType::S32),
        IoVariable::FragmentCoord => (b::FragCoord, f32v(AggType::VECTOR4)),
        IoVariable::FragmentOutputDepth => (b::FragDepth, AggType::FP32),
        IoVariable::FrontFacing => (b::FrontFacing, AggType::BOOL),
        IoVariable::GlobalId => (b::GlobalInvocationId, AggType::VECTOR3 | AggType::U32),
        IoVariable::InstanceId => (b::InstanceId, AggType::S32),
        IoVariable::InstanceIndex => (b::InstanceIndex, AggType::S32),
        IoVariable::InvocationId => (b::InvocationId, AggType::S32),
        IoVariable::Layer => (b::Layer, AggType::S32),
        IoVariable::PatchVertices => (b::PatchVertices, AggType::S32),
        IoVariable::PointCoord => (b::PointCoord, f32v(AggType::VECTOR2)),
        IoVariable::PointSize => (b::PointSize, AggType::FP32),
        IoVariable::Position => (b::Position, f32v(AggType::VECTOR4)),
        IoVariable::PrimitiveId => (b::PrimitiveId, AggType::S32),
        IoVariable::SampleId => (b::SampleId, AggType::S32),
        IoVariable::SubgroupEqMask => (b::SubgroupEqMask, AggType::VECTOR4 | AggType::U32),
        IoVariable::SubgroupGeMask => (b::SubgroupGeMask, AggType::VECTOR4 | AggType::U32),
        IoVariable::SubgroupGtMask => (b::SubgroupGtMask, AggType::VECTOR4 | AggType::U32),
        IoVariable::SubgroupLaneId => (b::SubgroupLocalInvocationId, AggType::U32),
        IoVariable::SubgroupLeMask => (b::SubgroupLeMask, AggType::VECTOR4 | AggType::U32),
        IoVariable::SubgroupLtMask => (b::SubgroupLtMask, AggType::VECTOR4 | AggType::U32),
        IoVariable::TessellationCoord => (b::TessCoord, f32v(AggType::VECTOR3)),
        IoVariable::TessellationLevelInner => (b::TessLevelInner, AggType::ARRAY | AggType::FP32),
        IoVariable::TessellationLevelOuter => (b::TessLevelOuter, AggType::ARRAY | AggType::FP32),
        IoVariable::ThreadId => (b::LocalInvocationId, AggType::VECTOR3 | AggType::U32),
        IoVariable::ThreadKill | IoVariable::HelperInvocation => {
            (b::HelperInvocation, AggType::BOOL)
        }
        IoVariable::VertexId => (b::VertexId, AggType::S32),
        IoVariable::VertexIndex => (b::VertexIndex, AggType::S32),
        IoVariable::ViewportIndex => (b::ViewportIndex, AggType::S32),
        IoVariable::ViewportMask => (b::ViewportMaskNV, AggType::ARRAY | AggType::S32),
        _ => (0, AggType::INVALID),
    }
}

/// `StorageKind.IsOutput`.
pub fn is_output(k: StorageKind) -> bool {
    matches!(k, StorageKind::Output | StorageKind::OutputPerPatch)
}

/// `StorageKind.IsPerPatch`.
pub fn is_per_patch(k: StorageKind) -> bool {
    matches!(k, StorageKind::InputPerPatch | StorageKind::OutputPerPatch)
}
