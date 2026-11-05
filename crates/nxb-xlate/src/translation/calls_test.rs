//! A compute shader with a `CAL`: no C# reference output exists for it (no corpus shader calls a function), so this checks
//! the shape of the result: the callee is translated with its input and output registers as arguments and called.

use super::core::Translator;
use crate::driver::run_job;
use crate::job::parse_jobs;

fn words(prog: &[u64]) -> Vec<u8> {
    prog.iter().flat_map(|w| w.to_le_bytes()).collect()
}

fn spv_ops(spv: &[u8]) -> Vec<(u32, u32)> {
    let w: Vec<u32> = spv
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
        .collect();
    let mut out = vec![];
    let mut i = 5;
    while i < w.len() {
        let n = w[i] >> 16;
        out.push((w[i] & 0xffff, n));
        i += n.max(1) as usize;
    }
    out
}

const PRED7: u64 = 0x7 << 16;

fn mov32i(rd: u64, imm: u32) -> u64 {
    0x40u64 << 50 | rd | (imm as u64) << 20 | PRED7 | 0xf << 12
}

fn mov(rd: u64, rs: u64) -> u64 {
    0x1726u64 << 50 | rd | rs << 20 | PRED7 | 0xf << 39
}

fn cal(from: u64, to: u64) -> u64 {
    0x3898u64 << 50 | ((to - from - 8) & 0xffffff) << 20 | PRED7
}

const RET: u64 = 0x38c8u64 << 50 | 0x70000 | 0xf;

#[test]
fn called_function_is_translated_with_arguments() {
    let jobs = parse_jobs(r#"[{"key": "t", "stage": "cs", "local": [1, 1, 1]}]"#).expect("job");
    // 0: scheduling word, 8: MOV32I R1, 16: CAL 40 (the decoder ends the block: nothing after an unconditional CAL is
    // decoded), 32: scheduling word, 40: MOV R2, R1, 48: RET
    let code = words(&[
        0,
        mov32i(1, 0x3f80_0000),
        cal(16, 40),
        0,
        0,
        mov(2, 1),
        RET,
        0,
    ]);
    let o = run_job(&Translator, &jobs[0], &code).expect("translate");
    let ops = spv_ops(&o.spv);
    let count = |op: u32| ops.iter().filter(|(o, _)| *o == op).count();
    // OpFunction 54, OpFunctionCall 57
    // OpFunction 54 (fun0 is the entry point, fun1 the callee), OpFunctionCall 57: result type, id, function, two
    // arguments (the input register R1 and the output register R2)
    assert_eq!(count(54), 2);
    let calls: Vec<_> = ops.iter().filter(|(o, _)| *o == 57).collect();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].1, 6);
}
