//! Intermediate representation (`IntermediateRepresentation/`): operands, operations and phi nodes in an arena.
//!
//! Identity matters in the C# (`Operand` objects, `AsgOp`, `UseOps`), so every operand is a handle into [`Ir::operands`].

pub mod dotnet_set;
#[rustfmt::skip]
pub mod inst;
pub mod block;
pub mod emitter;
#[rustfmt::skip]
pub mod emitter_gen_0;
#[rustfmt::skip]
pub mod emitter_gen_1;
pub mod emitter_insts;
pub mod emitter_ret;
pub mod ops;
pub mod text;

pub use block::Block;
pub use dotnet_set::DotnetSet;
pub use inst::{Inst, IoVariable, StorageKind};
pub use ops::*;
