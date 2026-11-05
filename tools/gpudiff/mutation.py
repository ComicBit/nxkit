#!/usr/bin/env python3
"""Mutation test: one small semantic change in a copy of the MSL source (swapped operands, changed constant, flipped
comparison, other swizzle, ...), compiled with the engine's chain as candidate B, compared with the unmodified
compile as reference A. A mutant whose optimized AIR is identical to the original's is reported as equivalent
(dead code or folded away) and kept out of the detection rate: no test on the binary could see it.

    python3 -I mutation.py [--n 60] [--out work/mutation] [--seed 7] [--vectors 3]
"""
import os, re, sys, json, time, random, hashlib, argparse, subprocess, collections
from concurrent.futures import ThreadPoolExecutor
sys.dont_write_bytecode = True
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import hlib, augment as AUG

BODY = re.compile(r'^\s{4,}\S.*$', re.M)


def fmt_float(x):
    r = repr(float(x))
    return r if ('.' in r or 'e' in r or 'inf' in r or 'nan' in r) else r + '.0'


def m_const_float(line, rnd):
    ms = [m for m in re.finditer(r'(?<![\w.])(\d+\.\d+(?:e[-+]?\d+)?)(?![\w.])', line)]
    if not ms: return None
    m = rnd.choice(ms); v = float(m.group(1))
    nv = v * 1.25 + 0.0625 if v != 0 else 0.0625
    return line[:m.start()] + fmt_float(nv) + line[m.end():]

def m_swap_operands(line, rnd):
    ms = list(re.finditer(r'(\b_\d+(?:\.[xyzw]+)?) ([-/]) (_\d+(?:\.[xyzw]+)?)\b', line))
    if not ms: return None
    m = rnd.choice(ms)
    return line[:m.start()] + '%s %s %s' % (m.group(3), m.group(2), m.group(1)) + line[m.end():]

def m_op_replace(line, rnd):
    ms = list(re.finditer(r'(?<=[\w)\]]) ([-+*]) (?=[\w(])', line))
    if not ms: return None
    m = rnd.choice(ms)
    op = {'+': '-', '-': '+', '*': '+'}[m.group(1)]
    return line[:m.start()] + ' %s ' % op + line[m.end():]

def m_cmp_boundary(line, rnd):
    ms = list(re.finditer(r' (<|>|<=|>=) ', line))
    if not ms: return None
    m = rnd.choice(ms)
    op = {'<': '<=', '<=': '<', '>': '>=', '>=': '>'}[m.group(1)]
    return line[:m.start()] + ' %s ' % op + line[m.end():]

def m_cmp_invert(line, rnd):
    ms = list(re.finditer(r' (<|>|<=|>=|==|!=) ', line))
    if not ms: return None
    m = rnd.choice(ms)
    op = {'<': '>=', '<=': '>', '>': '<=', '>=': '<', '==': '!=', '!=': '=='}[m.group(1)]
    return line[:m.start()] + ' %s ' % op + line[m.end():]

def m_swizzle(line, rnd):
    if '=' not in line: return None
    lhs, rhs = line.split('=', 1)
    ms = list(re.finditer(r'(?<=[\w\])])\.([xyzw])\b(?!\s*\()', rhs))
    if not ms: return None
    m = rnd.choice(ms)
    c = rnd.choice([x for x in 'xyzw' if x != m.group(1)])
    return lhs + '=' + rhs[:m.start()] + '.' + c + rhs[m.end():]

def m_fma_swap(line, rnd):
    ms = list(re.finditer(r'fma\(([^,()]+), ([^,()]+), ([^,()]+)\)', line))
    if not ms: return None
    m = rnd.choice(ms)
    return line[:m.start()] + 'fma(%s, %s, %s)' % (m.group(1), m.group(3), m.group(2)) + line[m.end():]

def m_index(line, rnd):
    ms = list(re.finditer(r'(_m\d+)\[(\d+)\]', line))
    if not ms: return None
    m = rnd.choice(ms)
    return line[:m.start()] + '%s[%d]' % (m.group(1), int(m.group(2)) + 1) + line[m.end():]

def m_cond_negate(line, rnd):
    m = re.search(r'\bif \((!?)(_\d+)\)', line)
    if not m: return None
    return line[:m.start()] + 'if (%s%s)' % ('' if m.group(1) else '!', m.group(2)) + line[m.end():]

def m_minmax(line, rnd):
    ms = list(re.finditer(r'\b(precise::)?(min|max)\(', line))
    if not ms: return None
    m = rnd.choice(ms)
    return line[:m.start()] + (m.group(1) or '') + ('max(' if m.group(2) == 'min' else 'min(') + line[m.end():]

