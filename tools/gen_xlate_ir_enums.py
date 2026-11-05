#!/usr/bin/env python3
"""Generate crates/nxb-xlate/src/ir/inst.rs (Instruction, IoVariable, StorageKind) from the C# IR sources.
Usage: gen_xlate_ir_enums.py <IntermediateRepresentation dir> <out file>"""
import re, sys, os
src, out = sys.argv[1], sys.argv[2]

def members(path, enum):
    t = open(os.path.join(src, path)).read()
    m = re.search(r'enum %s\s*\{(.*?)\n    \}' % enum, t, re.S)
    res, nxt = [], 0
    for line in m.group(1).split('\n'):
        line = line.split('//')[0].strip().rstrip(',')
        if not line: continue
        if '=' in line:
            n, v = [x.strip() for x in line.split('=')]
            if n in ('FP32', 'FP64', 'Mask'): continue
            nxt = int(v, 0)
        else:
            n = line
        res.append((n, nxt)); nxt += 1
    return res

with open(out, 'w') as f:
    f.write('//! Produced by tools/gen_xlate_ir_enums.py: do not edit.\n#![allow(non_upper_case_globals, dead_code)]\n\n')
    for name, path in [('Instruction', 'Instruction.cs'), ('IoVariable', 'IoVariable.cs'), ('StorageKind', 'StorageKind.cs')]:
        rn = 'Inst' if name == 'Instruction' else name
        f.write('/// `%s` (a number, as the C# flags enum).\n#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]\npub struct %s(pub u32);\nimpl %s {\n' % (name, rn, rn))
        for n, v in members(path, name):
            f.write('    pub const %s: %s = %s(%d);\n' % (n, rn, rn, v))
        if name == 'Instruction':
            f.write('    pub const FP32: u32 = 1 << 16;\n    pub const FP64: u32 = 1 << 17;\n    pub const MASK: u32 = 0xffff;\n')
        f.write('}\n\n')
