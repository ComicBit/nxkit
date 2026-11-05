#!/usr/bin/env python3
"""Generate crates/nxb-maxwell/src/{enums,insts_N,table}.rs from the C# decoder sources.
Usage: gen_xlate_decoders.py <Decoders dir> <out dir>   (dev tool, output is committed)"""
import re, sys, os

src, out = sys.argv[1], sys.argv[2]
text = open(os.path.join(src, 'InstDecoders.cs')).read()
os.makedirs(os.path.join(out, 'insts'), exist_ok=True)

def snake(n):
    s = re.sub(r'(?<=[a-z0-9])(?=[A-Z])', '_', n)
    s = re.sub(r'(?<=[A-Z])(?=[A-Z][a-z])', '_', s)
    return s.lower()

def balanced(s):
    """strip one pair of outer parens if they match"""
    s = s.strip()
    if s.startswith('(') and s.endswith(')'):
        d = 0
        for i, c in enumerate(s):
            d += c == '('
            d -= c == ')'
            if d == 0 and i < len(s) - 1:
                return s
        return s[1:-1].strip()
    return s

def split_or(s):
    parts, d, cur = [], 0, ''
    i = 0
    while i < len(s):
        c = s[i]
        d += c == '('
        d -= c == ')'
        if d == 0 and s[i:i+3] == ' | ':
            parts.append(cur); cur = ''; i += 3; continue
        cur += c; i += 1
    parts.append(cur)
    return parts

def u64(e):
    e = e.strip().replace('_opcode', 'self.0')
    return e

def to_int(e):
    """C# expression giving an int: (int)(...) possibly or-ed"""
    parts = []
    for p in split_or(e):
        p = p.strip()
        assert p.startswith('(int)'), p
        parts.append('((%s) as i32)' % u64(balanced(p[5:])))
    return ' | '.join(parts)

fields_re = re.compile(r'public (\w+) (\w+) => (.*);')
structs = []
cur = None
enums = []
cur_enum = None
for line in text.split('\n'):
    m = re.match(r'\s+enum (\w+)', line)
    if m:
        cur_enum = (m.group(1), []); enums.append(cur_enum); continue
    m = re.match(r'\s+(\w+) = (0x[0-9A-Fa-f]+|-?\d+),?', line)
    if m and cur_enum is not None and cur is None:
        cur_enum[1].append((m.group(1), m.group(2))); continue
    m = re.match(r'\s+readonly struct (\w+)', line)
    if m:
        cur = (m.group(1), []); structs.append(cur); cur_enum = None; continue
    m = fields_re.match(line.strip())
    if m and cur is not None:
        cur[1].append(m.groups())

enum_names = {e[0] for e in enums}
HDR = '//! Produced by tools/gen_xlate_decoders.py from InstDecoders.cs: do not edit.\n#![allow(non_upper_case_globals, non_camel_case_types, dead_code, unused_parens, clippy::all)]\n\n'

ef = None; en = 0; el = 0; ecount = 0
os.makedirs(os.path.join(out, 'enums'), exist_ok=True)
for n, ms in enums:
    if ef is None or el > 450:
        if ef: ef.close()
        ef = open(os.path.join(out, 'enums', 'enums_%d.rs' % ecount), 'w'); ecount += 1; el = 0
        ef.write(HDR)
    ef.write('#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\npub struct %s(pub i32);\nimpl %s {\n' % (n, n))
    for mn, v in ms:
        ef.write('    pub const %s: %s = %s(%s);\n' % (mn, n, n, v))
    ef.write('}\n')
    el += len(ms) + 4
ef.close()
with open(os.path.join(out, 'enums.rs'), 'w') as f:
    f.write(HDR)
    for i in range(ecount):
        f.write('mod enums_%d;\npub use enums_%d::*;\n' % (i, i))

