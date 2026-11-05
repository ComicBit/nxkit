#!/usr/bin/env python3
"""GPU machine code of the own paths A (air-opt -O1) and C (no optimiser) against Apple's, via A3's tt_compare
(metal-tt render pipeline, vertex _bp main0 + a fragment main0). usage: tt_ac.py N [seed]"""
import json, os, random, subprocess, sys, tempfile
sys.path.insert(0, ((os.environ.get('NXB_REF') or sys.exit('set NXB_REF')))); sys.dont_write_bytecode = True
import tt_compare as T
x = lambda t: subprocess.run(['xcrun', '-sdk', 'macosx', '-find', t], capture_output=True, text=True).stdout.strip()
SDK = subprocess.run(['xcrun', '-sdk', 'macosx', '--show-sdk-path'], capture_output=True, text=True).stdout.strip()
OPT, AS = x('air-opt'), x('air-as'); AIRC = (os.environ.get('NXB_AIRC', 'target/release/airc'))
ENV = {'SDKROOT': SDK, 'PATH': '/usr/bin:/bin'}
N = int(sys.argv[1]); rnd = random.Random(int(sys.argv[2]) if len(sys.argv) > 2 else 1)
L = open(((os.environ.get('NXB_CORPUS') or sys.exit('set NXB_CORPUS')))).read().split()
vs = [p for p in L if p.endswith('_bp.metal')]; fs = [p for p in L if p.endswith('_bu.metal')]
rnd.shuffle(vs); rnd.shuffle(fs)
d = tempfile.mkdtemp()

def own(src, tag, opt):
    out = os.path.join(d, tag + '.air')
    cmd = '%s emit %s | %s %s' % (AIRC, src, OPT if opt else AS, ('-O1 - -o - | %s --discard-value-names - -o %s' % (OPT, out)) if opt else ('- -o %s' % out))
    subprocess.run(cmd, shell=True, env=ENV, check=True, capture_output=True)
    return out

def text(res, stage):
    for k, v in res['sections'].items():
        if k.startswith(stage + ':') and k.endswith('__text'):
            return v['ref_bytes'], v['cand_bytes']
    return None, None

rows = []
for i in range(N * 3):
    if len(rows) >= N: break
    v, f = vs[i], fs[i]
    try:
        va, fa = own(v, 'va%d' % i, True), own(f, 'fa%d' % i, True)
        vc, fc = own(v, 'vc%d' % i, False), own(f, 'fc%d' % i, False)
        r1 = T.compare_pair('@' + v, va, '@' + f, fa)
        r2 = T.compare_pair('@' + v, vc, '@' + f, fc)
    except T.NotPossible as e:
        if i < 6: print('not possible', i, str(e)[:160])
        continue
    except Exception as e:
        print('skip', i, str(e)[:120]); continue
    (av, aa), (_, ac) = text(r1, 'vertex'), text(r2, 'vertex')
    (fv, fa_), (_, fc_) = text(r1, 'fragment'), text(r2, 'fragment')
    rows.append((av, aa, ac, fv, fa_, fc_, r1['code_equal'], r2['code_equal']))
    print('pair %d: vertex text apple %s A %s C %s | fragment apple %s A %s C %s | A==apple %s C==apple %s' % (len(rows), av, aa, ac, fv, fa_, fc_, r1['code_equal'], r2['code_equal']), flush=True)
if rows:
    s = lambda k: sum(r[k] or 0 for r in rows)
    print('total __text bytes: vertex apple %d A %d C %d; fragment apple %d A %d C %d; machine code identical to apple: A %d/%d, C %d/%d' % (
        s(0), s(1), s(2), s(3), s(4), s(5), sum(r[6] for r in rows), len(rows), sum(r[7] for r in rows), len(rows)))
