#!/usr/bin/env python3
"""Rough GPU time of the libraries of Apple, own path A (air-opt -O1) and C (no optimiser), with A2's gpudiff:
each library runs against itself (`a = b`), so gpu_ms is twice its own time. Small random workloads: a coarse
signal only. usage: runtime_ac.py OUT [--per-set 8] [--rounds 3]"""
import collections, glob, json, os, random, statistics, subprocess, sys
sys.path.insert(0, (os.environ.get('NXB_HARNESS', 'tools/gpudiff'))); sys.dont_write_bytecode = True
import hlib
x = lambda t: subprocess.run(['xcrun', '-sdk', 'macosx', '-find', t], capture_output=True, text=True).stdout.strip()
SDK = subprocess.run(['xcrun', '-sdk', 'macosx', '--show-sdk-path'], capture_output=True, text=True).stdout.strip()
OPT, AS, LIB = x('air-opt'), x('air-as'), x('metallib'); AIRC = (os.environ.get('NXB_AIRC', 'target/release/airc'))
ENV = {'SDKROOT': SDK, 'PATH': '/usr/bin:/bin'}
a = sys.argv[1:]; out = a[0]; os.makedirs(out, exist_ok=True)
per = int(a[a.index('--per-set') + 1]) if '--per-set' in a else 8
rounds = int(a[a.index('--rounds') + 1]) if '--rounds' in a else 3
specs = sorted(glob.glob(((os.environ.get('NXB_SPECS') or sys.exit('set NXB_SPECS')) + '/*.spec.json')))
by = collections.defaultdict(list)
for s in specs: by[os.path.basename(s).split('_')[0]].append(s)
rnd = random.Random(5); chosen = []
for k in sorted(by): l = by[k][:]; rnd.shuffle(l); chosen += l[:per]
jobs = []
for s in chosen:
    sp = json.load(open(s)); k = os.path.basename(s)[:-10]
    libs = {'apple': sp['reference']}
    for tag, mid in (('A', '%s -O1 - -o - | %s --discard-value-names - -o -' % (OPT, OPT)), ('C', '%s - -o -' % AS)):
        lib = os.path.join(out, '%s.%s.metallib' % (k, tag))
        r = subprocess.run('%s emit %s | %s | %s - -o %s' % (AIRC, sp['source'], mid, LIB, lib), shell=True, env=ENV, capture_output=True)
        if r.returncode: break
        libs[tag] = lib
    if len(libs) < 3: continue
    for tag, lib in libs.items():
        jobs.append({'id': '%s|%s' % (k, tag), 'a': lib, 'b': lib, 'spec': s})
t = collections.defaultdict(list)
for r in range(rounds):
    res, hangs, _ = hlib.run_batch(jobs, os.path.join(out, 'run%d' % r), workers=1, extra=['--vectors', '4', '--no-nondet'])
    for j in jobs:
        g = res.get(j['id'], {}).get('gpu_ms')
        if g: t[j['id']].append(g)
ratios = collections.defaultdict(list)
for k in {j['id'].split('|')[0] for j in jobs}:
    m = {tag: statistics.median(t[k + '|' + tag]) for tag in ('apple', 'A', 'C') if t.get(k + '|' + tag)}
    if len(m) == 3 and m['apple'] > 0:
        ratios['A'].append(m['A'] / m['apple']); ratios['C'].append(m['C'] / m['apple'])
        ratios['set:' + k.split('_')[0]].append(m['C'] / m['apple'])
for k, v in sorted(ratios.items()):
    print('%-8s n=%d  median time ratio vs apple %.3f  (p10 %.3f, p90 %.3f)' % (k, len(v), statistics.median(v), sorted(v)[len(v) // 10], sorted(v)[len(v) * 9 // 10]))
