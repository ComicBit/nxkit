#!/usr/bin/env python3
"""GPU differential test of the own front end with A2's harness (gpudiff).

For prepared specs of the harness (reference library A + spec, made by hlib.prepare), builds the
candidate B from the same source with `airc emit` -> air-as -> metallib, then runs `gpudiff batch`
through hlib.run_batch (which survives GPU hangs) and prints a summary.

usage: gpudiff_own.py OUT_DIR [--per-set N] [--workers 4] [--prep DIR] [--seed S]
Everything stays local (the corpus derives from a game)."""
import collections, glob, json, os, random, subprocess, sys, time

HARNESS = (os.environ.get('NXB_HARNESS', 'tools/gpudiff'))
sys.path.insert(0, HARNESS)
sys.dont_write_bytecode = True
import hlib  # noqa: E402

AIRC = os.environ.get('AIRC', 'target/release/airc')


def xfind(t):
    return subprocess.run(['xcrun', '-sdk', 'macosx', '-find', t], capture_output=True, text=True).stdout.strip()


SDK = subprocess.run(['xcrun', '-sdk', 'macosx', '--show-sdk-path'], capture_output=True, text=True).stdout.strip()
AIR_AS, METALLIB, AIR_OPT = xfind('air-as'), xfind('metallib'), xfind('air-opt')
# OWN_OPT=1: the A3 path, own IR -> air-opt -O1 -> air-opt --discard-value-names -> metallib
OPT = os.environ.get('OWN_OPT') == '1'
# OWN_PASSES='<new-PM pipeline>': air-opt -passes=... --discard-value-names instead of -O1 (stand-in for an own optimiser)
PASSES = os.environ.get('OWN_PASSES')
# OWN_PIPE=1: the build pipes of variant C (air-as | metallib), identical files to OWN_OPT unset
ENV = {'SDKROOT': SDK, 'PATH': '/usr/bin:/bin'}


def build_b(src, out_dir, key):
    ll = os.path.join(out_dir, key + '.ll')
    air = os.path.join(out_dir, key + '.air')
    lib = os.path.join(out_dir, key + '.own.metallib')
    r = subprocess.run([AIRC, 'emit', src, '-o', ll], capture_output=True, text=True)
    if r.returncode:
        return None, 'front: ' + r.stderr.strip()[:200]
    steps = [([AIR_AS, ll, '-o', air], 'air-as'), ([METALLIB, air, '-o', lib], 'metallib')]
    if OPT:
        bc = air + '.o1'
        steps = [([AIR_OPT, '-O1', ll, '-o', bc], 'air-opt'), ([AIR_OPT, '--discard-value-names', bc, '-o', air], 'discard'),
                 ([METALLIB, air, '-o', lib], 'metallib')]
    if PASSES:
        steps = [([AIR_OPT, '--discard-value-names', '-passes=' + PASSES, ll, '-o', air], 'air-opt'), ([METALLIB, air, '-o', lib], 'metallib')]
    for cmd, stage in steps:
        r = subprocess.run(cmd, capture_output=True, text=True, env=ENV)
        if r.returncode:
            return None, stage + ': ' + (r.stderr or r.stdout).strip()[:300]
    for f in (air, air + '.o1'):
        if os.path.exists(f):
            os.unlink(f)
    return lib, None


def main():
    args = sys.argv[1:]
    out = args[0]
    opt = lambda k, d: args[args.index(k) + 1] if k in args else d
    per_set = int(opt('--per-set', '20'))
    workers = int(opt('--workers', '4'))
    prep = opt('--prep', os.path.join(HARNESS, 'work/st200/prep'))
    seed = int(opt('--seed', '1'))
    os.makedirs(out, exist_ok=True)
    specs = sorted(glob.glob(os.path.join(prep, '*.spec.json')))
    by_set = collections.defaultdict(list)
    for s in specs:
        by_set[os.path.basename(s).split('_')[0]].append(s)
    rnd = random.Random(seed)
    chosen = []
    for k in sorted(by_set):
        lst = by_set[k][:]
        rnd.shuffle(lst)
        chosen += lst[:per_set]
    jobs, fails = [], collections.Counter()
    t0 = time.time()
    for s in chosen:
        sp = json.load(open(s))
        key = os.path.basename(s)[:-len('.spec.json')]
        lib, err = build_b(sp['source'], out, key)
        if err:
            fails[err.split(':')[0] + ': ' + err.split(':', 1)[1][:90]] += 1
            continue
        jobs.append({'id': key, 'a': sp['reference'], 'b': lib, 'spec': s})
    print('built %d candidates in %.0f s, %d not built' % (len(jobs), time.time() - t0, sum(fails.values())))
    for k, v in fails.most_common(20):
        print('  %4d %s' % (v, k))
    res, hangs, _ = hlib.run_batch(jobs, os.path.join(out, 'run'), workers=workers, extra=['--vectors', '12'])
    print('hangs:', hangs)
    st = collections.Counter()
    per_set_st = collections.defaultdict(collections.Counter)
    diffs = []
    for j in jobs:
        r = res.get(j['id'], {})
        s = r.get('status', 'missing')
        st[s] += 1
        per_set_st[j['id'].split('_')[0]][s] += 1
        if s not in ('same',):
            diffs.append((j['id'], s, r))
    print('status:', dict(st))
    for k in sorted(per_set_st):
        print('  %s %s' % (k, dict(per_set_st[k])))
    with open(os.path.join(out, 'summary.json'), 'w') as f:
        json.dump({'status': st, 'per_set': per_set_st, 'not_built': fails,
                   'diffs': [{'id': i, 'status': s, 'result': r} for i, s, r in diffs]}, f, indent=1, default=dict)
    for i, s, r in diffs[:15]:
        first = ''
        for c in r.get('cases', []):
            if c.get('status') not in ('same', None):
                first = json.dumps({k: c.get(k) for k in ('entry', 'status', 'error', 'diffs')})[:400]
                break
        print('  DIFF %s %s %s' % (i, s, first or r.get('error', '')[:300]))


if __name__ == '__main__':
    main()
