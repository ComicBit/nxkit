#!/usr/bin/env python3
"""metal-tt acceptance of raw libraries (airc raw, no Apple tool) and GPU machine code size against Apple's, via A3's
tt_compare: pairs of a vertex function (_bp main0, or _b main0) and a fragment function (_bu/_bf/_b main0), raw
library vs Apple's reference AIR for the same sources. usage: tt_raw.py N_PAIRS [seed] [--platform macos|ios]"""
import os, random, subprocess, sys, tempfile
sys.path.insert(0, ((os.environ.get('NXB_REF') or sys.exit('set NXB_REF')))); sys.dont_write_bytecode = True
import tt_compare as T
AIRC = (os.environ.get('NXB_AIRC', 'target/release/airc'))
N = int(sys.argv[1]); rnd = random.Random(int(sys.argv[2]) if len(sys.argv) > 2 and sys.argv[2].isdigit() else 5)
plat = sys.argv[sys.argv.index('--platform') + 1] if '--platform' in sys.argv else 'macos'
L = open(((os.environ.get('NXB_CORPUS') or sys.exit('set NXB_CORPUS')))).read().split()
vs = [p for p in L if p.endswith('_bp.metal')]
fs = [p for p in L if p.endswith(('_bu.metal', '_bf.metal')) or ('/variant/' in p)]
rnd.shuffle(vs); rnd.shuffle(fs)
d = tempfile.mkdtemp()
acc = rej = np_ = 0; vt = [0, 0]; ft = [0, 0]; code_same = 0; errs = {}

def raw(src, tag):
    out = os.path.join(d, tag + '.metallib')
    r = subprocess.run([AIRC, 'raw', src, '-o', out] + (['--ios'] if plat == 'ios' else []), capture_output=True)
    if r.returncode:
        raise RuntimeError('raw build failed')
    return out

def text(res, stage):
    for k, v in res['sections'].items():
        if k.startswith(stage + ':') and k.endswith('__text'):
            return v['ref_bytes'] or 0, v['cand_bytes'] or 0
    return 0, 0

i = 0
while acc + rej < N and i < N * 6:
    v, f = vs[i % len(vs)], fs[i % len(fs)]; i += 1
    try:
        r = T.compare_pair('@' + v, raw(v, 'v%d' % i), '@' + f, raw(f, 'f%d' % i),
                           arch='applegpu_g14s' if plat == 'macos' else 'applegpu_g16p', osmin='27.0' if plat == 'macos' else '26.0', platform=plat)
    except T.NotPossible:
        np_ += 1; continue
    except Exception as e:  # noqa: BLE001
        m = str(e)
        if '(ref)' in m:
            np_ += 1; continue           # Apple's own pair does not build (mismatched varyings): no verdict
        rej += 1; errs[m[:120]] = errs.get(m[:120], 0) + 1; continue
    acc += 1; code_same += bool(r['code_equal'])
    a, b = text(r, 'vertex'); vt[0] += a; vt[1] += b
    a, b = text(r, 'fragment'); ft[0] += a; ft[1] += b
print('%s: metal-tt accepted %d pairs, rejected %d (not possible %d); machine code identical %d; __text bytes vertex apple %d raw %d (%.3fx), fragment apple %d raw %d (%.3fx)'
      % (plat, acc, rej, np_, code_same, vt[0], vt[1], vt[1] / max(vt[0], 1), ft[0], ft[1], ft[1] / max(ft[0], 1)))
for k, c in sorted(errs.items(), key=lambda x: -x[1])[:5]:
    print('  %d x %s' % (c, k))
