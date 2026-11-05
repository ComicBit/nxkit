//! `RegisterUsage`: `RunPass` (register inputs and outputs of a called function turned into arguments), the
//! `FunctionRegisterUsage` it returns and `FixupCalls` (the argument registers appended to every `Call`).
use super::Cfg;
use crate::ir::{Block, Inst, Ir, Node, NodeId, OperandType, StorageKind, regtype};

/// `FunctionRegisterUsage`: the registers passed in and out, as (index, register type).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FunctionRegisterUsage {
    pub in_arguments: Vec<(i32, i32)>,
    pub out_arguments: Vec<(i32, i32)>,
}

const GPR_MASKS: usize = 4;
const PRED_MASKS: usize = 1;
const FLAG_MASKS: usize = 1;
const TOTAL_MASKS: usize = GPR_MASKS + PRED_MASKS + FLAG_MASKS + 2;

/// `RegisterMask`: four GPR words, predicates, flags, uniform registers, uniform predicates.
type Mask = [u64; 8];

const NONE: Mask = [0; 8];

fn and(x: Mask, y: Mask) -> Mask {
    std::array::from_fn(|i| x[i] & y[i])
}

fn or(x: Mask, y: Mask) -> Mask {
    std::array::from_fn(|i| x[i] | y[i])
}

fn not(x: Mask) -> Mask {
    std::array::from_fn(|i| !x[i])
}

fn get_mask((index, ty): (i32, i32)) -> Mask {
    let mut m = NONE;
    let bit = |i: i32| 1u64 << (i & 0x3f);
    match ty {
        regtype::GPR => m[(index >> 6) as usize & 3] = bit(index),
        regtype::PREDICATE => m[4] = bit(index),
        regtype::FLAG => m[5] = bit(index),
        regtype::UNIFORM => m[6] = bit(index),
        regtype::UNIFORM_PREDICATE => m[7] = bit(index),
        _ => {}
    }
    m
}

fn exchange(masks: &mut [Mask], index: usize, value: Mask) -> bool {
    let old = masks[index];
    masks[index] = value;
    old != value
}

fn ends_with_return(ir: &Ir, b: &Block) -> bool {
    b.ops
        .last()
        .is_some_and(|n| matches!(&ir.nodes[n.0 as usize], Node::Op(o) if o.inst == Inst::Return))
}

fn reg_type_and_base(i: usize) -> (i32, i32) {
    if i < GPR_MASKS {
        (regtype::GPR, (i * 64) as i32)
    } else if i == GPR_MASKS {
        (regtype::PREDICATE, 0)
    } else if i == GPR_MASKS + PRED_MASKS {
        (regtype::FLAG, 0)
    } else if i == GPR_MASKS + PRED_MASKS + FLAG_MASKS {
        (regtype::UNIFORM, 0)
    } else {
        (regtype::UNIFORM_PREDICATE, 0)
    }
}

/// The registers of a mask in the C# order (mask word by word, lowest bit first).
fn registers_of(masks: &Mask) -> Vec<(i32, i32)> {
    let mut v = Vec::new();
    for (i, word) in masks.iter().enumerate().take(TOTAL_MASKS) {
        let (ty, base) = reg_type_and_base(i);
        let mut mask = *word;
        while mask != 0 {
            let bit = mask.trailing_zeros();
            mask &= !(1u64 << bit);
            v.push((base + bit as i32, ty));
        }
    }
    v
}

fn register_operand(ir: &Ir, n: NodeId) -> Vec<(i32, i32)> {
    let Node::Op(o) = &ir.nodes[n.0 as usize] else {
        return vec![];
    };
    o.sources
        .iter()
        .flatten()
        .filter(|s| ir.ty(**s) == OperandType::Register)
        .map(|s| ir.reg_of(*s))
        .collect()
}

