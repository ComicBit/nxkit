#!/usr/bin/env python3
"""Generate crates/nxb-xlate/src/codegen/spirv_enums.rs from Spv.Generator/spirv.cs (enum values of the SPIR-V grammar)."""
import re, sys

src = open(sys.argv[1]).read()
out = ["//! Produced by tools/gen_spirv_enums.py from Spv.Generator/spirv.cs: do not edit.\n#![allow(non_upper_case_globals, dead_code)]\n"]
def snake(n):
    s = re.sub(r'(?<=[a-z0-9])(?=[A-Z])', '_', n)
    return s.lower()
SKIP = re.compile(r'SourceLanguage|Kernel|RayFlags|RayQuery|FragmentShadingRate|ImageChannel|FPFastMath|LinkageType|FunctionParameterAttribute|\w+Shift$')
which = sys.argv[2]
for m in re.finditer(r'public enum (\w+)\s*\{(.*?)\n        \}', src, re.S):
    name, body = m.group(1), m.group(2)
    group = 'op' if name == 'Op' else 'cap' if name == 'Capability' else 'enums'
    if group != which or SKIP.match(name):
        continue
    vals = {}
    items = []
    for line in body.split('\n'):
        line = line.split('//')[0].strip()
        mm = re.match(r'(\w+)\s*=\s*(.+?),$', line)
        if not mm:
            continue
        k, e = mm.group(1), mm.group(2)
        e = re.sub(r'0x[0-9a-fA-F]+u?', lambda x: x.group(0).rstrip('u'), e)
        e2 = re.sub(r'[A-Za-z_]\w*', lambda x: str(vals[x.group(0)]) if x.group(0) in vals else x.group(0), e)
        v = eval(e2)
        vals[k] = v
        items.append((k, v))
    out.append(f"pub mod {snake(name)} {{")
    for k, v in items:
        k2 = k if not k[0].isdigit() else '_' + k
        out.append(f"    pub const {k2}: u32 = {v & 0xffffffff};")
    out.append("}\n")
print('\n'.join(out))
