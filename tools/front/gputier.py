#!/usr/bin/env python3
"""GPU tier for own-validated shaders: gpudiff (A2's harness) of the own library against Apple's, resumable.

GPUTIER_RAW=1: the raw container path (airc raw, no Apple tool) for every source that lowers, instead of the
own-validated ones through Apple's back half. GPUTIER_WORK: work dir.

usage: gputier.py run RESULTS.jsonl CHECK.jsonl [--chunk 300] [--chunks N] [--workers 6] [--families bu,bf,b,bp,bv]
       gputier.py tiers RESULTS.jsonl CHECK.jsonl OUT_DIR

run: takes the `own-validated` sources of an `airc check --opt --ref-store` result, family by family in the given
order (largest first), skips sources already in RESULTS (keyed by the source's sha256), and per chunk of --chunk
sources: takes $NXB_LOCK (mkdir, retry every 2 minutes), prepares the harness specs (reference
library + spec, hlib.prepare, cached in the work dir), builds the own library from the source the spec uses (vertex
shaders get capture twins) with airc bc | air-opt -O1 --discard-value-names | metallib, runs gpudiff batch, appends
one line per source to RESULTS and releases the lock. A chunk that would hold the lock past 14 minutes stops early.

Per source: status identical (every case same) | diff (words differ outside the nondeterminism mask; diff_words,
max_ulp over the reported words, vertex = a vertex-stage case differs) | nondet (no difference, but some words are
masked because the reference differs from itself) | notbuilt (own path failed on the spec's source) | error.

tiers: allow-identical (verdict own-identical), gpu-verified (status identical, so 0 differing words in every output
including vertex positions, and no masked words), rejected (everything else), with their share of the corpus.
Everything stays local (the corpus derives from a game)."""
import collections, json, os, subprocess, sys, time
from concurrent.futures import ThreadPoolExecutor

sys.dont_write_bytecode = True
HARNESS = (os.environ.get('NXB_HARNESS', 'tools/gpudiff'))
sys.path.insert(0, HARNESS)
import hlib  # noqa: E402

LOCK = (os.environ.get('NXB_LOCK', '.nxb-bench-lock'))
WORK = os.environ.get('GPUTIER_WORK', '')
AIRC = (os.environ.get('NXB_AIRC', 'target/release/airc'))
x = lambda t: subprocess.run(['xcrun', '-sdk', 'macosx', '-find', t], capture_output=True, text=True).stdout.strip()
SDK = subprocess.run(['xcrun', '-sdk', 'macosx', '--show-sdk-path'], capture_output=True, text=True).stdout.strip()
OPT, LIB = x('air-opt'), x('metallib')
ENV = {'SDKROOT': SDK, 'PATH': '/usr/bin:/bin'}
HOLD = 14 * 60


def lock():
    while True:
        try:
            os.mkdir(LOCK)
            return time.time()
        except FileExistsError:
            time.sleep(120)


def unlock():
    try:
        os.rmdir(LOCK)
    except OSError:
        pass


RAW = os.environ.get('GPUTIER_RAW') == '1'


def build_own(src, key):
    if RAW:
        lib = os.path.join(WORK, 'own', key + '.raw.metallib')
        r = subprocess.run([AIRC, 'raw', src, '-o', lib], capture_output=True, text=True)
        return (None, 'raw: ' + r.stderr.strip()[:200]) if r.returncode else (lib, None)
    bc, air, lib = (os.path.join(WORK, 'own', key + e) for e in ('.bc', '.air', '.metallib'))
    for cmd in ([AIRC, 'bc', src, '-o', bc], [OPT, '-O1', '--discard-value-names', bc, '-o', air], [LIB, air, '-o', lib]):
        r = subprocess.run(cmd, capture_output=True, text=True, env=ENV)
        if r.returncode:
            return None, os.path.basename(cmd[0]) + ': ' + (r.stderr or r.stdout).strip()[:200]
    for f in (bc, air):
        os.unlink(f)
    return lib, None


def summarize(r):
    cases = r.get('cases', [])
    st = r.get('status', 'error')
    words = sum(c.get('diff_words', 0) for c in cases)
    nondet = sum(c.get('nondet_words', 0) for c in cases)
    ulps = [d.get('ulp') for c in cases for f in c.get('first', []) for d in f.get('diffs', []) if d.get('ulp') is not None]
    vertex = any(c.get('mode') == 'vertex' and c.get('diff_words', 0) for c in cases)
    if st == 'same' and nondet == 0:
        s = 'identical'
    elif words:
        s = 'diff'
    elif st in ('same', 'nondet'):
        s = 'nondet'
    else:
        s = st if st in ('hang', 'mismatch') else 'error'
    return {'status': s, 'diff_words': words, 'max_ulp': max(ulps) if ulps else None, 'vertex': vertex,
            'nondet_words': nondet, 'cases': len(cases), 'raw_status': st, 'error': (r.get('error') or '')[:200]}


