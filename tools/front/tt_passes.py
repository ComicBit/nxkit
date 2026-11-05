#!/usr/bin/env python3
"""Stand-in for an own optimiser: own IR -> air-opt -passes=P --discard-value-names -> metallib, against Apple's
library: library bytes, GPU machine code (metal-tt, A3's tt_compare, vertex _bp main0 + fragment _bu main0) and CPU
time of air-opt and metallib. usage: tt_passes.py N PASSES|O1 [seed]"""
import os, random, resource, subprocess, sys, tempfile, time
sys.path.insert(0, ((os.environ.get('NXB_REF') or sys.exit('set NXB_REF')))); sys.dont_write_bytecode = True
import tt_compare as T
x = lambda t: subprocess.run(['xcrun', '-sdk', 'macosx', '-find', t], capture_output=True, text=True).stdout.strip()
SDK = subprocess.run(['xcrun', '-sdk', 'macosx', '--show-sdk-path'], capture_output=True, text=True).stdout.strip()
OPT, LIB = x('air-opt'), x('metallib'); AIRC = (os.environ.get('NXB_AIRC', 'target/release/airc'))
ENV = {'SDKROOT': SDK, 'PATH': '/usr/bin:/bin'}
N, P = int(sys.argv[1]), sys.argv[2]; rnd = random.Random(int(sys.argv[3]) if len(sys.argv) > 3 else 1)
L = open(((os.environ.get('NXB_CORPUS') or sys.exit('set NXB_CORPUS')))).read().split()
vs = [p for p in L if p.endswith('_bp.metal')]; fs = [p for p in L if p.endswith('_bu.metal')]
rnd.shuffle(vs); rnd.shuffle(fs)
d = tempfile.mkdtemp(); cpu = {'opt': 0.0, 'lib': 0.0}; nlib = 0

def run(cmd, k):
    a = resource.getrusage(resource.RUSAGE_CHILDREN)
    subprocess.run(cmd, env=ENV, check=True, capture_output=True)
    b = resource.getrusage(resource.RUSAGE_CHILDREN)
    cpu[k] += (b.ru_utime - a.ru_utime) + (b.ru_stime - a.ru_stime)

def own(src, tag):
    bc, air, lib = (os.path.join(d, tag + e) for e in ('.bc', '.air', '.metallib'))
    subprocess.run([AIRC, 'bc', src, '-o', bc], check=True, capture_output=True)
    run([OPT, '--discard-value-names'] + (['-O1'] if P == 'O1' else ['-passes=' + P]) + [bc, '-o', air], 'opt')
    run([LIB, air, '-o', lib], 'lib')
    return air, lib

def ref_lib(src):
    import hashlib, json
    k = hashlib.sha256(open(src, 'rb').read()).hexdigest()
    return json.load(open(os.path.join(T.STORE, 'mac', k[:2], k, 'meta.json'))).get('lib_sha_ref')

import hashlib
same_lib = code_same = pairs = 0
for i in range(N * 4):
    if pairs >= N: break
    v, f = vs[i], fs[i]
    try:
        (va, vl), (fa, fl) = own(v, 'v%d' % i), own(f, 'f%d' % i); nlib += 2
        same_lib += sum(hashlib.sha256(open(l, 'rb').read()).hexdigest() == ref_lib(s) for l, s in ((vl, v), (fl, f)))
        r = T.compare_pair('@' + v, va, '@' + f, fa)
    except T.NotPossible:
        continue
    except Exception as e:
        print('skip', i, str(e)[:120]); continue
    pairs += 1; code_same += bool(r['code_equal'])
print('%s: %d pairs: machine code identical %d; libraries byte-identical %d/%d; CPU per library: air-opt %.1f ms, metallib %.1f ms'
      % (P, pairs, code_same, same_lib, nlib, cpu['opt'] * 1000 / nlib, cpu['lib'] * 1000 / nlib))
