#!/usr/bin/env python3
"""Checks the shared case file (crates/msl-parse/tests/cases.msl) against the Python prototype, so that the expectations
the Rust tests assert are the prototype's own verdicts.

usage: check_cases.py [cases.msl]
For a case with `prelude: <helper>` the hashes of its template items are added to the prototype's PRELUDE table (in
memory only) under that helper name."""
import os, sys, collections
PROTO = os.environ.get('MSL_PROTO', '')
sys.path.insert(0, PROTO)
import msl_parse as M
from msl_lex import tokens, LexError

HERE = os.path.dirname(os.path.abspath(__file__))


def load(path):
    cases, cur = [], None
    for line in open(path).read().split('\n'):
        if line.startswith('=== '):
            cur = {'name': line[4:].strip(), 'expect': None, 'prelude': None, 'src': [], 'hdr': True}
            cases.append(cur); continue
        if cur is None: continue
        if cur['hdr'] and line.startswith('expect: '): cur['expect'] = line[8:].strip(); continue
        if cur['hdr'] and line.startswith('prelude: '): cur['prelude'] = line[9:].strip(); continue
        cur['hdr'] = False
        cur['src'].append(line)
    for c in cases: c['src'] = '\n'.join(c['src'])
    return cases


def template_hashes(src):
    toks = tokens(src) + [('eof', '<eof>', 0)]
    p = M.Parser.__new__(M.Parser); p.t = toks; i = 0; out = []
    while toks[i][0] != 'eof':
        end = M.Parser.item_end(p, i)
        if toks[i][1] == 'template': out.append(M.text_hash(toks[i:end]))
        i = end
    return out


def verdict(src):
    try:
        p = M.Parser(src); p.run()
    except M.Reject as e:
        return 'reject ' + e.key
    except RecursionError:
        return 'recursion'
    except LexError:
        return 'lex-error'
    except Exception as e:
        return 'crash:' + type(e).__name__
    if not p.reasons: return 'accept'
    keys = []
    for k, n in p.reasons.items(): keys += [k] * n
    return 'reject ' + ' | '.join(keys)


def canon(v):
    if not v.startswith('reject '): return v
    return 'reject ' + ' | '.join(sorted(v[7:].split(' | ')))


if __name__ == '__main__':
    path = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, '..', 'crates', 'msl-parse', 'tests', 'cases.msl')
    orig = dict(M.PRELUDE); bad = 0
    cases = load(path)
    for c in cases:
        M.PRELUDE.clear(); M.PRELUDE.update(orig)
        if c['prelude']:
            for h in template_hashes(c['src']): M.PRELUDE[h] = c['prelude']
        got = verdict(c['src'])
        if canon(got) != canon(c['expect']):
            bad += 1; print(f"{c['name']}: expected {c['expect']!r}, prototype says {got!r}")
    print(f'{len(cases) - bad}/{len(cases)} cases agree with the prototype')
    sys.exit(1 if bad else 0)