def run(args):
    res_path, check = args[0], args[1]
    opt = lambda k, d: args[args.index(k) + 1] if k in args else d
    chunk, chunks, workers = int(opt('--chunk', '300')), int(opt('--chunks', '1000')), int(opt('--workers', '6'))
    fams = opt('--families', 'bu,bf,b,bp,bv').split(',')
    corpus = {r['src']: r for r in hlib.load_corpus(os.path.join(HARNESS, 'work/corpus/corpus.jsonl'))}
    done = set()
    if os.path.exists(res_path):
        done = {json.loads(l)['sha'] for l in open(res_path)}
    todo = collections.defaultdict(list)
    for l in open(check):
        c = json.loads(l)
        wanted = c['verdict'] != 'apple' if RAW else c['verdict'] == 'own-validated'
        if wanted and c['source_sha256'] not in done and c['path'] in corpus:
            todo[corpus[c['path']]['set']].append(c)
    if RAW:
        # family coverage first: round robin over the families
        import itertools
        queue = [c for grp in itertools.zip_longest(*[todo.get(f, []) for f in fams]) for c in grp if c]
    else:
        queue = [c for f in fams for c in todo.get(f, [])]
    print('to do: %d (%s)' % (len(queue), {f: len(todo.get(f, [])) for f in fams}), flush=True)
    os.makedirs(os.path.join(WORK, 'own'), exist_ok=True)
    for n in range(chunks):
        part, queue = queue[:chunk], queue[chunk:]
        if not part:
            break
        t0 = lock()
        try:
            prepared, out = [], []
            def prep(c):
                rec = corpus[c['path']]
                try:
                    src, a, spec = hlib.prepare(rec, os.path.join(WORK, 'prep'))
                except Exception as e:  # noqa: BLE001
                    return c, None, str(e)[:200]
                lib, err = build_own(src, rec['id'].replace(':', '_'))
                if err:
                    return c, None, err
                return c, {'id': rec['id'].replace(':', '_'), 'a': a, 'b': lib, 'spec': spec}, None
            with ThreadPoolExecutor(workers) as ex:
                for c, job, err in ex.map(prep, part):
                    if time.time() - t0 > HOLD / 2 and not job:
                        pass
                    if job:
                        prepared.append((c, job))
                    else:
                        out.append({'sha': c['source_sha256'], 'path': c['path'], 'family': corpus[c['path']]['set'],
                                    'status': 'notbuilt', 'error': err})
            res, hangs, _ = hlib.run_batch([j for _, j in prepared], os.path.join(WORK, 'run', 'c%d' % n), workers=workers,
                                           extra=['--vectors', '12'], log=lambda *a: None)
            for c, j in prepared:
                s = summarize(res.get(j['id'], {}))
                s.update({'sha': c['source_sha256'], 'path': c['path'], 'family': corpus[c['path']]['set']})
                out.append(s)
            with open(res_path, 'a') as f:
                for o in out:
                    f.write(json.dumps(o) + '\n')
            cnt = collections.Counter(o['status'] for o in out)
            print('chunk %d: %d sources in %.0f s, hangs %s: %s; left %d' % (n, len(part), time.time() - t0, hangs, dict(cnt), len(queue)), flush=True)
        finally:
            unlock()
        time.sleep(20)  # let the others take the lock


def tiers(args):
    res_path, check, out = args
    os.makedirs(out, exist_ok=True)
    R = {}
    for l in open(res_path):
        r = json.loads(l)
        R[r['sha']] = r
    C = [json.loads(l) for l in open(check)]
    total = len({c['source_sha256'] for c in C})
    allow = sorted({c['source_sha256'] for c in C if c['verdict'] == 'own-identical'})
    gpu = sorted({c['source_sha256'] for c in C if c['verdict'] == 'own-validated' and R.get(c['source_sha256'], {}).get('status') == 'identical'})
    tested = sorted({c['source_sha256'] for c in C if c['source_sha256'] in R})
    rejected = sorted({c['source_sha256'] for c in C if c['verdict'] == 'own-validated' and c['source_sha256'] in R and R[c['source_sha256']]['status'] != 'identical'})
    for name, lst in (('allow-identical', allow), ('gpu-verified', gpu), ('rejected', rejected)):
        with open(os.path.join(out, name + '.txt'), 'w') as f:
            f.write('# %s: %d of %d sources\n' % (name, len(lst), total))
            for s in lst:
                f.write(s + '\n')
    st = collections.Counter(R[s]['status'] for s in tested)
    fam = collections.defaultdict(collections.Counter)
    for s in tested:
        fam[R[s]['family']][R[s]['status']] += 1
    ulp = collections.Counter()
    for s in tested:
        r = R[s]
        if r['status'] == 'diff':
            m = r.get('max_ulp')
            ulp['<=4 ulp' if m is not None and m <= 4 else '<=256 ulp' if m is not None and m <= 256 else 'larger'] += 1
    vert = sum(1 for s in tested if R[s].get('vertex'))
    print('corpus %d: allow-identical %d (%.1f %%), gpu-verified %d (%.1f %%), together %.1f %%; rejected %d; tested %d of the own-validated'
          % (total, len(allow), 100 * len(allow) / total, len(gpu), 100 * len(gpu) / total, 100 * (len(allow) + len(gpu)) / total, len(rejected), len(tested)))
    print('status of tested:', dict(st))
    print('per family:', {k: dict(v) for k, v in sorted(fam.items())})
    print('diff max ULP (reported words):', dict(ulp), '; vertex-stage diffs:', vert)


if __name__ == '__main__':
    {'run': run, 'tiers': tiers}[sys.argv[1]](sys.argv[2:])
