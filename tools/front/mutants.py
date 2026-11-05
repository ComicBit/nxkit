#!/usr/bin/env python3
"""Rebuilds the mutated sources of the prototype's strictness runs (mut300.json, mut500.json) and checks that the prototype
still gives the recorded verdict for each.

mutate.py records path, mutation name and verdicts, not the mutated text. A mutant is fully determined by its path and its
seed: the seed is the job index i (one((path, i)) uses random.Random(i)), and results keep job order with failed jobs
(None) dropped. So the k-th record's seed is the first i after the previous record's seed whose mutation of that path
has the recorded name and, where metal reported an error, has metal's quoted source line at the reported line number.

The verdict is not used to find the seed: mut300.json was recorded with an earlier revision of msl_parse.py, and some of
its verdicts differ from what the current prototype says. expected.jsonl holds both: `ok`/`reasons` from the current
prototype (what the Rust port must reproduce) and `recorded_ok`/`recorded_reasons` from the file.

usage: mutants.py <mutNNN.json> <out dir>
Writes <out dir>/<k>.metal and <out dir>/expected.jsonl (path of the mutant, recorded parser verdict and reasons, the
mutation name and the original path)."""
import os, sys, re, json, random
PROTO = os.environ.get('MSL_PROTO', '')
sys.path.insert(0, PROTO)
sys.setrecursionlimit(20000)
import mutate as MU


def mutant(path, seed):
    rnd = random.Random(seed)
    src = open(path).read()
    for _ in range(20):
        name, pat, rep = rnd.choice(MU.MUT)
        ms = list(re.finditer(pat, src))
        if not ms: continue
        m = rnd.choice(ms)
        return name, src[:m.start()] + m.expand(rep) + src[m.end():]
    return None, None


def err_line(err):
    """(line number, column, quoted source line or None) from metal's first diagnostic, or None. Clang leaves out the
    quote for very long lines: then the next line of the output is another diagnostic or the error count."""
    m = re.match(r'<stdin>:(\d+):(\d+): [^\n]*\n([^\n]*)', err or '')
    if not m: return None
    quote = m.group(3)
    if quote.startswith('<stdin>:') or re.match(r'\d+ errors? generated', quote): quote = None
    return int(m.group(1)), int(m.group(2)), quote


def matches_err(src, el):
    if el is None: return True
    line, col, quote = el
    lines = src.split('\n')
    if line > len(lines): return False
    if quote is None: return len(lines[line - 1]) + 1 >= col
    # mutate.py keeps only 300 characters of metal's output, so the quoted line may be cut short
    return lines[line - 1].startswith(quote.rstrip())


def judge(src):   # mutate.judge without the metal half
    p = MU.M.Parser(src)
    try:
        p.run(); acc = not p.reasons
    except Exception:
        acc = False
    return acc, (dict(p.reasons) if not acc else {})


if __name__ == '__main__':
    recs = json.load(open(sys.argv[1]))
    out = sys.argv[2]; os.makedirs(out, exist_ok=True)
    seed, bad, changed = -1, 0, 0
    with open(os.path.join(out, 'expected.jsonl'), 'w') as ex:
        for k, r in enumerate(recs):
            for s in range(seed + 1, seed + 50):
                name, src = mutant(r['path'], s)
                if name != r['mut']: continue
                if matches_err(src, err_line(r.get('err'))):
                    break
            else:
                print('not reproduced:', k, r['path'], r['mut']); bad += 1; continue
            seed = s
            mp = os.path.join(out, f'{k}.metal')
            with open(mp, 'w', newline='\n') as f: f.write(src)
            ok, why = judge(src)
            if (ok, why) != (r['parser'], r['why']):
                changed += 1
                print(f'verdict changed since recording: {k} {r["mut"]} recorded {r["parser"]} {r["why"]} now {ok} {why}')
            ex.write(json.dumps({'path': mp, 'ok': ok, 'reasons': why, 'recorded_ok': r['parser'], 'recorded_reasons': r['why'],
                                 'metal_ok': r['metal'], 'mut': r['mut'], 'orig': r['path'], 'seed': s}) + '\n')
    print(f'{len(recs) - bad}/{len(recs)} mutants reproduced, {changed} verdicts of the current prototype differ from the recorded ones')
