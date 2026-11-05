#!/usr/bin/env python3
"""Generate crates/nxb-xlate/src/ir/emitter_gen.rs from Translation/EmitterContextInsts.cs (the one-line wrappers).
Usage: gen_xlate_emitter_insts.py <EmitterContextInsts.cs> <out file>
Methods with a body longer than one `return context.Add(...)` are written by hand in ir/emitter_insts.rs.
Overloads get the number of operands as a suffix; a default `Instruction fpType = Instruction.FP32` gives `name` (FP32) and `name_t` (explicit)."""
import re, sys, collections
src, out = sys.argv[1], sys.argv[2]
s = open(src).read()
ms = re.split(r'\n        public static ', s)[1:]

def snake(n):
    n = re.sub(r'(?<=[a-z0-9])(?=[A-Z])', '_', n)
    n = re.sub(r'(?<=[A-Z])(?=[A-Z][a-z])', '_', n)
    return n.lower()

def split_top(a):
    r, d, cur = [], 0, ''
    for c in a:
        if c in '([': d += 1
        if c in ')]': d -= 1
        if c == ',' and d == 0:
            r.append(cur.strip()); cur = ''
        else:
            cur += c
    if cur.strip(): r.append(cur.strip())
    return r

meths = []
for m in ms:
    head = m.split('\n')[0]
    body = m[len(head):]
    lines = [l.strip() for l in body.strip().split('\n') if l.strip() not in ('{', '}', '')]
    if not (len(lines) == 1 and lines[0].startswith('return context.Add(')):
        continue
    if head.startswith('('):
        continue
    mm = re.match(r'(\w+) (\w+)\((.*)\)\s*$', head)
    ret, name, params = mm.group(1), mm.group(2), split_top(mm.group(3))
    params = params[1:]
    meths.append((ret, name, params, lines[0][len('return context.Add('):-2]))

counts = collections.Counter(m[1] for m in meths)
o = ['//! Produced by tools/gen_xlate_emitter_insts.py from EmitterContextInsts.cs: do not edit.\n',
     '#![allow(clippy::too_many_arguments, clippy::needless_lifetimes)]\n',
     'use super::emitter::{EmitterContext, FP32};\nuse super::inst::{Inst, StorageKind};\nuse super::ops::Opd;\n\n',
     'impl EmitterContext<\'_, \'_> {\n']
for ret, name, params, call in meths:
    ps = []
    deffp = False
    for p in params:
        if '=' in p:
            deffp = True
            continue
        t, n = p.rsplit(' ', 1)
        t = t.replace('params ', '')
        rt = {'Operand': 'Opd', 'Operand[]': '&[Opd]', 'StorageKind': 'StorageKind', 'int': 'i32', 'bool': 'bool', 'IoVariable': 'IoVariable'}[t]
        ps.append((snake(n), rt, n))
    suffix = ''
    if counts[name] > 1:
        suffix = '_%s%d' % ('b' if any(p[1] == 'i32' for p in ps) else ('v' if any(p[1] == 'IoVariable' for p in ps) else 'o'), len([1 for p in ps if p[1] in ('Opd', '&[Opd]')]))
    args = split_top(call)
    inst = args[0]
    rest = args[1:]
    kind = 'StorageKind::None'
    if rest and rest[0] == 'storageKind':
        kind = 'storage_kind'; rest = rest[1:]
    dest = None
    if rest:
        dest = rest[0]; rest = rest[1:]
    def mk(variants):
        pre = []
        srcs = []
        tmp = 0
        for a in rest:
            mc = re.match(r'Const\((.*)\)$', a)
            mf = re.match(r'ConstF\((.*)\)$', a)
            if mc:
                v = mc.group(1)
                pre.append('let t%d = self.ir.const_(%s);' % (tmp, snake(v) if re.match(r'^[a-z]\w*$', v) else v)); srcs.append('t%d' % tmp); tmp += 1
            elif mf:
                v = mf.group(1)
                pre.append('let t%d = self.ir.const_f(%s);' % (tmp, v if '.' in v else v + '.0')); srcs.append('t%d' % tmp); tmp += 1
            else:
                srcs.append(snake(a))
        return pre, srcs
    pre, srcs = mk(None)
    isvec = [p for p in ps if p[1] == '&[Opd]']
    for variant in ([False, True] if deffp else [False]):
        fname = snake(name) + suffix + ('_t' if variant else '')
        sig = ', '.join('%s: %s' % (n, t) for n, t, _ in ps)
        if deffp and variant:
            sig += (', ' if sig else '') + 'fp_type: Inst'
        retty = {'Operand': 'Option<Opd>', 'void': '()'}.get(ret, 'Option<Opd>')
        instx = re.sub(r'Instruction\.(\w+)', r'Inst::\1', inst)
        instx = instx.replace('Inst::FP32', 'FP32').replace('Inst::FP64', 'FP64')
        instx = instx.replace('fpType', 'fp_type')
        if deffp and not variant:
            instx = instx.replace('fp_type', 'Inst(Inst::FP32)')
        lines = []
        for l in pre: lines.append('        ' + l)
        if dest == 'Local()':
            lines.append('        let dest = self.ir.local();'); dx = 'Some(dest)'
        elif dest is None or dest == 'null':
            dx = 'None'
        else:
            dx = 'Some(%s)' % snake(dest)
        if isvec:
            srcexpr = '&[%s]' % ', '.join(srcs)
            # params Operand[]: appended after the fixed operands
            srcexpr = '&[%s].iter().copied().chain(%s.iter().copied()).collect::<Vec<_>>()' % (', '.join(x for x in srcs if x != isvec[0][0]), isvec[0][0]) if srcs and srcs[-1] == isvec[0][0] else '&%s' % isvec[0][0]
        else:
            srcexpr = '&[%s]' % ', '.join(srcs)
        lines.append('        self.add_op(%s, %s, %s, %s);' % (instx, kind, dx, srcexpr))
        if retty == 'Option<Opd>':
            lines.append('        ' + ('Some(dest)' if dest == 'Local()' else 'None'))
        if retty == 'Option<Opd>' and dest != 'Local()':
            # operations without a result still returned the (null) dest in C#
            pass
        rt = 'Opd' if dest == 'Local()' else '()'
        if rt == '()':
            lines = [l for l in lines if l.strip() not in ('None',)]
        else:
            lines[-1] = '        dest'
        o.append('    pub fn %s(&mut self%s)%s {\n%s\n    }\n\n' % (fname, ', ' + sig if sig else '', '' if rt == '()' else ' -> ' + rt, '\n'.join(lines)))
o.append('}\n')
head = o[:3]
body = o[4:-1]
half = (len(body) + 1) // 2
for n, part in enumerate((body[:half], body[half:])):
    open(out.replace('.rs', '_%d.rs' % n), 'w').write(''.join(head) + "impl EmitterContext<'_, '_> {\n" + ''.join(part) + '}\n')
