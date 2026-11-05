//! Basic blocks of a function (`BasicBlock`): node lists by handle, successor and predecessor links by block index.

use super::ops::NodeId;

/// A basic block. `preds` keeps the C# order (it decides the phi source order).
#[derive(Clone, Debug, Default)]
pub struct Block {
    pub index: usize,
    pub ops: Vec<NodeId>,
    pub next: Option<usize>,
    pub branch: Option<usize>,
    pub preds: Vec<usize>,
}
