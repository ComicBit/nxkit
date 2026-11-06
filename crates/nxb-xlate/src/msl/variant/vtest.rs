//! A builder of small vertex programs for the tests of the vertex variants.

#![allow(dead_code)]

use crate::attr::Stage;
use crate::codegen::props::Props;
use crate::driver::Program;
use crate::ir::{Inst, IoVariable, StorageKind};
use crate::job::parse_jobs;
use crate::msl::emit::Options;
use crate::msl::entry::build;
use crate::msl::{Context, StageInputs, Translated, Varying};
use crate::structured::ast::{A, BlockType};
use crate::structured::types::AggType;
use crate::structured::{StructFunction, StructInfo};
use crate::translation::msl_entry::Structured;

pub(super) const ONE: i32 = 0x3f80_0000;

/// A vertex program of one function under construction.
pub(super) struct V {
    pub(super) s: Structured,
    pub(super) main: A,
}

impl V {
    /// A vertex program with the attributes `attrs` (location, type code of the answer table: 0 float, 1 int, 2 uint).
    pub(super) fn new(attrs: &[(i32, u32)]) -> V {
        let job = parse_jobs(r#"[{"key": "k", "stage": "vs"}]"#)
            .expect("job")
            .remove(0);
        let mut info = StructInfo::default();
        let main = info.ast.new_block(BlockType::Main, None);
        info.functions.push(StructFunction {
            main_block: main,
            name: "main".into(),
            return_type: AggType::VOID,
            in_arguments: vec![],
            out_arguments: vec![],
            locals: vec![],
        });
        let mut props = Props::default();
        props.definitions.stage = Some(Stage::Vertex);
        let top = attrs
            .iter()
            .map(|a| a.0)
            .max()
            .map_or(0, |m| m as usize + 1);
        props.definitions.attribute_types = vec![0; top];
        for (l, t) in attrs {
            props.definitions.attribute_types[*l as usize] = *t;
        }
        let mut v = V {
            s: Structured {
                job,
                info,
                props,
                program: Program::default(),
                notes: vec![],
                fetch1d: Default::default(),
            },
            main,
        };
        for (l, _) in attrs {
            v.io(StorageKind::Input, IoVariable::UserDefined, *l);
        }
        v.io(StorageKind::Output, IoVariable::Position, 0);
        v
    }

    pub(super) fn io(&mut self, kind: StorageKind, var: IoVariable, loc: i32) {
        self.s.info.add_io(crate::structured::IoDef {
            storage_kind: kind,
            io_variable: var,
            location: loc,
            component: 0,
        });
    }

    pub(super) fn k(&mut self, v: i32) -> A {
        self.s.info.ast.new_const(v)
    }

    pub(super) fn local(&mut self, ty: AggType) -> A {
        let l = self.s.info.ast.new_local(ty);
        self.s.info.functions[0].locals.push(l);
        l
    }

    pub(super) fn put_in(&mut self, block: A, n: A) {
        self.s.info.ast.add(block, n).expect("add");
    }

    pub(super) fn put(&mut self, n: A) {
        self.put_in(self.main, n);
    }

    pub(super) fn op(&mut self, inst: Inst, kind: StorageKind, src: Vec<A>) -> A {
        self.s.info.ast.new_op(inst, kind, false, 0, src)
    }

    /// `local = op` in `block`, and the local.
    pub(super) fn def_in(&mut self, block: A, ty: AggType, op: A) -> A {
        let l = self.local(ty);
        let a = self.s.info.ast.new_assign(l, op);
        self.put_in(block, a);
        l
    }

    pub(super) fn def(&mut self, ty: AggType, op: A) -> A {
        self.def_in(self.main, ty, op)
    }

    /// A load of component `comp` of attribute `loc` as `ty`.
    pub(super) fn attr(&mut self, loc: i32, comp: i32, ty: AggType) -> A {
        let src = vec![
            self.k(IoVariable::UserDefined.0 as i32),
            self.k(loc),
            self.k(comp),
        ];
        let op = self.op(Inst::Load, StorageKind::Input, src);
        self.def(ty, op)
    }

    /// A load of a scalar input builtin.
    pub(super) fn builtin(&mut self, var: IoVariable, ty: AggType) -> A {
        let src = vec![self.k(var.0 as i32)];
        let op = self.op(Inst::Load, StorageKind::Input, src);
        self.def(ty, op)
    }

    /// A store of `v` to component `comp` of an output.
    pub(super) fn store(&mut self, var: IoVariable, loc: Option<i32>, comp: i32, v: A) {
        self.store_in(self.main, var, loc, comp, v);
    }

    pub(super) fn store_in(
        &mut self,
        block: A,
        var: IoVariable,
        loc: Option<i32>,
        comp: i32,
        v: A,
    ) {
        let mut src = vec![self.k(var.0 as i32)];
        if let Some(l) = loc {
            src.push(self.k(l));
        }
        src.push(self.k(comp));
        src.push(v);
        let op = self.op(Inst::Store, StorageKind::Output, src);
        self.put_in(block, op);
    }

    pub(super) fn ret(&mut self) {
        let r = self.op(Inst::Return, StorageKind::None, vec![]);
        self.put(r);
    }

    /// A user output in the plan.
    pub(super) fn out(&mut self, loc: i32) {
        self.io(StorageKind::Output, IoVariable::UserDefined, loc);
    }

    pub(super) fn translate_with(&self, partners: &[StageInputs]) -> Translated {
        let ctx = Context {
            chain: "V",
            source: "p#0".into(),
            pc_paired: "",
            partners,
            no_facts: false,
        };
        build(&self.s, &ctx, &Options::default()).expect("build")
    }

    pub(super) fn translate(&self) -> Translated {
        self.translate_with(&[])
    }

    pub(super) fn msl(&self) -> String {
        self.translate().libraries[0].msl.clone()
    }
}

/// The inputs of a pixel shader.
pub(super) fn partner(v: &[(u32, &str, &str)]) -> StageInputs {
    StageInputs {
        varyings: v
            .iter()
            .map(|(l, t, i)| Varying {
                location: *l,
                msl_type: (*t).into(),
                interpolation: (*i).into(),
            })
            .collect(),
    }
}

pub(super) const F32: AggType = AggType::FP32;
pub(super) const U32: AggType = AggType::U32;
pub(super) const S32: AggType = AggType::S32;

/// The standard program: `position.x = a0.x + 1`, `loc1.z = same`.
pub(super) fn simple() -> V {
    let mut b = V::new(&[(0, 0)]);
    b.out(1);
    let x = b.attr(0, 0, F32);
    let one = b.k(ONE);
    let add = b.op(Inst::Add, StorageKind::None, vec![x, one]);
    let y = b.def(F32, add);
    b.store(IoVariable::Position, None, 0, y);
    b.store(IoVariable::UserDefined, Some(1), 2, y);
    b.ret();
    b
}
