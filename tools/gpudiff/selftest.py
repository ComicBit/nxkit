#!/usr/bin/env python3
"""Self-test A = B: every selected shader is compared with itself (the same metallib loaded twice, separate pipelines).
Any difference is a harness defect (non-deterministic input, uninitialised output, ...). Selection: at least N shaders
for every entry kind of the corpus (all of them where the corpus has fewer).

    python3 -I selftest.py [--per-kind 200] [--out work/selftest] [--workers 6] [--jobs 10] [--seed 1]
"""
import os, sys, json, time, random, hashlib, argparse, collections
from concurrent.futures import ThreadPoolExecutor
sys.dont_write_bytecode = True
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import hlib


def select(recs, per_kind, seed):
    rnd = random.Random(seed)
    seen, uniq = set(), []
    for r in recs:
        h = hashlib.sha1(open(r['src'], 'rb').read()).hexdigest()
        if h in seen: continue
        seen.add(h); uniq.append(r)
    rnd.shuffle(uniq)
    keys = lambda r: ['%s %s' % (r['set'], e) for e in r['entries']]      # every entry kind of every corpus set
    have = collections.Counter(k for r in uniq for k in keys(r))
    need = {k: min(per_kind, v) for k, v in have.items()}
    got, chosen = collections.Counter(), []
    for r in sorted(uniq, key=lambda r: min(have[k] for k in keys(r)) if r['entries'] else 1e9):   # rare kinds first
        if any(got[k] < need[k] for k in keys(r)):
            chosen.append(r)
            for k in keys(r): got[k] += 1
    return chosen, need, got


def summarize(chosen, results):
    by_id = {r['id']: r for r in chosen}
    kinds = collections.defaultdict(lambda: collections.Counter())
    sets = collections.defaultdict(lambda: collections.Counter())
    for jid, res in results.items():
        rec = by_id.get(jid)
        if not rec: continue
        sets[rec['set']][res.get('status', 'error')] += 1
        kind_of = {e.split()[1]: e for e in rec['entries']}
        seen = set()
        for c in res.get('cases', []):
            k = kind_of.get(c.get('entry'), 'vertex ' + c.get('entry', '?') if c.get('entry', '').endswith('_hcap') else c.get('entry'))
            st = kinds[k]
            st['cases'] += 1
            st['case_' + c.get('status', 'error')] += 1
            if c.get('written_words', 0) > 0: st['cases_writing'] += 1
            if k not in seen:
                seen.add(k); st['shaders'] += 1
        if res.get('status') not in ('same',) and not res.get('cases'):
            for e in rec['entries']: kinds[e]['shader_' + res.get('status', 'error')] += 1
    return {k: dict(v) for k, v in sorted(kinds.items())}, {k: dict(v) for k, v in sets.items()}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--per-kind', type=int, default=200)
    ap.add_argument('--out', default=os.path.join(HERE, 'work', 'selftest'))
    ap.add_argument('--workers', type=int, default=6)
    ap.add_argument('--jobs', type=int, default=10)
    ap.add_argument('--seed', type=int, default=1)
    ap.add_argument('--corpus', default=os.path.join(HERE, 'work', 'corpus', 'corpus.jsonl'))
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    recs = hlib.load_corpus(a.corpus)
    chosen, need, got = select(recs, a.per_kind, a.seed)
    print('selected %d shaders; per kind: %s' % (len(chosen), dict(got)))
    t0 = time.time()
    prep_err = {}
    jobs = []
    def prep(r):
        try:
            src, lib, spec = hlib.prepare(r, os.path.join(a.out, 'prep'))
            return {'id': r['id'], 'a': lib, 'b': lib, 'spec': spec, 'meta': {'set': r['set']}}
        except Exception as e:
            prep_err[r['id']] = str(e)[-400:]
            return None
    with ThreadPoolExecutor(a.jobs) as ex:
        for j in ex.map(prep, chosen):
            if j: jobs.append(j)
    t_prep = time.time() - t0
    print('prepared %d jobs in %.0f s, %d failed' % (len(jobs), t_prep, len(prep_err)))
    results, hangs, t_run = hlib.run_batch(jobs, os.path.join(a.out, 'run'), workers=a.workers)
    kinds, sets = summarize(chosen, results)
    counts = collections.Counter(r.get('status') for r in results.values())
    out = {'shaders': len(chosen), 'jobs': len(jobs), 'prep_failed': prep_err, 'prep_seconds': t_prep, 'run_seconds': t_run,
           'shaders_per_second_run': len(jobs) / max(t_run, 1e-3), 'status': dict(counts), 'hangs': hangs, 'by_kind': kinds, 'by_set': sets,
           'not_same': {k: {'status': v.get('status'), 'error': v.get('error'),
                            'cases': [c for c in v.get('cases', []) if c.get('status') != 'same']} for k, v in results.items() if v.get('status') != 'same'}}
    json.dump(out, open(os.path.join(a.out, 'summary.json'), 'w'), indent=1)
    print(json.dumps({k: out[k] for k in ('shaders', 'jobs', 'prep_seconds', 'run_seconds', 'shaders_per_second_run', 'status', 'hangs')}, indent=1))
    print('by kind:')
    for k, v in kinds.items(): print('  %-24s %s' % (k, v))
    print('by set:', json.dumps(sets))


if __name__ == '__main__':
    main()
