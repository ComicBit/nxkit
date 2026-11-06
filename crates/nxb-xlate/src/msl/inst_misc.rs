//! Subgroup operations (ballot, vote, shuffle, swizzle add), barriers and the statement instructions.

use super::emit::{Em, R, Shader, Val};
use super::types::literal;
use crate::ir::Inst;
use crate::structured::ast::{BlockType, OpData};
use crate::structured::types::AggType;

impl Em<'_> {
    /// Subgroup operations and barriers.
    pub(super) fn subgroup(&mut self, inst: Inst, o: &OpData) -> R<Option<Val>> {
        match inst {
            Inst::Ballot => self.ballot(o).map(Some),
            Inst::VoteAll | Inst::VoteAllEqual | Inst::VoteAny => {
                let a = self.src(o, 0)?;
                let p = self.get(AggType::BOOL, a)?;
                let s = match inst {
                    Inst::VoteAll => format!("simd_all({})", p.s),
                    Inst::VoteAny => format!("simd_any({})", p.s),
                    _ => {
                        let p = self.once(p)?;
                        format!("(simd_all({0}) || !simd_any({0}))", p.s)
                    }
                };
                Ok(Some(Val::new(s, AggType::BOOL)))
            }
            Inst::Shuffle | Inst::ShuffleDown | Inst::ShuffleUp | Inst::ShuffleXor => {
                let a = self.src(o, 0)?;
                let v = self.get(AggType::FP32, a)?;
                let a = self.src(o, 1)?;
                let i = self.get(AggType::U32, a)?;
                let f = match inst {
                    Inst::Shuffle => "simd_shuffle",
                    Inst::ShuffleDown => "simd_shuffle_down",
                    Inst::ShuffleUp => "simd_shuffle_up",
                    _ => "simd_shuffle_xor",
                };
                let call = format!("{f}({}, ushort({}))", v.s, i.s);
                Ok(Some(self.effect(call, AggType::FP32)?))
            }
            Inst::SwizzleAdd => self.swizzle_add(o).map(Some),
            Inst::Barrier => {
                let skip = !self.props.host.supports_shader_barrier_divergence
                    && (self.block_type() != Some(BlockType::Main)
                        || self.may_return
                        || self.func != 0);
                if !skip {
                    self.line("threadgroup_barrier(mem_flags::mem_threadgroup);");
                }
                Ok(None)
            }
            Inst::SubgroupBarrier => {
                self.line("simdgroup_barrier(mem_flags::mem_threadgroup);");
                Ok(None)
            }
            Inst::GroupMemoryBarrier => {
                self.line(
                    "atomic_thread_fence(mem_flags::mem_device | mem_flags::mem_threadgroup | mem_flags::mem_texture, memory_order_seq_cst, thread_scope_threadgroup);",
                );
                Ok(None)
            }
            _ => {
                self.line(
                    "atomic_thread_fence(mem_flags::mem_device | mem_flags::mem_threadgroup | mem_flags::mem_texture, memory_order_seq_cst, thread_scope_device);",
                );
                Ok(None)
            }
        }
    }

    pub(super) fn block_type(&self) -> Option<BlockType> {
        self.cur.and_then(|c| self.info.ast.block_type(c))
    }

    fn ballot(&mut self, o: &OpData) -> R<Val> {
        let a = self.src(o, 0)?;
        let p = self.get(AggType::BOOL, a)?;
        let s = if self.opt.visibility_counter {
            if o.index == 0 {
                format!("({} ? 1u : 0u)", p.s)
            } else {
                "0u".into()
            }
        } else {
            match o.index {
                0 => format!("uint(ulong(simd_ballot({})))", p.s),
                1 => format!("uint(ulong(simd_ballot({})) >> 32)", p.s),
                _ => "0u".into(),
            }
        };
        Ok(Val::new(s, AggType::U32))
    }

    fn swizzle_add(&mut self, o: &OpData) -> R<Val> {
        let a = self.src(o, 0)?;
        let x = self.get(AggType::FP32, a)?;
        let x = self.once(x)?;
        let a = self.src(o, 1)?;
        let y = self.get(AggType::FP32, a)?;
        let y = self.once(y)?;
        let a = self.src(o, 2)?;
        let m = self.get(AggType::U32, a)?;
        let lane = self.plan.lane();
        let idx = format!("(({} >> (({lane} & 3u) << 1u)) & 3u)", m.s);
        let idx = self.hoist(Val::new(idx, AggType::U32))?;
        let s = format!(
            "({} * float4(1.0, -1.0, 1.0, 0.0)[{idx}] + {} * float4(1.0, 1.0, -1.0, 1.0)[{idx}])",
            x.s,
            y.s,
            idx = idx.s
        );
        Ok(Val::new(s, AggType::FP32))
    }

    /// The instructions that are statements.
    pub(super) fn control_op(&mut self, inst: Inst, o: &OpData) -> R<Option<Val>> {
        match inst {
            Inst::Return => {
                self.may_return = true;
                self.return_stmt(o)?;
            }
            Inst::Discard => {
                self.line("discard_fragment();");
                self.return_default()?;
            }
            Inst::LoopBreak => self.line("break;"),
            Inst::LoopContinue => self.line("continue;"),
            Inst::FSIBegin | Inst::FSIEnd => {
                if self.props.host.supports_fragment_shader_interlock {
                    return Err("unported: fragment shader interlock".into());
                }
            }
            Inst::Comment => {}
            _ => return Err("unported: geometry stream output".into()),
        }
        Ok(None)
    }

    fn return_stmt(&mut self, o: &OpData) -> R<()> {
        if let Some(a) = o.sources.first() {
            let ty = self.info.functions[self.func].return_type;
            let v = self.get(ty, *a)?;
            self.line(&format!("return {};", v.s));
        } else {
            self.return_default()?;
        }
        Ok(())
    }

    /// A return without a value of the current function.
    pub(super) fn return_default(&mut self) -> R<()> {
        if self.func == 0 {
            match self.shader {
                Shader::Compute => self.line("return;"),
                Shader::Fragment if self.plan.out_members.is_empty() => self.line("return;"),
                s => {
                    if s == Shader::Vertex {
                        self.line("out.gl_Position.z = fma(out.gl_Position.z, 0.5, out.gl_Position.w * 0.5);");
                    }
                    self.line("return out;");
                }
            }
            return Ok(());
        }
        let ret = self.info.functions[self.func].return_type;
        if ret == AggType::VOID {
            self.line("return;");
        } else {
            self.line(&format!("return {};", literal(ret, 0)));
        }
        Ok(())
    }
}