def conv(t, e):
    e = e.strip()
    if e == '!P && Imm11 == 0 && SrcA != RegisterConsts.RegisterZeroIndex':
        return '!self.p() && self.imm11() == 0 && self.src_a() != 255'
    if t == 'bool':
        m = re.match(r'\(_opcode & (0x[0-9A-F]+)\) != 0', e)
        assert m, e
        return '(self.0 & %s) != 0' % m.group(1)
    if t == 'int':
        return to_int(e)
    assert t in enum_names, t
    m = re.match(r'\((\w+)\)(.*)$', e)
    assert m and m.group(1) == t, e
    inner = balanced(m.group(2))
    if '(int)' in inner:
        return '%s(%s)' % (t, to_int(inner))
    return '%s((%s) as i32)' % (t, u64(inner))

files = 0
lines = 0
f = None
for name, fl in structs:
    if f is None or lines > 480:
        if f: f.close()
        f = open(os.path.join(out, 'insts', 'insts_%d.rs' % files), 'w'); files += 1; lines = 0
        f.write(HDR + 'use crate::maxwell::enums::*;\n\n')
    f.write('#[derive(Clone, Copy, Debug)]\npub struct %s(pub u64);\nimpl %s {\n' % (name, name))
    seen = set()
    for t, n, e in fl:
        rn = snake(n)
        if rn in seen: continue
        seen.add(rn)
        rt = {'bool': 'bool', 'int': 'i32'}.get(t, t)
        f.write('    pub fn %s(&self) -> %s {\n        %s\n    }\n' % (rn, rt, conv(t, e)))
    f.write('}\n\n')
    lines += 3 * len(fl) + 4
f.close()
with open(os.path.join(out, 'insts.rs'), 'w') as f:
    f.write(HDR)
    for i in range(files):
        f.write('mod insts_%d;\npub use insts_%d::*;\n' % (i, i))

# InstName and table
nm = open(os.path.join(src, 'InstName.cs')).read()
names = re.findall(r'^\s+(\w+)(?: = \d+)?,', nm, re.M)
tbl = open(os.path.join(src, 'InstTable.cs')).read()
rows = re.findall(r'Add\("([01x]+)",\s*InstName\.(\w+),\s*InstEmit\.(\w+)(?:,\s*([^)]*))?\);', tbl)
emits = []
for r in rows:
    if r[2] not in emits: emits.append(r[2])
props = ['Rd','Rd2','Ra','Rb','Rb2','Ib','Rc','Pd','LPd','SPd','TPd','VPd','Pdn','Ps','Tex','TexB','Bra','NoPred']
with open(os.path.join(out, 'names.rs'), 'w') as f:
    f.write(HDR)
    f.write('#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\npub enum InstName {\n    Invalid,\n')
    for n in names:
        if n != 'Invalid': f.write('    %s,\n' % n)
    f.write('}\n\n#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\npub enum Emit {\n')
    for e in emits: f.write('    %s,\n' % e)
    f.write('}\n')
with open(os.path.join(out, 'table_gen.rs'), 'w') as f:
    f.write(HDR + 'use super::names::{Emit, InstName};\n\n')
    f.write('/// (encoding, name, emitter, props bits)\npub const ROWS: &[(&str, InstName, Emit, u16)] = &[\n')
    for enc, n, e, p in rows:
        v = 0
        for tok in [t.strip() for t in (p or '').split('|') if t.strip()]:
            k = tok.replace('InstProps.', '')
            if k == 'None': continue
            bit = {'Rd':1,'Rd2':2,'Ra':4,'Rb':8,'Rb2':16,'Ib':32,'Rc':64,'Pd':1<<7,'LPd':2<<7,'SPd':3<<7,'TPd':4<<7,'VPd':5<<7,'Pdn':1<<10,'Ps':1<<11,'Tex':1<<12,'TexB':1<<13,'Bra':1<<14,'NoPred':1<<15}[k]
            v |= bit
        f.write('    ("%s", InstName::%s, Emit::%s, %d),\n' % (enc[:14], n, e, v))
    f.write('];\n')
print(len(enums), 'enums', len(structs), 'structs', files, 'files', len(rows), 'rows', len(emits), 'emitters')