def m_int_const(line, rnd):
    ms = list(re.finditer(r'(<<|>>|&) (\d+)\b', line)) + list(re.finditer(r'(?<![\w.])(\d+)u\b', line))
    if not ms: return None
    m = rnd.choice(ms)
    if m.re.pattern.startswith('(<<'):
        v = int(m.group(2)); nv = v + 1 if m.group(1) != '&' else (v ^ 1 if v else 1)
        return line[:m.start()] + '%s %d' % (m.group(1), nv) + line[m.end():]
    v = int(m.group(1))
    return line[:m.start()] + '%du' % (v + 1) + line[m.end():]

def m_select_swap(line, rnd):
    ms = list(re.finditer(r'select\(([^,()]+), ([^,()]+), ', line))
    if not ms: return None
    m = rnd.choice(ms)
    return line[:m.start()] + 'select(%s, %s, ' % (m.group(2), m.group(1)) + line[m.end():]

def m_drop_output(line, rnd):
    m = re.match(r'^(\s*)(out\.\w+(?:\.[xyzw])?) = (.+);\s*$', line)
    if not m or m.group(3).strip() in ('0.0', '0', '0u'): return None
    return '%s%s = %s;' % (m.group(1), m.group(2), '0.0' if 'm_' in m.group(2) or 'gl_' in m.group(2) else '0')

OPS = {'const-float': m_const_float, 'swap-operands': m_swap_operands, 'op-replace': m_op_replace, 'cmp-boundary': m_cmp_boundary,
       'cmp-invert': m_cmp_invert, 'swizzle': m_swizzle, 'fma-swap': m_fma_swap, 'index+1': m_index, 'cond-negate': m_cond_negate,
       'min-max': m_minmax, 'int-const': m_int_const, 'select-swap': m_select_swap, 'drop-output': m_drop_output}


def mutate(src, rnd, op):
    lines = src.split('\n')
    # only lines inside function bodies (indented statements), not struct members or declarations
    cand = [i for i, l in enumerate(lines) if re.match(r'^\s{4,}\S', l) and (';' in l or l.strip().startswith('if')) and '[[' not in l]
    rnd.shuffle(cand)
    for i in cand[:400]:
        nl = OPS[op](lines[i], rnd)
        if nl and nl != lines[i]:
            out = lines[:]; out[i] = nl
            return '\n'.join(out), i + 1, lines[i].strip(), nl.strip()
    return None


