#!/usr/bin/env python3
"""Instruction-level difference classes over a candidate dir (irequal.sh output): for every differing
function hunk, the opcode of the first removed and first added line. usage: irclasses.py DIR [N]"""
import collections, glob, os, re, subprocess, sys
d = sys.argv[1]; top = int(sys.argv[2]) if len(sys.argv) > 2 else 25
IRDIFF = ((os.environ.get('NXB_REF') or sys.exit('set NXB_REF')) + '/irdiff.py')
op = lambda l: (re.search(r'=\s+(\w+)', l) or re.search(r'^\s*[-+]\s+(\w+)', l) or re.search(r'(\w+)', l)).group(1)
c, files = collections.Counter(), collections.defaultdict(set)
for f in sorted(glob.glob(os.path.join(d, '*.ll'))):
    k = os.path.basename(f)[:-3]
    out = subprocess.run([sys.executable, IRDIFF, k, f, '--funcs', '50', '--lines', '400'], capture_output=True, text=True).stdout
    for h in out.split('@@')[1:]:
        lines = h.split('\n')[1:]
        a = next((l for l in lines if l.startswith('   -   ')), None)
        b = next((l for l in lines if l.startswith('   +   ')), None)
        key = '%s -> %s' % (op(a[7:]) if a else '.', op(b[7:]) if b else '.')
        if a and b and op(a[7:]) == op(b[7:]) == 'phi':
            key = 'phi order/operands'
        c[key] += 1; files[key].add(k)
for k, v in c.most_common(top):
    print('%6d hunks %4d files  %s' % (v, len(files[k]), k))