/// `RegisterUsage.RunPass(cfg)`: inserts the context loads and stores as `Copy` operations from and to arguments.
pub fn run_pass(ir: &mut Ir, cfg: &mut Cfg) -> FunctionRegisterUsage {
    let count = cfg.blocks.len();
    let mut local_inputs = vec![NONE; count];
    let mut local_outputs = vec![NONE; count];
    for block in &cfg.blocks {
        let bi = block.index;
        for &n in &block.ops {
            for r in register_operand(ir, n) {
                local_inputs[bi] = or(local_inputs[bi], and(get_mask(r), not(local_outputs[bi])));
            }
            if let Node::Op(o) = &ir.nodes[n.0 as usize] {
                for d in o.dests.iter().flatten() {
                    if ir.ty(*d) == OperandType::Register {
                        local_outputs[bi] = or(local_outputs[bi], get_mask(ir.reg_of(*d)));
                    }
                }
            }
        }
    }

    let mut global_cmn_outputs = vec![NONE; count];
    let mut global_inputs = vec![NONE; count];
    let mut global_outputs = vec![NONE; count];
    let mut all_outputs = NONE;
    let mut all_cmn_outputs: Mask = [u64::MAX; 8];
    all_cmn_outputs[6] = 0;
    all_cmn_outputs[7] = 0;
    let mut first_pass = true;
    loop {
        let mut modified = false;
        for index in (0..cfg.post_order.len()).rev() {
            let bi = cfg.post_order[index];
            let block = &cfg.blocks[bi];
            if let Some((&p0, rest)) = block.preds.split_first() {
                let mut cmn_outputs = or(local_outputs[p0], global_cmn_outputs[p0]);
                let mut outputs = global_outputs[p0];
                for &p in rest {
                    cmn_outputs = and(cmn_outputs, or(local_outputs[p], global_cmn_outputs[p]));
                    outputs = or(outputs, global_outputs[p]);
                }
                global_inputs[bi] = or(global_inputs[bi], and(outputs, not(cmn_outputs)));
                if !first_pass {
                    cmn_outputs = and(cmn_outputs, global_cmn_outputs[bi]);
                }
                if ends_with_return(ir, block) {
                    all_cmn_outputs = and(all_cmn_outputs, or(cmn_outputs, local_outputs[bi]));
                }
                if exchange(&mut global_cmn_outputs, bi, cmn_outputs) {
                    modified = true;
                }
                outputs = or(outputs, local_outputs[bi]);
                let merged = or(global_outputs[bi], outputs);
                if exchange(&mut global_outputs, bi, merged) {
                    all_outputs = or(all_outputs, outputs);
                    modified = true;
                }
            } else if exchange(&mut global_outputs, bi, local_outputs[bi]) {
                all_outputs = or(all_outputs, local_outputs[bi]);
                modified = true;
            }
        }
        for index in 0..cfg.post_order.len() {
            let bi = cfg.post_order[index];
            let block = &cfg.blocks[bi];
            let mut inputs = local_inputs[bi];
            if let Some(nx) = block.next {
                inputs = or(inputs, global_inputs[nx]);
            }
            if let Some(br) = block.branch {
                inputs = or(inputs, global_inputs[br]);
            }
            inputs = and(inputs, not(global_cmn_outputs[bi]));
            let merged = or(global_inputs[bi], inputs);
            if exchange(&mut global_inputs, bi, merged) {
                modified = true;
            }
        }
        first_pass = false;
        if !modified {
            break;
        }
    }

    let mut in_arguments: Vec<(i32, i32)> = Vec::new();
    let mut out_arguments: Vec<(i32, i32)> = Vec::new();
    #[allow(clippy::needless_range_loop)]
    for bi in 0..count {
        if cfg.blocks[bi].preds.is_empty() {
            let inputs = or(global_inputs[bi], and(all_outputs, not(all_cmn_outputs)));
            load_locals(ir, &mut cfg.blocks[bi], &inputs, &mut in_arguments);
        }
        if ends_with_return(ir, &cfg.blocks[bi]) {
            store_locals(
                ir,
                &mut cfg.blocks[bi],
                &all_outputs,
                in_arguments.len(),
                &mut out_arguments,
            );
        }
    }
    FunctionRegisterUsage {
        in_arguments,
        out_arguments,
    }
}

fn load_locals(ir: &mut Ir, block: &mut Block, masks: &Mask, in_arguments: &mut Vec<(i32, i32)>) {
    let fill = in_arguments.is_empty();
    for (arg_index, reg) in registers_of(masks).into_iter().enumerate() {
        if fill {
            in_arguments.push(reg);
        }
        let dest = ir.register(reg.0, reg.1);
        let src = ir.argument(arg_index as i32);
        let copy = ir.new_op(Inst::Copy, StorageKind::None, Some(dest), &[src]);
        block.ops.insert(arg_index, copy);
    }
}

fn store_locals(
    ir: &mut Ir,
    block: &mut Block,
    masks: &Mask,
    in_arguments_count: usize,
    out_arguments: &mut Vec<(i32, i32)>,
) {
    let fill = out_arguments.is_empty();
    let last = block.ops.len() - 1;
    for (i, reg) in registers_of(masks).into_iter().enumerate() {
        if fill {
            out_arguments.push(reg);
        }
        let dest = ir.argument((in_arguments_count + i) as i32);
        let src = ir.register(reg.0, reg.1);
        let copy = ir.new_op(Inst::Copy, StorageKind::None, Some(dest), &[src]);
        block.ops.insert(last + i, copy);
    }
}

