#!/usr/bin/env python3
"""Byte equality of the final library: own IR -> air-opt -O1 -> air-opt --discard-value-names -> metallib,
against the sha256 of Apple's library in A3's store (meta.json lib_sha_ref). usage: libequal.py LIST OUT [-j 4]"""
import hashlib, json, os, subprocess, sys
from concurrent.futures import ThreadPoolExecutor

AIRC = os.environ.get('AIRC', 'target/release/airc')
STORE = ((os.environ.get('NXB_REF') or sys.exit('set NXB_REF')) + '/store/mac')
x = lambda t: subprocess.run(['xcrun', '-sdk', 'macosx', '-find', t], capture_output=True, text=True).stdout.strip()
SDK = subprocess.run(['xcrun', '-sdk', 'macosx', '--show-sdk-path'], capture_output=True, text=True).stdout.strip()
OPT, LIB = x('air-opt'), x('metallib')
ENV = {'SDKROOT': SDK, 'PATH': '/usr/bin:/bin'}


def one(arg):
    i, src, out = arg
    key = hashlib.sha256(open(src, 'rb').read()).hexdigest()
    meta = os.path.join(STORE, key[:2], key, 'meta.json')
    if not os.path.exists(meta):
        return src, 'no-ref'
    ref = json.load(open(meta)).get('lib_sha_ref')
    p = lambda e: os.path.join(out, '%d.%s' % (i, e))
    steps = [[AIRC, 'emit', src, '-o', p('ll')], [OPT, '-O1', p('ll'), '-o', p('o1')],
             [OPT, '--discard-value-names', p('o1'), '-o', p('air')], [LIB, p('air'), '-o', p('lib')]]
    for s in steps:
        if subprocess.run(s, capture_output=True, env=ENV).returncode:
            return src, 'fail:' + os.path.basename(s[0])
    got = hashlib.sha256(open(p('lib'), 'rb').read()).hexdigest()
    for e in ('ll', 'o1', 'air', 'lib'):
        os.unlink(p(e))
    return src, 'equal' if got == ref else 'different'


def main():
    lst, out = sys.argv[1], sys.argv[2]
    j = int(sys.argv[sys.argv.index('-j') + 1]) if '-j' in sys.argv else 4
    os.makedirs(out, exist_ok=True)
    srcs = [l.strip() for l in open(lst) if l.strip()]
    with ThreadPoolExecutor(j) as ex:
        res = list(ex.map(one, [(i, s, out) for i, s in enumerate(srcs)]))
    from collections import Counter
    c = Counter(r for _, r in res)
    print('libraries byte-identical to Apple: %d / %d' % (c['equal'], len(res)), dict(c))
    json.dump(res, open(os.path.join(out, 'result.json'), 'w'), indent=0)


main()
