#!/usr/bin/env python3
"""gpudiff of raw libraries built with and without the Rust mem2reg/DCE for given sources (specs from gputier's
cache). usage: gpuraw_ab.py WORK SRC..."""
import json, os, subprocess, sys
sys.dont_write_bytecode = True
sys.path.insert(0, (os.environ.get('NXB_HARNESS', 'tools/gpudiff'))); import hlib
W = sys.argv[1]; AIRC = (os.environ.get('NXB_AIRC', 'target/release/airc'))
corpus = {r['src']: r for r in hlib.load_corpus((os.environ.get('NXB_HARNESS', 'tools/gpudiff') + '/work/corpus/corpus.jsonl'))}
jobs = []
for p in sys.argv[2:]:
    rec = corpus[p]; key = rec['id'].replace(':', '_')
    src, a, spec = hlib.prepare(rec, os.path.join(W, 'prep'))
    for tag, env in (('opt', {}), ('noopt', {'AIR_FRONT_NO_OPT': '1'})):
        lib = os.path.join(W, 'ab', key + '.' + tag + '.metallib'); os.makedirs(os.path.dirname(lib), exist_ok=True)
        subprocess.run([AIRC, 'raw', src, '-o', lib], env=dict(os.environ, **env), check=True)
        jobs.append({'id': key + '|' + tag, 'a': a, 'b': lib, 'spec': spec})
res, hangs, _ = hlib.run_batch(jobs, os.path.join(W, 'ab', 'run'), workers=4, extra=['--vectors', '12'], log=lambda *a: None)
for j in jobs:
    r = res.get(j['id'], {}); cs = r.get('cases', [])
    bad = [(c.get('entry'), c.get('fc'), c.get('diff_words')) for c in cs if c.get('diff_words')]
    print(j['id'], r.get('status'), bad[:4])
