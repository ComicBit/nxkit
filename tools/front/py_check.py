#!/usr/bin/env python3
"""Runs the prototype checker (msl_parse.py) on a list of files and writes one JSON object per line, in the shape of
`mslcheck --json --stats`. Unlike msl_parse.py it survives the Python exceptions the prototype does not catch: a LexError
becomes the reason `lex-error`, any other exception `crash:<ExceptionName>`. RecursionError stays `recursion`, as in
parse_file. check() mirrors parse_file line by line and is mapped by a multiprocessing pool exactly like msl_parse.py
does, so the Python stack depth (and with it the recursion limit) is the same as in the prototype's own runs.

usage: py_check.py <file list> <out.jsonl>     (J=12 processes by default)"""
import os, sys, re, json, time, multiprocessing
PROTO = os.environ.get('MSL_PROTO', '')
sys.path.insert(0, PROTO)
import msl_parse as M
from msl_lex import LexError


def check(path):
    t0 = time.perf_counter()
    src = open(path, encoding='utf-8', errors='replace').read()
    try:
        p = M.Parser(src); t1 = time.perf_counter(); p.run()
    except M.Reject as e:
        return {'path': path, 'ok': False, 'reasons': {e.key: 1}, 'stats': {}, 'sec': time.perf_counter() - t0, 'detail': ''}
    except RecursionError:
        return {'path': path, 'ok': False, 'reasons': {'recursion': 1}, 'stats': {}, 'sec': time.perf_counter() - t0, 'detail': ''}
    except LexError as e:
        return {'path': path, 'ok': False, 'reasons': {'lex-error': 1}, 'stats': {}, 'sec': time.perf_counter() - t0, 'detail': str(e)}
    except Exception as e:
        return {'path': path, 'ok': False, 'reasons': {'crash:' + type(e).__name__: 1}, 'stats': {}, 'sec': time.perf_counter() - t0, 'detail': ''}
    dt = time.perf_counter() - t0
    st = dict(p.stats)
    st['ssa-names-reassigned'] = sum(1 for k in p.assigned if re.fullmatch(r'_\d+', k))
    return {'path': path, 'ok': not p.reasons, 'reasons': dict(p.reasons), 'stats': st, 'sec': dt, 'detail': getattr(p, 'last_detail', '')}


if __name__ == '__main__':
    files = [l.strip() for l in open(sys.argv[1]) if l.strip()]
    with multiprocessing.Pool(int(os.environ.get('J', 12))) as pool:
        res = pool.map(check, files, chunksize=8)
    with open(sys.argv[2], 'w') as f:
        for r in res: f.write(json.dumps(r) + '\n')
    ok = sum(r['ok'] for r in res)
    print(f'{ok}/{len(res)} accepted')