def irhash(path):
    r = subprocess.run([os.path.join(HERE, 'irhash.sh'), path], capture_output=True, text=True, timeout=600)
    return r.stdout.strip()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--n', type=int, default=60)
    ap.add_argument('--out', default=os.path.join(HERE, 'work', 'mutation'))
    ap.add_argument('--seed', type=int, default=7)
    ap.add_argument('--vectors', type=int, default=6)
    ap.add_argument('--jobs', type=int, default=8)
    ap.add_argument('--corpus', default=os.path.join(HERE, 'work', 'corpus', 'corpus.jsonl'))
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    rnd = random.Random(a.seed)
    recs = hlib.load_corpus(a.corpus)
    # stratified: pixel units, fetch variants, _b pixel, _b vertex, pulled / vin vertex variants, kernels
    strata = collections.OrderedDict([
        ('bu', [r for r in recs if r['set'] == 'bu']),
        ('bf', [r for r in recs if r['set'] == 'bf']),
        ('b-frag', [r for r in recs if r['set'] == 'b' and any(e.startswith('fragment') for e in r['entries'])]),
        ('b-vert', [r for r in recs if r['set'] == 'b' and any(e.startswith('vertex') for e in r['entries'])]),
        ('bp', [r for r in recs if r['set'] == 'bp']),
        ('bv', [r for r in recs if r['set'] == 'bv']),
        ('kernel', [r for r in recs if any(e.startswith('kernel') for e in r['entries'])]),
    ])
    share = {'bu': 0.2, 'bf': 0.15, 'b-frag': 0.2, 'b-vert': 0.15, 'bp': 0.1, 'bv': 0.1, 'kernel': 0.1}
    picks = []
    for k, rs in strata.items():
        rs = [r for r in rs if r['bytes'] < 120000]
        picks += [(k, r) for r in rnd.sample(rs, max(1, round(a.n * share[k])))]
    ops = list(OPS)
    plan = [(k, r, ops[i % len(ops)], a.seed * 1000 + i) for i, (k, r) in enumerate(picks)]

    def build(p):
        k, rec, op, seed = p
        key = rec['id'].replace(':', '_')
        d = os.path.join(a.out, key)
        os.makedirs(d, exist_ok=True)
        try:
            src, lib, spec = hlib.prepare(rec, d, force_compile=True)
            # the mutation goes into the original source, before augment.py adds the capture twins: a front-end defect
            # is systematic, so main0 and its twin carry it alike (the same augmentation runs on both sides)
            orig = open(rec['src'], errors='replace').read()
            vertex = src.endswith('.aug.metal')
            h0 = irhash(src)
            r = random.Random(seed)
            for attempt in range(25):
                mu = mutate(orig, r, op if attempt < 12 else r.choice(ops))
                if not mu: continue
                text, line, before, after = mu
                if vertex: text, _ = AUG.augment(text)
                ms = os.path.join(d, key + '.mut.metal')
                open(ms, 'w').write(text)
                mlib = os.path.join(d, key + '.mut.metallib')
                try: hlib.compile_msl(ms, mlib)
                except Exception: continue
                h1 = irhash(ms)
                return {'id': rec['id'], 'stratum': k, 'op': op if attempt < 12 else 'fallback', 'line': line, 'before': before, 'after': after,
                        'a': lib, 'b': mlib, 'spec': spec, 'equivalent_ir': h0 == h1, 'entries': rec['entries']}
            return {'id': rec['id'], 'stratum': k, 'op': op, 'error': 'no compilable mutant'}
        except Exception as e:
            return {'id': rec['id'], 'stratum': k, 'op': op, 'error': str(e)[-300:]}

    t0 = time.time()
    with ThreadPoolExecutor(a.jobs) as ex: muts = list(ex.map(build, plan))
    print('built %d mutants in %.0f s' % (len(muts), time.time() - t0))
    jobs = [{'id': m['id'], 'a': m['a'], 'b': m['b'], 'spec': m['spec']} for m in muts if 'error' not in m]
    results, hangs, t_run = hlib.run_batch(jobs, os.path.join(a.out, 'run'), workers=6, extra=['--vectors', str(a.vectors)])
    t_main = t_run
    # not caught: try again with many more vectors (is it an input-coverage gap?)
    missed = [j for j in jobs if results.get(j['id'], {}).get('status') == 'same']
    more = {}
    if missed:
        more, _, _ = hlib.run_batch(missed, os.path.join(a.out, 'run_more'), workers=6, extra=['--vectors', '48', '--seed', '99'])
    rows = []
    for m in muts:
        res = results.get(m['id'], {})
        row = {k: m.get(k) for k in ('id', 'stratum', 'op', 'line', 'before', 'after', 'equivalent_ir', 'error')}
        row['status'] = res.get('status', 'not run')
        row['status_48_vectors'] = more.get(m['id'], {}).get('status')
        row['diff_cases'] = [(c.get('entry'), c.get('fc'), c.get('diff_words')) for c in res.get('cases', []) if c.get('status') == 'diff']
        row['written'] = sum(c.get('written_words', 0) for c in res.get('cases', []))
        rows.append(row)
    valid = [r for r in rows if not r.get('error') and r['status'] not in ('not run', 'error', 'hang', 'skipped')]
    undecidable = [r for r in valid if not r['equivalent_ir'] and r['status'] == 'nondet']
    noneq = [r for r in valid if not r['equivalent_ir'] and r['status'] != 'nondet']
    caught = [r for r in noneq if r['status'] in ('diff', 'mismatch')]
    caught24 = [r for r in noneq if r['status'] in ('diff', 'mismatch') or r['status_48_vectors'] in ('diff', 'mismatch')]
    eq_flagged = [r for r in valid if r['equivalent_ir'] and r['status'] != 'same']
    summary = {'mutants': len(rows), 'valid': len(valid), 'ir_equivalent': sum(1 for r in valid if r['equivalent_ir']),
               'non_equivalent': len(noneq), 'undecidable_nondet': len(undecidable), 'caught': len(caught), 'rate': len(caught) / max(1, len(noneq)),
               'caught_with_48_vectors': len(caught24), 'rate_48': len(caught24) / max(1, len(noneq)),
               'false_alarms_on_equivalent': len(eq_flagged), 'hangs': hangs, 'run_seconds': t_main,
               'job_seconds': sorted(results[j['id']].get('time_s', 0) for j in jobs),
               'by_op': {}, 'by_stratum': {}}
    for key, field in (('by_op', 'op'), ('by_stratum', 'stratum')):
        c = collections.defaultdict(collections.Counter)
        for r in noneq: c[r[field]]['n'] += 1; c[r[field]]['caught'] += r['status'] in ('diff', 'mismatch')
        summary[key] = {k: dict(v) for k, v in c.items()}
    json.dump({'summary': summary, 'rows': rows}, open(os.path.join(a.out, 'mutation.json'), 'w'), indent=1)
    print(json.dumps(summary, indent=1))
    print('not caught (non-equivalent):')
    for r in noneq:
        if r['status'] not in ('diff', 'mismatch'):
            print('  %s %s %s line %s: %s  ->  %s  [48 vectors: %s]' % (r['id'], r['stratum'], r['op'], r['line'], r['before'][:120], r['after'][:120], r['status_48_vectors']))
    print('errors:', [(r['id'], r.get('error') or r['status']) for r in rows if r.get('error') or r['status'] in ('error', 'hang', 'not run')])


if __name__ == '__main__':
    main()
