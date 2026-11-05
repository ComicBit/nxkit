//! Numbered metadata nodes with structural deduplication.

use std::collections::HashMap;
use std::fmt::Write;

use crate::ir::escape;

/// One operand of a metadata tuple.
#[derive(Clone, Debug)]
pub enum Md {
    /// `!"text"`
    Str(String),
    /// `i32 n`
    I32(i64),
    /// A reference to another node.
    Node(usize),
    /// Any other operand spelled verbatim (`i8 addrspace(2)* @g`, `void ()* @f`).
    Raw(String),
}

/// A string operand.
pub fn s(text: &str) -> Md {
    Md::Str(text.to_string())
}

/// An `i32` operand.
pub fn i(v: impl Into<i64>) -> Md {
    Md::I32(v.into())
}

/// The metadata of a module.
#[derive(Debug, Default)]
pub struct Meta {
    nodes: Vec<String>,
    index: HashMap<String, usize>,
}

impl Meta {
    /// Adds (or finds) the tuple of the operands, returns its number.
    pub fn node(&mut self, ops: &[Md]) -> usize {
        let body = self.node_text(ops);
        if let Some(n) = self.index.get(&body) {
            return *n;
        }
        self.nodes.push(body.clone());
        let n = self.nodes.len() - 1;
        self.index.insert(body, n);
        n
    }

    fn node_text(&self, ops: &[Md]) -> String {
        let mut body = String::from("!{");
        for (k, op) in ops.iter().enumerate() {
            if k > 0 {
                body.push_str(", ");
            }
            match op {
                Md::Str(t) => {
                    let _ = write!(body, "!\"{}\"", escape(t));
                }
                Md::I32(v) => {
                    let _ = write!(body, "i32 {v}");
                }
                Md::Node(n) => {
                    let _ = write!(body, "!{n}");
                }
                Md::Raw(r) => body.push_str(r),
            }
        }
        body.push('}');
        body
    }

    /// A new distinct node whose first operand is itself (loop metadata).
    pub fn distinct_self(&mut self, ops: &[Md]) -> usize {
        let n = self.nodes.len();
        let mut all = vec![Md::Node(n)];
        all.extend(ops.iter().cloned());
        let body = self.node_text(&all);
        self.nodes.push(format!("distinct {body}"));
        n
    }

    /// `!N = !{...}` lines.
    pub fn render(&self, out: &mut String) {
        for (n, body) in self.nodes.iter().enumerate() {
            let _ = writeln!(out, "!{n} = {body}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_and_render() {
        let mut m = Meta::default();
        let a = m.node(&[s("air.buffer"), i(3)]);
        let b = m.node(&[s("air.buffer"), i(3)]);
        let c = m.node(&[Md::Node(a), Md::Raw("i8 0".into())]);
        assert_eq!(a, b);
        let mut out = String::new();
        m.render(&mut out);
        assert_eq!(out, "!0 = !{!\"air.buffer\", i32 3}\n!1 = !{!0, i8 0}\n");
        assert_eq!(c, 1);
    }
}