/// `RegisterUsage.FixupCalls(blocks, frus)`: appends the input registers to the sources of every `Call` and the output
/// registers (after one empty slot) to its destinations. `frus[0]` is the main function and is never called.
pub fn fixup_calls(
    ir: &mut Ir,
    blocks: &[Block],
    frus: &[FunctionRegisterUsage],
) -> Result<(), String> {
    for b in blocks {
        for &n in &b.ops {
            let Node::Op(o) = &ir.nodes[n.0 as usize] else {
                continue;
            };
            if o.inst != Inst::Call {
                continue;
            }
            let func_id = o
                .sources
                .first()
                .copied()
                .flatten()
                .ok_or("call without function id")?;
            if ir.ty(func_id) != OperandType::Constant {
                return Err("call with a non constant function id".into());
            }
            let fru = frus
                .get(ir.value(func_id) as usize)
                .ok_or("call to an unknown function")?;
            let ins: Vec<_> = fru
                .in_arguments
                .iter()
                .map(|r| ir.register(r.0, r.1))
                .collect();
            let mut outs = vec![None];
            outs.extend(
                fru.out_arguments
                    .iter()
                    .map(|r| Some(ir.register(r.0, r.1))),
            );
            ir.append_sources(n, &ins);
            ir.append_dests(n, &outs);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpr(ir: &mut Ir, i: i32) -> crate::ir::Opd {
        ir.register(i, regtype::GPR)
    }

    #[test]
    fn straight_line_function_gets_arguments() {
        let mut ir = Ir::default();
        let (r1, r2, r3) = (gpr(&mut ir, 1), gpr(&mut ir, 2), gpr(&mut ir, 3));
        let add = ir.new_op(Inst::Add, StorageKind::None, Some(r1), &[r2, r3]);
        let ret = ir.new_op(Inst::Return, StorageKind::None, None, &[]);
        let mut cfg = Cfg::create(&ir, &[add, ret]).expect("cfg");
        let fru = run_pass(&mut ir, &mut cfg);
        assert_eq!(fru.in_arguments, vec![(2, regtype::GPR), (3, regtype::GPR)]);
        assert_eq!(fru.out_arguments, vec![(1, regtype::GPR)]);
        let ops = &cfg.blocks[0].ops;
        assert_eq!(ops.len(), 5);
        let inst = |i: usize| ir.op(ops[i]).inst;
        assert_eq!(
            [inst(0), inst(1), inst(2), inst(3), inst(4)],
            [Inst::Copy, Inst::Copy, Inst::Add, Inst::Copy, Inst::Return]
        );
        // the loads copy arguments 0 and 1 into r2 and r3, the store copies r1 into argument 2
        assert_eq!(ir.op(ops[0]).sources[0].map(|s| ir.value(s)), Some(0));
        assert_eq!(ir.op(ops[1]).sources[0].map(|s| ir.value(s)), Some(1));
        assert_eq!(ir.op(ops[3]).dests[0].map(|s| ir.value(s)), Some(2));
    }

    #[test]
    fn register_written_on_one_path_is_also_an_input() {
        let mut ir = Ir::default();
        let p0 = ir.register(0, regtype::PREDICATE);
        let r4 = gpr(&mut ir, 4);
        let one = ir.const_(1);
        let label = ir.label();
        let br = ir.new_op(Inst::BranchIfFalse, StorageKind::None, Some(label), &[p0]);
        let cp = ir.new_op(Inst::Copy, StorageKind::None, Some(r4), &[one]);
        let mark = ir.new_op(Inst::MarkLabel, StorageKind::None, Some(label), &[]);
        let ret = ir.new_op(Inst::Return, StorageKind::None, None, &[]);
        let mut cfg = Cfg::create(&ir, &[br, cp, mark, ret]).expect("cfg");
        let fru = run_pass(&mut ir, &mut cfg);
        assert_eq!(
            fru.in_arguments,
            vec![(4, regtype::GPR), (0, regtype::PREDICATE)]
        );
        assert_eq!(fru.out_arguments, vec![(4, regtype::GPR)]);
    }

    #[test]
    fn fixup_appends_argument_registers_to_calls() {
        let mut ir = Ir::default();
        let id = ir.const_(1);
        let call = ir.new_op(Inst::Call, StorageKind::None, None, &[id]);
        let blocks = vec![Block {
            index: 0,
            ops: vec![call],
            ..Default::default()
        }];
        let frus = vec![
            FunctionRegisterUsage::default(),
            FunctionRegisterUsage {
                in_arguments: vec![(2, regtype::GPR)],
                out_arguments: vec![(1, regtype::GPR), (3, regtype::GPR)],
            },
        ];
        fixup_calls(&mut ir, &blocks, &frus).expect("fixup");
        let o = ir.op(call);
        assert_eq!(o.sources.len(), 2);
        assert_eq!(o.dests.len(), 3);
        assert!(o.dests[0].is_none());
        assert_eq!(o.dests[1].map(|d| ir.reg_of(d)), Some((1, regtype::GPR)));
        let missing = fixup_calls(&mut ir, &blocks, &frus[..1]);
        assert!(missing.is_err());
    }
}
