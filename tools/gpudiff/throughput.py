#!/usr/bin/env python3
"""Throughput on this Mac, cold: shaders the harness has not seen (no cached spec, no cached pipelines).

  1. prepare: augment (vertex shaders), compile the reference, gen_spec (msl_info + gpudiff reflect, which compiles the
     reference pipelines of the first function constant set: cold)
  2. candidate: the same source with a harmless volatile store at the top of every entry point, so that its AIR
     differs and Metal's pipeline cache cannot serve it (a candidate front end's output is never in the cache)
  3. gpudiff batch, 6 processes, default options (12 vectors)

    python3 -I throughput.py [--n 150] [--out work/throughput] [--seed 5]
"""
import os, re, sys, json, time, random, argparse, subprocess, collections, statistics
from concurrent.futures import ThreadPoolExecutor
sys.dont_write_bytecode = True
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import hlib, augment as AUG

ENTRY = re.compile(r'^((?:\[\[[^\n]*?\]\]\s*)?(?:vertex|fragment|kernel)\s+[^\n]*\)\s*\n\{)', re.M)


def perturb(src):
    """a volatile local store at the top of every entry point: no effect on any output, different AIR"""
    return ENTRY.sub(lambda m: m.group(1) + '\n    { volatile uint dm_tp_ = 0u; dm_tp_ = dm_tp_ + 1u; }', src)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--n', type=int, default=150)
    ap.add_argument('--out', default=os.path.join(HERE, 'work', 'throughput'))
    ap.add_argument('--seed', type=int, default=5)
    ap.add_argument('--workers', type=int, default=6)
    ap.add_argument('--corpus', default=os.path.join(HERE, 'work', 'corpus', 'corpus.jsonl'))
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    rnd = random.Random(a.seed)
    recs = hlib.load_corpus(a.corpus)
    seen = set()
    for d in ('selftest', 'st200'):
        p = os.path.join(HERE, 'work', d, 'prep')
        if os.path.isdir(p): seen |= {f.split('.')[0] for f in os.listdir(p)}
    # the corpus mix: the share of each set in the corpus
    pool = [r for r in recs if r['id'].replace(':', '_') not in seen]
    pick = rnd.sample(pool, a.n)
    print('picked', collections.Counter(r['set'] for r in pick))
    prep_t = {}
    def prep(r):
        t0 = time.time()
        try:
            src, lib, spec = hlib.prepare(r, os.path.join(a.out, 'prep'), force_compile=True)
            t1 = time.time()
            text = open(src).read()
            bsrc = os.path.join(a.out, 'prep', r['id'].replace(':', '_') + '.cand.metal')
            open(bsrc, 'w').write(perturb(text))
            blib = bsrc[:-6] + '.metallib'
            hlib.compile_msl(bsrc, blib)
            prep_t[r['id']] = (t1 - t0, time.time() - t1)
            return {'id': r['id'], 'a': lib, 'b': blib, 'spec': spec, 'meta': {'set': r['set']}}
        except Exception as e:
            print('prep failed', r['id'], str(e)[-200:])
            return None
    t0 = time.time()
    with ThreadPoolExecutor(8) as ex: jobs = [j for j in ex.map(prep, pick) if j]
    t_prep = time.time() - t0
    results, hangs, t_run = hlib.run_batch(jobs, os.path.join(a.out, 'run'), workers=a.workers)
    times = sorted(r.get('time_s', 0) for r in results.values())
    comp = sorted(r.get('compile_ms', 0) / 1000 for r in results.values())
    cases = sum(len(r.get('cases', [])) for r in results.values())
    q = lambda xs, p: xs[min(len(xs) - 1, int(p * len(xs)))] if xs else 0
    by_set = collections.defaultdict(list)
    for j in jobs: by_set[j['meta']['set']].append(results.get(j['id'], {}).get('time_s', 0))
    out = {'shaders': len(jobs), 'status': dict(collections.Counter(r.get('status') for r in results.values())), 'hangs': hangs,
           'prep_wall_s': t_prep, 'prep_per_shader_s_8_parallel': t_prep / max(1, len(jobs)),
           'prep_reference_s_median': statistics.median(v[0] for v in prep_t.values()),
           'candidate_compile_s_median': statistics.median(v[1] for v in prep_t.values()),
           'run_wall_s': t_run, 'run_shaders_per_s': len(jobs) / max(1e-3, t_run), 'cases': cases,
           'job_s': {'median': q(times, 0.5), 'p90': q(times, 0.9), 'p99': q(times, 0.99), 'max': times[-1] if times else 0},
           'job_pipeline_compile_s': {'median': q(comp, 0.5), 'p90': q(comp, 0.9), 'max': comp[-1] if comp else 0},
           'job_s_by_set_median': {k: statistics.median(v) for k, v in by_set.items()},
           'not_same': {k: {'status': v.get('status'), 'cases': [(c.get('entry'), c.get('status'), c.get('error', '')[:120]) for c in v.get('cases', []) if c.get('status') != 'same']}
                        for k, v in results.items() if v.get('status') != 'same'}}
    json.dump(out, open(os.path.join(a.out, 'throughput.json'), 'w'), indent=1)
    print(json.dumps(out, indent=1))


if __name__ == '__main__':
    main()
